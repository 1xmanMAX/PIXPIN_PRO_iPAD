//! **Cambiar lo que dicen los textos que ya hay**: traducir, corregir,
//! renombrar, y que quepa.
//!
//! Pedido del usuario (22-oct-2026): «traduce esa figura a chino» y el
//! asistente contestaba que no tenia herramienta. Ahora es UNA llamada con
//! la lista `[{ref, texto}]`: cada texto (o la caja que lo lleva de rotulo)
//! cambia lo que dice y nada mas —sitio, letra, color y alineacion se
//! quedan—, en un solo paso de deshacer.
//!
//! Lo que hace falta para que no quede peor que antes:
//!
//! - **Se vuelve a medir** con `texto::medida` (o la del texto rico): un
//!   chino es mas corto que su espanol y un aleman mas largo, y una caja con
//!   la medida vieja corta o sobra.
//! - **Lo centrado sigue centrado** y lo de la derecha sigue pegado a la
//!   derecha: se ancla el punto que la alineacion dice, no la esquina. Un
//!   texto suelto centrado a ojo dentro de una caja (como los dibuja
//!   `dibujar_en_lienzo`) tambien.
//! - **El rotulo de una caja** se parte a lo ancho de su hueco y se coloca
//!   con `texto_en_figuras`, las mismas cuentas que el editor.
//! - **Si ya no cabe, se dice** (y con «ajustar» se encoge la letra o se
//!   parte en renglones): el modelo no ve la pantalla.

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::texto::{self as tx, AlineacionTexto, EstiloDeTexto};
use pixpin_motor2d::texto_en_figuras as tf;
use serde_json::Value;

use super::referencia;
use super::{Contexto, Edicion, Hecho, listar};

/// La letra mas pequena a la que se encoge un texto para que quepa: por
/// debajo ya no se lee.
pub const TAM_MINIMO: f32 = 8.0;

/// Cuantos textos como mucho devuelve «listar» de una vez.
const MAX_LISTA: usize = 120;

/// Que hacer con un texto que no cabe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ajuste {
    /// Nada: se avisa.
    No,
    Encoger,
    Partir,
    /// Partir y, si aun no cabe, encoger.
    Auto,
}

impl Ajuste {
    fn de(t: Option<&str>) -> Ajuste {
        match t {
            Some("encoger") => Ajuste::Encoger,
            Some("partir") => Ajuste::Partir,
            Some("auto") => Ajuste::Auto,
            _ => Ajuste::No,
        }
    }
    fn parte(self) -> bool {
        matches!(self, Ajuste::Partir | Ajuste::Auto)
    }
    fn encoge(self) -> bool {
        matches!(self, Ajuste::Encoger | Ajuste::Auto)
    }
}

// ---------------------------------------------------------------------------
// De quien es cada texto

fn es_texto(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Texto { .. })
}

/// **La caja de la que es rotulo atado** este texto (`containerId`), si lo es.
pub fn caja_del_rotulo(escena: &Escena, e: &Elemento) -> Option<u64> {
    if !es_texto(e) || e.extras.contenedor.is_none() {
        return None;
    }
    tf::contenedor_de(
        e,
        &escena.elementos,
        &pixpin_motor2d::enlace::id_de_texto_de,
    )
    .map(|c| c.id)
}

/// **El rotulo atado de una caja**, si lo lleva.
pub fn rotulo_de(escena: &Escena, e: &Elemento) -> Option<u64> {
    if !tf::admite_texto_dentro(&e.figura) || e.extras.atados.is_empty() {
        return None;
    }
    tf::texto_de(
        &pixpin_motor2d::enlace::id_de_texto_de(e),
        &escena.elementos,
    )
    .map(|t| t.id)
}

/// Si es una figura cerrada de las que llevan texto dentro.
fn es_caja(e: &Elemento) -> bool {
    tf::admite_texto_dentro(&e.figura)
}

/// **La caja que rodea a un texto suelto** (no atado): la figura cerrada mas
/// pequena que tiene dentro el centro del texto y es de su talla (al menos
/// tan alta como el, y no mucho mas estrecha). Es como `dibujar_en_lienzo`
/// pone los rotulos: un texto centrado encima de un rectangulo, sin atar.
pub fn caja_que_lo_rodea<'a>(escena: &'a Escena, t: &Elemento) -> Option<&'a Elemento> {
    let (x0, y0, x1, y1) = t.caja();
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (w, h) = (x1 - x0, y1 - y0);
    escena
        .a_la_vista()
        .filter(|c| c.id != t.id && es_caja(c) && !c.extras.referencia)
        .filter(|c| {
            let (a0, b0, a1, b1) = c.caja();
            let (cw, ch) = (a1 - a0, b1 - b0);
            cx >= a0
                && cx <= a1
                && cy >= b0
                && cy <= b1
                && ch >= h * 0.9
                && cw >= w * 0.3
                && cw * ch >= 0.5 * w * h
        })
        .min_by(|a, b| {
            let area = |e: &Elemento| e.ancho * e.alto;
            area(a)
                .partial_cmp(&area(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// **El texto que se cambia al nombrar `id`**: el mismo si es un texto; su
/// rotulo atado si es una caja; o el unico texto suelto que tiene encima.
pub fn texto_para(escena: &Escena, id: u64) -> Option<u64> {
    let e = escena.buscar(id).filter(|e| !e.borrado)?;
    if es_texto(e) {
        return Some(id);
    }
    if let Some(t) = rotulo_de(escena, e) {
        return Some(t);
    }
    if !es_caja(e) {
        return None;
    }
    let encima: Vec<u64> = escena
        .a_la_vista()
        .filter(|t| es_texto(t) && t.extras.contenedor.is_none())
        .filter(|t| caja_que_lo_rodea(escena, t).is_some_and(|c| c.id == id))
        .map(|t| t.id)
        .collect();
    (encima.len() == 1).then(|| encima[0])
}

// ---------------------------------------------------------------------------
// Medir

fn estilo_de(e: &Elemento) -> EstiloDeTexto {
    EstiloDeTexto {
        negrita: e.extras.negrita,
        cursiva: e.extras.cursiva,
        tachado: e.extras.tachado,
    }
}

/// Los renglones de `texto` a un tamano, partidos a `ancho` si se da.
fn renglones(
    texto: &str,
    tam: f32,
    familia: &str,
    estilo: EstiloDeTexto,
    ancho: Option<f32>,
) -> Vec<String> {
    match ancho {
        Some(w) => {
            tf::repartir_en_lineas(texto, w, &|s: &str| tx::ancho_real(s, tam, familia, estilo))
        }
        None => texto.split('\n').map(str::to_string).collect(),
    }
}

/// Lo que ocupa de verdad el renglon mas largo (sin la holgura del cursor).
fn ancho_de(renglones: &[String], tam: f32, familia: &str, estilo: EstiloDeTexto) -> f32 {
    renglones
        .iter()
        .map(|r| tx::ancho_real(r.trim_end(), tam, familia, estilo))
        .fold(0.0, f32::max)
}

fn alto_de(n: usize, tam: f32, familia: &str) -> f32 {
    n.max(1) as f32 * tam * tx::interlineado_de(familia).unwrap_or(tx::INTERLINEADO)
}

/// **Si un texto cabe donde esta**: `Some((ancho, alto, cabe_ancho,
/// cabe_alto, caja))` con lo que mide y lo que hay, si NO cabe. Lo usa la
/// revision (`lint.rs`) con las mismas cuentas que el cambio.
pub fn no_cabe(escena: &Escena, t: &Elemento) -> Option<(f32, f32, f32, f32, u64)> {
    let Figura::Texto {
        texto,
        tam,
        familia,
    } = &t.figura
    else {
        return None;
    };
    let estilo = estilo_de(t);
    let plano = t
        .extras
        .rico
        .as_deref()
        .map(pixpin_motor2d::texto_rico::plano)
        .unwrap_or_else(|| texto.clone());
    if let Some(c) = tf::contenedor_de(
        t,
        &escena.elementos,
        &pixpin_motor2d::enlace::id_de_texto_de,
    ) {
        let (cw, ch) = (tf::ancho_que_cabe(c), tf::alto_que_cabe(c));
        let r = renglones(&plano, *tam, familia, estilo, Some(cw.max(1.0)));
        let (w, h) = (
            ancho_de(&r, *tam, familia, estilo),
            alto_de(r.len(), *tam, familia),
        );
        return (w > cw + 1.0 || h > ch + 1.0).then_some((w, h, cw, ch, c.id));
    }
    let c = caja_que_lo_rodea(escena, t)?;
    let r = renglones(&plano, *tam, familia, estilo, None);
    let (w, h) = (
        ancho_de(&r, *tam, familia, estilo),
        alto_de(r.len(), *tam, familia),
    );
    // Suelto encima de una caja: lo que la caja tiene de ancho (con el aire
    // de los bordes), o el hueco de dentro si es un ovalo o un rombo.
    let cw = match c.figura {
        Figura::Rectangulo => c.ancho - 2.0,
        _ => tf::ancho_que_cabe(c) + 2.0 * tf::RELLENO_DEL_TEXTO,
    };
    let ch = match c.figura {
        Figura::Rectangulo => c.alto - 2.0,
        _ => tf::alto_que_cabe(c) + 2.0 * tf::RELLENO_DEL_TEXTO,
    };
    (w > cw + 1.0 || h > ch + 1.0).then_some((w, h, cw, ch, c.id))
}

// ---------------------------------------------------------------------------
// Cambiar uno

/// Lo que paso con un texto.
#[derive(Debug, Default)]
struct Informe {
    cambiado: bool,
    /// Ya no cabe: el aviso para el modelo.
    desborda: Option<String>,
    /// Se encogio la letra a este tamano.
    encogido: Option<f32>,
}

/// **Pone `nuevo` en el texto `id`** (o lo deja como esta si es `None`) y lo
/// ajusta. Quien llama abre el paso de deshacer; aqui se apunta la edicion.
fn poner(
    escena: &mut Escena,
    id: u64,
    nuevo: Option<&str>,
    ajuste: Ajuste,
    ancho_max: Option<f32>,
) -> Informe {
    let Some(antes) = escena.buscar(id).cloned() else {
        return Informe::default();
    };
    let Figura::Texto {
        texto: viejo,
        tam: tam0,
        familia,
    } = &antes.figura
    else {
        return Informe::default();
    };
    let familia = familia.clone();
    let estilo = estilo_de(&antes);
    let fuente_vieja = antes.extras.rico.clone().unwrap_or_else(|| viejo.clone());
    let base = nuevo.map(str::to_string).unwrap_or(fuente_vieja);
    let rico = pixpin_motor2d::texto_rico::tiene_marcas(&base);
    let plano = if rico {
        pixpin_motor2d::texto_rico::plano(&base)
    } else {
        base.clone()
    };
    let atada = tf::contenedor_de(
        &antes,
        &escena.elementos,
        &pixpin_motor2d::enlace::id_de_texto_de,
    )
    .cloned();
    let rodea = if atada.is_none() {
        caja_que_lo_rodea(escena, &antes).cloned()
    } else {
        None
    };
    let mut x = antes.clone();
    let mut informe = Informe::default();
    let mut tam = *tam0;

    if let Some(c) = &atada {
        // **Rotulo de una caja**: se parte a lo ancho del hueco (como lo
        // pinta `pintado.rs`) y se coloca con su alineacion.
        let (cw, ch) = (
            tf::ancho_que_cabe(c).max(4.0),
            tf::alto_que_cabe(c).max(4.0),
        );
        let medir = |t: f32| {
            let r = renglones(&plano, t, &familia, estilo, Some(cw));
            (
                ancho_de(&r, t, &familia, estilo),
                alto_de(r.len(), t, &familia),
            )
        };
        let (mut w, mut h) = medir(tam);
        if ajuste.encoge() {
            while (w > cw + 0.5 || h > ch + 0.5) && tam > TAM_MINIMO {
                tam = (tam * 0.92).max(TAM_MINIMO);
                (w, h) = medir(tam);
            }
        }
        if let Figura::Texto { tam: t, .. } = &mut x.figura {
            *t = tam;
        }
        pixpin_motor2d::texto_rico::poner_fuente(&mut x, &base);
        // Un renglon un pelo mas ancho que lo escrito: al pintarlo se parte
        // otra vez a este ancho y tiene que salir lo mismo.
        x.ancho = (w + 1.0).max(1.0);
        x.alto = h;
        let p = tf::sitio_alineado_dentro(
            c,
            (x.ancho, x.alto),
            x.extras.alineacion.unwrap_or_default(),
            x.extras.alineacion_vertical.unwrap_or_default(),
        );
        x.x = p.x;
        x.y = p.y;
        if w > cw + 1.0 || h > ch + 1.0 {
            informe.desborda = Some(format!(
                "#{id} no cabe en #{} (mide {}x{} y hay {}x{})",
                c.id,
                w.round(),
                h.round(),
                cw.round(),
                ch.round()
            ));
        }
    } else {
        // **Suelto**: se mide entero y se ancla por donde dice su
        // alineacion (o por el centro, si estaba centrado en su caja).
        let (x0, y0, x1, y1) = antes.caja();
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let limite_w = ancho_max
            .or(rodea.as_ref().map(tf::ancho_que_cabe))
            .map(|w| w.max(4.0));
        let limite_h = if ancho_max.is_some() {
            None
        } else {
            rodea.as_ref().map(tf::alto_que_cabe)
        };
        let partir = ajuste.parte() && !rico && limite_w.is_some();
        let componer = |t: f32| -> (String, f32, f32) {
            let f = if partir {
                renglones(&base, t, &familia, estilo, limite_w).join("\n")
            } else {
                base.clone()
            };
            let r = renglones(&plano_de(&f, rico), t, &familia, estilo, None);
            let (w, h) = (
                ancho_de(&r, t, &familia, estilo),
                alto_de(r.len(), t, &familia),
            );
            (f, w, h)
        };
        let sobra = |w: f32, h: f32| {
            limite_w.is_some_and(|l| w > l + 0.5) || limite_h.is_some_and(|l| h > l + 0.5)
        };
        let (mut f, mut w, mut h) = componer(tam);
        if ajuste.encoge() {
            while sobra(w, h) && tam > TAM_MINIMO {
                tam = (tam * 0.92).max(TAM_MINIMO);
                (f, w, h) = componer(tam);
            }
        }
        if let Figura::Texto { tam: t, .. } = &mut x.figura {
            *t = tam;
        }
        pixpin_motor2d::texto_rico::poner_fuente(&mut x, &f);
        let centrado_en_su_caja = rodea
            .as_ref()
            .is_some_and(|c| (cx - (c.x + c.ancho / 2.0)).abs() < 3.0);
        match antes.extras.alineacion {
            Some(AlineacionTexto::Centro) => x.x = cx - x.ancho / 2.0,
            Some(AlineacionTexto::Derecha) => x.x = x1 - x.ancho,
            None if centrado_en_su_caja => x.x = cx - x.ancho / 2.0,
            _ => {}
        }
        // A media altura de su caja: sigue a media altura con mas o menos
        // renglones.
        if rodea
            .as_ref()
            .is_some_and(|c| (cy - (c.y + c.alto / 2.0)).abs() < 0.15 * c.alto)
        {
            x.y = cy - x.alto / 2.0;
        }
        if sobra(w, h) {
            let donde = match &rodea {
                Some(c) if ancho_max.is_none() => format!("en #{}", c.id),
                _ => format!("en {} px", limite_w.unwrap_or(0.0).round()),
            };
            informe.desborda = Some(format!(
                "#{id} no cabe {donde} (mide {}x{} y hay {}x{})",
                w.round(),
                h.round(),
                limite_w.unwrap_or(w).round(),
                limite_h.unwrap_or(h).round()
            ));
        }
    }
    if (tam - *tam0).abs() > 0.01 {
        informe.encogido = Some(tam);
    }
    let igual = {
        let mut a = antes.clone();
        let mut b = x.clone();
        a.version = 0;
        b.version = 0;
        a == b
    };
    if igual {
        return informe;
    }
    escena.apuntar_edicion(id);
    if let Some(e) = escena.buscar_mut(id) {
        *e = x;
        e.tocar();
    }
    informe.cambiado = true;
    informe
}

fn plano_de(f: &str, rico: bool) -> String {
    if rico {
        pixpin_motor2d::texto_rico::plano(f)
    } else {
        f.to_string()
    }
}

// ---------------------------------------------------------------------------
// Elegir los textos

/// Los `#n` escritos en una referencia.
fn almohadillas(q: &str) -> Vec<u64> {
    q.split(|c: char| !(c.is_ascii_digit() || c == '#'))
        .filter_map(|t| t.strip_prefix('#'))
        .filter_map(|t| t.parse().ok())
        .collect()
}

/// **Un error que dice como arreglarlo**: el #id que no existe con los
/// textos de id parecido, o lo que conto la referencia con la pista de
/// «listar».
fn error_de(escena: &Escena, que: &str, m: &str) -> String {
    let textos: Vec<&Elemento> = escena.a_la_vista().filter(|e| es_texto(e)).collect();
    let faltan: Vec<u64> = almohadillas(que)
        .into_iter()
        .filter(|id| escena.buscar(*id).is_none_or(|e| e.borrado))
        .collect();
    if let Some(&id) = faltan.first() {
        let mut cerca: Vec<&Elemento> = textos.clone();
        cerca.sort_by_key(|e| e.id.abs_diff(id));
        let sugeridos: Vec<String> = cerca
            .iter()
            .take(3)
            .map(|e| {
                let t = match &e.figura {
                    Figura::Texto { texto, .. } => texto
                        .chars()
                        .take(24)
                        .collect::<String>()
                        .replace('\n', " "),
                    _ => String::new(),
                };
                format!("#{} \"{t}\"", e.id)
            })
            .collect();
        return if sugeridos.is_empty() {
            format!("no existe #{id} y en el lienzo no hay textos")
        } else {
            format!(
                "no existe #{id}; textos con id parecido: {} (accion «listar» da todos)",
                sugeridos.join(" ")
            )
        };
    }
    let primera = m.lines().next().unwrap_or(m);
    format!("{primera} (accion «listar» da el #id de cada texto)")
}

/// **Un solo texto** por su referencia (un texto, o una caja con rotulo).
fn uno_de(escena: &Escena, que: &str, ctx: &Contexto) -> Result<u64, String> {
    let r = referencia::resolver(escena, que, ctx, None).map_err(|m| error_de(escena, que, &m))?;
    let mut ids: Vec<u64> = r
        .ids
        .iter()
        .filter_map(|id| texto_para(escena, *id))
        .collect();
    ids.dedup();
    match ids.len() {
        1 => Ok(ids[0]),
        0 => {
            let n = r
                .ids
                .first()
                .and_then(|id| escena.buscar(*id))
                .map(super::nombre_de)
                .unwrap_or("figura");
            Err(format!(
                "{que} es un {n} sin texto: para ponerle uno nuevo usa dibujar_en_lienzo"
            ))
        }
        n => Err(format!(
            "«{que}» encaja con {n} textos; di cual por su #id:\n{}",
            listar(escena, &ids, ctx.marco)
        )),
    }
}

/// **Varios textos**: todos los que nombra la referencia («todo», «los
/// textos», «#3 #5»), y los rotulos de las cajas nombradas.
fn varios_de(escena: &Escena, que: &str, ctx: &Contexto) -> Result<Vec<u64>, String> {
    let r = referencia::resolver(escena, que, ctx, None).map_err(|m| error_de(escena, que, &m))?;
    let mut ids: Vec<u64> = Vec::new();
    for id in r.ids.iter().filter_map(|id| texto_para(escena, *id)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(format!("en «{que}» no hay textos"));
    }
    Ok(ids)
}

// ---------------------------------------------------------------------------
// La herramienta

pub(super) fn editar_texto(
    escena: &mut Escena,
    e: &Edicion,
    ctx: &Contexto,
) -> Result<Hecho, String> {
    let hay_cambios = e.args.contains_key("cambios") || e.texto("texto").is_some();
    let ajuste = Ajuste::de(e.texto("ajustar"));
    let accion = e.texto("accion").unwrap_or(if hay_cambios {
        "cambiar"
    } else if ajuste != Ajuste::No {
        "ajustar"
    } else {
        "listar"
    });
    let ancho_max = e
        .numero("ancho_max")
        .map(|w| w as f32)
        .filter(|w| *w >= 4.0);
    match accion {
        "listar" => Ok(listar_textos(escena, e, ctx)),
        "ajustar" => {
            let ids = varios_de(escena, e.texto("que").unwrap_or("todo"), ctx)?;
            let ajuste = if ajuste == Ajuste::No {
                Ajuste::Auto
            } else {
                ajuste
            };
            let pares: Vec<(u64, Option<String>)> = ids.into_iter().map(|id| (id, None)).collect();
            Ok(aplicar_todos(
                escena,
                &pares,
                ajuste,
                ancho_max,
                Vec::new(),
                ctx,
            ))
        }
        _ => {
            let mut lista: Vec<(String, String)> = e
                .args
                .get("cambios")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|c| {
                            Some((
                                c.get("ref")?.as_str()?.to_string(),
                                c.get("texto")?.as_str()?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Some(t) = e.texto("texto") {
                let que = e.texto("que").ok_or(
                    "con «texto» hace falta «que»: el #id del texto que cambia (o usa «cambios»)",
                )?;
                lista.push((que.to_string(), t.to_string()));
            }
            if lista.is_empty() {
                return Err("para cambiar hace falta «cambios» [{\"ref\": \"#3\", \"texto\": \"...\"}] o «que» con «texto»".into());
            }
            let mut fallos = Vec::new();
            let mut pares: Vec<(u64, Option<String>)> = Vec::new();
            for (r, t) in lista {
                match uno_de(escena, &r, ctx) {
                    Ok(id) if escena.buscar(id).is_some_and(|x| x.bloqueado) => {
                        fallos.push(format!(
                            "#{id} esta bloqueado: el usuario lo dejo quieto a proposito"
                        ));
                    }
                    Ok(id) => {
                        // Dos cambios al mismo: vale el ultimo.
                        pares.retain(|(i, _)| *i != id);
                        pares.push((id, Some(t)));
                    }
                    Err(m) => fallos.push(format!("{r}: {m}")),
                }
            }
            if pares.is_empty() {
                return Err(format!("no se cambio nada:\n{}", fallos.join("\n")));
            }
            Ok(aplicar_todos(
                escena, &pares, ajuste, ancho_max, fallos, ctx,
            ))
        }
    }
}

/// Cambia (o ajusta) todos en UN paso y escribe el «hecho».
fn aplicar_todos(
    escena: &mut Escena,
    pares: &[(u64, Option<String>)],
    ajuste: Ajuste,
    ancho_max: Option<f32>,
    fallos: Vec<String>,
    ctx: &Contexto,
) -> Hecho {
    let mut cambiados = Vec::new();
    let mut iguales = Vec::new();
    let mut avisos = Vec::new();
    let mut encogidos = Vec::new();
    escena.abrir_paso();
    for (id, nuevo) in pares {
        let inf = poner(escena, *id, nuevo.as_deref(), ajuste, ancho_max);
        if inf.cambiado {
            cambiados.push(*id);
        } else {
            iguales.push(format!("#{id}"));
        }
        if let Some(t) = inf.encogido {
            encogidos.push(format!("#{id} a {}px", t.round()));
        }
        avisos.extend(inf.desborda);
    }
    escena.cerrar_paso();
    let n = cambiados.len();
    let mut s = format!(
        "hecho: {n} texto{} cambiado{}: {}",
        if n == 1 { "" } else { "s" },
        if n == 1 { "" } else { "s" },
        cambiados
            .iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    if !encogidos.is_empty() {
        s.push_str(&format!("\nletra encogida: {}", encogidos.join(", ")));
    }
    if !iguales.is_empty() {
        s.push_str(&format!("\nsin cambio (ya era asi): {}", iguales.join(" ")));
    }
    if !avisos.is_empty() {
        s.push_str(&format!(
            "\nno caben (repite con «ajustar»: «auto», o agranda la caja con transformar_elementos):\n{}",
            avisos.join("\n")
        ));
    }
    if !fallos.is_empty() {
        s.push_str(&format!("\nno se pudo:\n{}", fallos.join("\n")));
    }
    // La lista corta de lo cambiado, para encadenar (hasta 12).
    let lista = listar(escena, &cambiados[..cambiados.len().min(12)], ctx.marco);
    if !lista.is_empty() {
        s.push('\n');
        s.push_str(&lista);
    }
    Hecho {
        texto: s,
        ids: cambiados,
    }
}

/// **Todos los textos con su #id y su texto entero**, en orden de lectura
/// (de arriba abajo y de izquierda a derecha), para traducir o corregir sin
/// dejarse ninguno. Sin ids en el «hecho»: no toca nada.
fn listar_textos(escena: &Escena, e: &Edicion, ctx: &Contexto) -> Hecho {
    let elegidos: Option<Vec<u64>> = e.texto("que").and_then(|q| varios_de(escena, q, ctx).ok());
    let mut textos: Vec<&Elemento> = escena
        .a_la_vista()
        .filter(|x| es_texto(x) && !x.extras.referencia)
        .filter(|x| elegidos.as_ref().is_none_or(|v| v.contains(&x.id)))
        .collect();
    textos.sort_by(|a, b| {
        let (ya, yb) = ((a.y / 10.0).round(), (b.y / 10.0).round());
        ya.partial_cmp(&yb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
    let total = textos.len();
    if total == 0 {
        return Hecho {
            texto: "en el lienzo no hay textos".into(),
            ids: Vec::new(),
        };
    }
    let desde = (e.numero("desde").unwrap_or(0.0).max(0.0) as usize).min(total);
    let r = |v: f32| v.round() as i64;
    let mut s = format!("{total} textos (de arriba abajo):");
    let mut puestos = 0;
    for t in textos.iter().skip(desde).take(MAX_LISTA) {
        let Figura::Texto { texto, tam, .. } = &t.figura else {
            continue;
        };
        let (x0, y0, x1, y1) = t.caja();
        s.push_str(&format!(
            "\n#{} \"{}\" [{},{},{},{}] letra {}",
            t.id,
            texto.replace('\n', "\\n"),
            r(x0 - ctx.marco.0),
            r(y0 - ctx.marco.1),
            r(x1 - x0),
            r(y1 - y0),
            r(*tam)
        ));
        if let Some(c) = caja_del_rotulo(escena, t) {
            s.push_str(&format!(" (rotulo de #{c})"));
        }
        puestos += 1;
    }
    if desde + puestos < total {
        s.push_str(&format!(
            "\n(+{} mas: repite con «desde»: {})",
            total - desde - puestos,
            desde + puestos
        ));
    }
    Hecho {
        texto: s,
        ids: Vec::new(),
    }
}
