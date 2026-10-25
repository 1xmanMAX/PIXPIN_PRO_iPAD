//! **Cada herramienta de editar, hecha con las operaciones del motor.**
//!
//! Lo que se traza (lapiz, grafito, resaltador, recta, circulo, arco) nace de
//! un [`Gesto`] del motor al que se le dan pulsar, mover y soltar —el mismo
//! camino que el raton en el editor, sin ventana ni teclado ni raton del
//! sistema—: asi sale con la pluma, la presion simulada, el material, la
//! semilla y el estilo exactos de lo dibujado a mano. Lo demas llama a la
//! funcion del motor que usa el editor para lo mismo.

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta as Mano};
use pixpin_motor2d::seleccion::Seleccion;
use pixpin_motor2d::tinta::MaterialTinta;
use pixpin_motor2d::tinta::grafito::Dureza;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{estilo, organizar, portapapeles, recorte, recorte_imagen, transformar};

use super::referencia::{self, uno, varios};
use super::{
    Anfitrion, CORTAR, Contexto, ESTILO, EXTENDER, Edicion, GEOMETRIA, Hecho, IMAGEN, SELECCIONAR, TRANSFORMAR, TRAZAR,
    listar,
};

pub(super) fn hacer(escena: &mut Escena, e: &Edicion, ctx: &Contexto, an: &mut dyn Anfitrion) -> Result<Hecho, String> {
    match e.herramienta {
        SELECCIONAR => seleccionar(escena, e, ctx),
        TRANSFORMAR => transformar_(escena, e, ctx),
        ESTILO => cambiar_estilo(escena, e, ctx),
        TRAZAR => trazar(escena, e, ctx),
        GEOMETRIA => geometria(escena, e, ctx),
        CORTAR => cortar(escena, e, ctx),
        EXTENDER => extender(escena, e, ctx),
        IMAGEN => imagen(escena, e, ctx, an),
        otro => Err(format!("la herramienta «{otro}» no es de editar")),
    }
}

/// El texto de «hecho» con la lista corta de lo tocado, para encadenar.
fn hecho(escena: &Escena, que: &str, ids: Vec<u64>, ctx: &Contexto) -> Hecho {
    let lista = listar(escena, &ids, ctx.marco);
    Hecho {
        texto: if lista.is_empty() { format!("hecho: {que}") } else { format!("hecho: {que}\n{lista}") },
        ids,
    }
}

fn seleccion_de(ids: &[u64]) -> Seleccion {
    let mut s = Seleccion::nueva();
    s.poner_todos(ids.iter().copied());
    s
}

fn color_rgba(texto: &str) -> Result<ColorRgba, String> {
    let c = crate::herramientas::color(texto).ok_or_else(|| format!("no conozco el color «{texto}»"))?;
    let v = u32::from_str_radix(c.0.trim_start_matches('#'), 16).map_err(|_| format!("color raro «{texto}»"))?;
    Ok(ColorRgba::opaco(
        ((v >> 16) & 0xff) as f32 / 255.0,
        ((v >> 8) & 0xff) as f32 / 255.0,
        (v & 0xff) as f32 / 255.0,
    ))
}

fn puntos_de(v: &[f64], ctx: &Contexto) -> Vec<Punto2> {
    v.chunks_exact(2).map(|p| ctx.a_escena(p[0], p[1])).collect()
}

fn punto(e: &Edicion, campo: &str, ctx: &Contexto) -> Option<Punto2> {
    e.numeros(campo).filter(|v| v.len() >= 2).map(|v| ctx.a_escena(v[0], v[1]))
}

fn trazo_de(t: &str) -> EstiloTrazo {
    match t {
        "discontinuo" => EstiloTrazo::Discontinuo,
        "punteado" => EstiloTrazo::Punteado,
        "raya_punto" | "rayapunto" => EstiloTrazo::RayaPunto,
        _ => EstiloTrazo::Solido,
    }
}

// ---------------------------------------------------------------------------
// Elegir

fn seleccionar(escena: &mut Escena, e: &Edicion, ctx: &Contexto) -> Result<Hecho, String> {
    let que = e.texto("que").unwrap_or("todo");
    let zona = e
        .numeros("zona")
        .filter(|z| z.len() >= 4)
        .map(|z| (z[0] as f32 + ctx.marco.0, z[1] as f32 + ctx.marco.1, z[2] as f32, z[3] as f32));
    let r = referencia::resolver(escena, que, ctx, zona)?;
    if r.ids.is_empty() {
        return Err(format!("no hay nada que encaje con «{que}» ahi"));
    }
    let n = r.ids.len();
    Ok(hecho(escena, &format!("{n} encontrado{}", if n == 1 { "" } else { "s" }), r.ids, ctx))
}

// ---------------------------------------------------------------------------
// Mover, escalar, girar, duplicar, borrar, agrupar, ordenar, voltear

fn caja_de(escena: &Escena, ids: &[u64]) -> Option<(f32, f32, f32, f32)> {
    ids.iter()
        .filter_map(|id| escena.buscar(*id))
        .map(Elemento::caja)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

fn transformar_(escena: &mut Escena, e: &Edicion, ctx: &Contexto) -> Result<Hecho, String> {
    let que = e.texto("que").unwrap_or_default();
    let ids = varios(escena, que, ctx)?;
    // Lo bloqueado se ve pero no se toca (como con el raton).
    let ids: Vec<u64> = ids.into_iter().filter(|id| escena.buscar(*id).is_some_and(|x| !x.bloqueado)).collect();
    if ids.is_empty() {
        return Err("esta bloqueado: el usuario lo dejo quieto a proposito".into());
    }
    let sel = seleccion_de(&ids);
    let accion = e.texto("accion").unwrap_or("mover");
    let (x0, y0, x1, y1) = caja_de(escena, &ids).ok_or("no tiene medida")?;
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let editar = |escena: &mut Escena, f: &mut dyn FnMut(&mut Elemento)| {
        escena.abrir_paso();
        for id in &ids {
            escena.apuntar_edicion(*id);
            if let Some(x) = escena.buscar_mut(*id) {
                f(x);
                x.tocar();
            }
        }
        escena.cerrar_paso();
    };
    match accion {
        "mover" => {
            let (dx, dy) = match punto(e, "a", ctx) {
                Some(p) => (p.x - x0, p.y - y0),
                None => (e.numero("dx").unwrap_or(0.0) as f32, e.numero("dy").unwrap_or(0.0) as f32),
            };
            if dx == 0.0 && dy == 0.0 {
                return Err("para mover hace falta «dx»/«dy» o «a»".into());
            }
            editar(escena, &mut |x| x.mover(dx, dy));
            Ok(hecho(escena, "movido", ids, ctx))
        }
        "escalar" => {
            let f = e.numero("factor").ok_or("para escalar hace falta «factor»")? as f32;
            if !(0.05..=20.0).contains(&f) {
                return Err("«factor» tiene que ir de 0.05 a 20".into());
            }
            // El tirador de la esquina, con Shift (proporcional) y Alt (desde
            // el centro): lo mismo que estirar con el raton.
            editar(escena, &mut |x| {
                let (a0, b0, a1, b1) = x.caja();
                let c = if ids.len() == 1 { Punto2::nuevo((a0 + a1) / 2.0, (b0 + b1) / 2.0) } else { centro };
                if ids.len() == 1 {
                    let destino = Punto2::nuevo(c.x + (a1 - a0) / 2.0 * f, c.y + (b1 - b0) / 2.0 * f);
                    transformar::escalar(x, pixpin_geom::Tirador::SuresteEsquina, destino, true, true);
                } else {
                    // Varios: cada uno escala y se aleja del centro comun.
                    let mio = Punto2::nuevo((a0 + a1) / 2.0, (b0 + b1) / 2.0);
                    let destino = Punto2::nuevo(mio.x + (a1 - a0) / 2.0 * f, mio.y + (b1 - b0) / 2.0 * f);
                    transformar::escalar(x, pixpin_geom::Tirador::SuresteEsquina, destino, true, true);
                    x.mover((mio.x - c.x) * (f - 1.0), (mio.y - c.y) * (f - 1.0));
                }
            });
            Ok(hecho(escena, &format!("escalado x{f}"), ids, ctx))
        }
        "girar" => {
            let g = e.numero("grados").ok_or("para girar hace falta «grados»")? as f32;
            // En pantalla la y va hacia abajo: antihorario es restar.
            let delta = -g.to_radians();
            editar(escena, &mut |x| transformar::girar(x, centro, delta));
            Ok(hecho(escena, &format!("girado {g} grados"), ids, ctx))
        }
        "duplicar" => {
            let copias = portapapeles::copiar(escena, &sel);
            let (dx, dy) = match punto(e, "a", ctx) {
                Some(p) => (p.x - x0, p.y - y0),
                None => (
                    e.numero("dx").map(|v| v as f32).unwrap_or(portapapeles::DESPLAZAMIENTO * 2.0),
                    e.numero("dy").map(|v| v as f32).unwrap_or(portapapeles::DESPLAZAMIENTO * 2.0),
                ),
            };
            let nuevos = portapapeles::pegar(escena, &copias, dx, dy);
            Ok(hecho(escena, "duplicado", nuevos, ctx))
        }
        "borrar" => {
            let texto = format!("borrados {}: {}", ids.len(), listar(escena, &ids, ctx.marco).replace('\n', "; "));
            escena.abrir_paso();
            for id in &ids {
                escena.borrar_apuntando(*id);
            }
            escena.cerrar_paso();
            Ok(Hecho { texto: format!("hecho: {texto}"), ids: Vec::new() })
        }
        "agrupar" => {
            if ids.len() < 2 {
                return Err("para agrupar hacen falta dos o mas".into());
            }
            organizar::agrupar(escena, &sel).ok_or("no se pudieron agrupar")?;
            Ok(hecho(escena, "agrupados", ids, ctx))
        }
        "desagrupar" => {
            organizar::desagrupar(escena, &sel);
            Ok(hecho(escena, "desagrupados", ids, ctx))
        }
        "al_frente" => {
            organizar::al_frente(escena, &sel);
            Ok(hecho(escena, "al frente", ids, ctx))
        }
        "al_fondo" => {
            organizar::al_fondo(escena, &sel);
            Ok(hecho(escena, "al fondo", ids, ctx))
        }
        "voltear_horizontal" | "voltear_vertical" => {
            let eje = if accion == "voltear_horizontal" {
                transformar::EjeVolteo::Horizontal
            } else {
                transformar::EjeVolteo::Vertical
            };
            let mut copias: Vec<Elemento> = ids.iter().filter_map(|id| escena.buscar(*id).cloned()).collect();
            transformar::voltear(&mut copias, eje);
            escena.abrir_paso();
            for c in copias {
                escena.apuntar_edicion(c.id);
                if let Some(x) = escena.buscar_mut(c.id) {
                    *x = c;
                    x.tocar();
                }
            }
            escena.cerrar_paso();
            transformar::atar_tras_voltear(escena, &ids);
            Ok(hecho(escena, "volteado", ids, ctx))
        }
        otra => Err(format!("accion desconocida «{otra}»")),
    }
}

// ---------------------------------------------------------------------------
// Estilo

fn nivel_de_grosor(g: f32) -> estilo::NivelGrosor {
    if g < 1.5 {
        estilo::NivelGrosor::Fino
    } else if g < 3.0 {
        estilo::NivelGrosor::Medio
    } else {
        estilo::NivelGrosor::Grueso
    }
}

fn cambiar_estilo(escena: &mut Escena, e: &Edicion, ctx: &Contexto) -> Result<Hecho, String> {
    use estilo::CambioEstilo as C;
    let ids = varios(escena, e.texto("que").unwrap_or_default(), ctx)?;
    let sel = seleccion_de(&ids);
    let mut cambios: Vec<C> = Vec::new();
    if let Some(c) = e.texto("color") {
        cambios.push(C::Trazo(color_rgba(c)?));
    }
    if let Some(r) = e.texto("relleno") {
        let n = crate::acciones::normalizar(r);
        cambios.push(C::Relleno(if ["ninguno", "nada", "sin relleno", "transparente", "quitar"].contains(&n.as_str()) {
            None
        } else {
            Some(color_rgba(r)?)
        }));
    }
    if let Some(t) = e.texto("trazo") {
        cambios.push(C::Estilo(trazo_de(t)));
    }
    if let Some(o) = e.numero("opacidad") {
        cambios.push(C::Opacidad((o as f32 / 100.0).clamp(0.05, 1.0)));
    }
    if let Some(m) = e.texto("material") {
        let m = if m == "grafito" { MaterialTinta::Cuadritos } else { MaterialTinta::desde_palabra(m).unwrap_or_default() };
        cambios.push(C::Material(m));
    }
    if let Some(d) = e.texto("dureza").and_then(Dureza::desde_rotulo) {
        cambios.push(C::Dureza(d));
    }
    let grosor = e.numero("grosor").map(|g| (g as f32).clamp(0.5, 40.0));
    let tam = e.numero("tam_letra").map(|t| (t as f32).clamp(6.0, 200.0));
    if cambios.is_empty() && grosor.is_none() && tam.is_none() {
        return Err("no se pidio ningun cambio: di «color», «relleno», «grosor», «trazo», «opacidad», «material» o «dureza»".into());
    }
    if let Some(t) = tam {
        let textos: Vec<u64> = ids.iter().copied().filter(|id| escena.buscar(*id).is_some_and(|x| matches!(x.figura, Figura::Texto { .. }))).collect();
        if textos.is_empty() && cambios.is_empty() && grosor.is_none() {
            return Err("«tam_letra» solo cambia textos y en lo elegido no hay ninguno".into());
        }
        escena.abrir_paso();
        for id in &textos {
            escena.apuntar_edicion(*id);
            if let Some(x) = escena.buscar_mut(*id) {
                // El tamano y la caja que le toca, por el mismo camino que
                // el editor al salir de la caja (texto rico o llano).
                let fuente = x.extras.rico.clone().or_else(|| match &x.figura {
                    Figura::Texto { texto, .. } => Some(texto.clone()),
                    _ => None,
                });
                if let Figura::Texto { tam, .. } = &mut x.figura {
                    *tam = t;
                }
                if let Some(f) = fuente {
                    pixpin_motor2d::texto_rico::poner_fuente(x, &f);
                }
                x.tocar();
            }
        }
        escena.cerrar_paso();
    }
    // Cada cambio, por la misma puerta que el panel lateral.
    for c in cambios {
        estilo::aplicar_a(escena, &sel, c);
    }
    if let Some(g) = grosor {
        // El panel solo tiene tres niveles; un numero exacto se pone tal cual
        // (fino/medio/grueso si cae en uno de ellos).
        let nivel = nivel_de_grosor(g);
        if (nivel.de_forma() - g).abs() < 0.01 {
            estilo::aplicar_a(escena, &sel, C::Grosor(nivel));
        } else {
            escena.abrir_paso();
            for id in &ids {
                escena.apuntar_edicion(*id);
                if let Some(x) = escena.buscar_mut(*id) {
                    x.grosor = match x.figura {
                        Figura::Lapiz { .. } => g / pixpin_motor2d::tinta::FACTOR_VARIABLE,
                        _ => g,
                    };
                    x.tocar();
                }
            }
            escena.cerrar_paso();
        }
    }
    Ok(hecho(escena, "estilo cambiado", ids, ctx))
}

// ---------------------------------------------------------------------------
// Trazos a mano: el Gesto del motor

/// **Traza con el gesto del editor**: pulsar en el primero, mover por los
/// demas y soltar en el ultimo. Devuelve el id de lo que nacio.
pub(crate) fn con_gesto(escena: &mut Escena, mano: Mano, puntos: &[Punto2], ajustar: impl FnOnce(&mut Gesto)) -> Option<u64> {
    let (primero, ultimo) = (*puntos.first()?, *puntos.last()?);
    let mut g = Gesto::default();
    // El iman no: el modelo da coordenadas exactas y no un cursor a pulso.
    g.enganche.activo = false;
    g.tomar_herramienta(mano);
    ajustar(&mut g);
    let antes = escena.siguiente_id;
    let raton = |p: Punto2| EventoGesto::Mover {
        p,
        shift: false,
        alt: false,
        presion: None,
    };
    g.evento(
        EventoGesto::Pulsar {
            p: primero,
            shift: false,
            alt: false,
            presion: None,
        },
        escena,
        1.0,
    );
    for p in &puntos[1..] {
        g.evento(raton(*p), escena, 1.0);
    }
    g.evento(EventoGesto::Soltar { p: ultimo }, escena, 1.0);
    escena
        .elementos
        .iter()
        .rev()
        .find(|x| x.id >= antes && !x.borrado)
        .map(|x| x.id)
}

/// **El recorrido de una mano**: los puntos dados, repartidos como los da un
/// raton —mas juntos al arrancar y al frenar, mas separados en medio— y con
/// un temblor de menos de un pixel. La presion la simula la tinta de la
/// velocidad (como con el raton), asi que el trazo engorda y adelgaza solo.
pub(crate) fn a_mano(puntos: &[Punto2], semilla: u32) -> Vec<Punto2> {
    if puntos.len() < 2 {
        return puntos.to_vec();
    }
    let largos: Vec<f32> = puntos.windows(2).map(|p| p[1].distancia(p[0])).collect();
    let total: f32 = largos.iter().sum();
    if total < 1.0 {
        return puntos.to_vec();
    }
    // Unos 4 px por aviso de raton, entre 8 y 400 avisos.
    let n = ((total / 4.0) as usize).clamp(8, 400);
    let mut azar = semilla.max(1);
    let mut temblor = || {
        azar ^= azar << 13;
        azar ^= azar >> 17;
        azar ^= azar << 5;
        (azar % 1000) as f32 / 1000.0 - 0.5
    };
    let mut salida = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let u = i as f32 / n as f32;
        // Arranca y frena despacio (seno), como una mano.
        let s = total * (1.0 - (std::f32::consts::PI * u).cos()) / 2.0;
        let mut recorrido = 0.0;
        let mut p = *puntos.last().unwrap();
        for (k, l) in largos.iter().enumerate() {
            if s <= recorrido + l || k == largos.len() - 1 {
                let t = if *l > 0.0 { ((s - recorrido) / l).clamp(0.0, 1.0) } else { 0.0 };
                p = puntos[k].hacia(puntos[k + 1], t);
                break;
            }
            recorrido += l;
        }
        if i > 0 && i < n {
            p = Punto2::nuevo(p.x + temblor() * 0.8, p.y + temblor() * 0.8);
        }
        salida.push(p);
    }
    salida
}

fn trazar(escena: &mut Escena, e: &Edicion, ctx: &Contexto) -> Result<Hecho, String> {
    let herramienta = e.texto("herramienta").unwrap_or("lapiz");
    let mano = match herramienta {
        "grafito" => Mano::Grafito,
        "resaltador" => Mano::Resaltador,
        _ => Mano::Lapiz,
    };
    let (camino, alto_de_lo_tapado): (Vec<Punto2>, Option<f32>) = match (e.texto("sobre"), e.numeros("puntos")) {
        (Some(sobre), _) => {
            let id = uno(escena, sobre, ctx, None)?;
            let objetivo = escena.buscar(id).cloned().ok_or("no esta")?;
            let como = e.texto("como").unwrap_or(if mano == Mano::Resaltador { "resaltar" } else { "subrayar" });
            camino_sobre(&objetivo, como)?
        }
        (None, Some(v)) if v.len() >= 4 => (puntos_de(&v, ctx), None),
        _ => return Err("hace falta «puntos» (al menos dos) o «sobre» (algo del lienzo)".into()),
    };
    let color = match e.texto("color") {
        Some(c) => color_rgba(c)?,
        None if mano == Mano::Resaltador => ColorRgba::opaco(1.0, 0.83, 0.23),
        None => ColorRgba::opaco(0.118, 0.118, 0.118),
    };
    let grosor = e.numero("grosor").map(|g| (g as f32).clamp(0.5, 40.0));
    let dureza = e.texto("dureza").and_then(Dureza::desde_rotulo);
    let recorrido = a_mano(&camino, (escena.siguiente_id as u32).wrapping_mul(2_654_435_761).wrapping_add(7));
    let id = con_gesto(escena, mano, &recorrido, |g| {
        g.estilo.trazo = color;
        if let Some(d) = dureza {
            g.estilo.dureza = d;
        }
        if let Some(px) = grosor.filter(|_| mano != Mano::Resaltador) {
            // El lapiz pinta a `grosor * FACTOR_VARIABLE` de ancho.
            g.grosor_tinta = (px / pixpin_motor2d::tinta::FACTOR_VARIABLE).clamp(0.2, 8.0);
        }
    })
    .ok_or("el trazo no salio")?;
    // El resaltador tapa la altura de lo que resalta (o el grosor pedido).
    if mano == Mano::Resaltador {
        let ancho = grosor.or(alto_de_lo_tapado);
        if let Some(ancho) = ancho {
            let g = ancho / pixpin_motor2d::tinta::ancho_del_resaltador(1.0);
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            if let Some(x) = escena.buscar_mut(id) {
                x.grosor = g.clamp(0.5, 20.0);
                x.tocar();
            }
            escena.cerrar_paso();
        }
    }
    Ok(hecho(escena, &format!("trazo con {herramienta}"), vec![id], ctx))
}

/// Por donde pasa la mano para subrayar, resaltar, rodear, tachar o repasar
/// algo (y la altura que tapa, para el resaltador).
fn camino_sobre(o: &Elemento, como: &str) -> Result<(Vec<Punto2>, Option<f32>), String> {
    let (x0, y0, x1, y1) = o.caja();
    let (w, h) = (x1 - x0, y1 - y0);
    let p = Punto2::nuevo;
    Ok(match como {
        "subrayar" => (vec![p(x0, y1 + 3.0), p(x1, y1 + 3.0)], None),
        "tachar" => (vec![p(x0 - 2.0, (y0 + y1) / 2.0), p(x1 + 2.0, (y0 + y1) / 2.0)], None),
        "resaltar" => (
            vec![p(x0 - 2.0, (y0 + y1) / 2.0), p(x1 + 2.0, (y0 + y1) / 2.0)],
            Some((h * 0.9).max(8.0)),
        ),
        "rodear" => {
            // Un ovalo a mano que se pasa un poco de donde empezo.
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let (rx, ry) = (w / 2.0 * 1.25 + 8.0, h / 2.0 * 1.35 + 8.0);
            let pasos = 48;
            let v = (0..=pasos + 5)
                .map(|i| {
                    let t = std::f32::consts::PI * (0.8 + 2.0 * i as f32 / pasos as f32);
                    let r = 1.0 + 0.03 * (i as f32 / pasos as f32);
                    p(cx + rx * r * t.cos(), cy + ry * r * t.sin())
                })
                .collect();
            (v, None)
        }
        "repasar" => {
            let segs = pixpin_motor2d::segmentos_de(o, pixpin_motor2d::PASO_PERIMETRO);
            if segs.is_empty() {
                return Err("eso no tiene contorno que repasar".into());
            }
            let mut v = vec![segs[0].0];
            for (a, b) in segs {
                if v.last().is_some_and(|u: &Punto2| u.distancia(a) > 0.5) {
                    v.push(a);
                }
                v.push(b);
            }
            v.truncate(400);
            (v, None)
        }
        otro => return Err(format!("«como» desconocido: {otro}")),
    })
}

// ---------------------------------------------------------------------------
// Geometria precisa

/// Pone a una figura recien trazada el color, grosor y trazo pedidos.
fn rematar_estilo(escena: &mut Escena, id: u64, e: &Edicion) -> Result<(), String> {
    let color = e.texto("color").map(color_rgba).transpose()?;
    let grosor = e.numero("grosor").map(|g| (g as f32).clamp(0.5, 40.0));
    let trazo = e.texto("trazo").map(trazo_de);
    escena.abrir_paso();
    escena.apuntar_edicion(id);
    if let Some(x) = escena.buscar_mut(id) {
        if let Some(c) = color {
            x.trazo = c;
        }
        if let Some(g) = grosor {
            x.grosor = g;
        }
        if let Some(t) = trazo {
            x.estilo = t;
        }
        x.tocar();
    }
    escena.cerrar_paso();
    Ok(())
}

/// Una recta exacta de `a` a `b`, trazada con la herramienta Linea.
fn recta(escena: &mut Escena, a: Punto2, b: Punto2) -> Result<u64, String> {
    if a.distancia(b) < 0.5 {
        return Err("los dos puntos de la recta son el mismo".into());
    }
    let id = con_gesto(escena, Mano::Linea, &[a, a.hacia(b, 0.5), b], |_| {}).ok_or("la recta no salio")?;
    // El gesto ya la deja ahi; se fijan los extremos exactos por si algo
    // (una restriccion de angulo) los movio.
    escena.abrir_paso();
    escena.apuntar_edicion(id);
    if let Some(x) = escena.buscar_mut(id)
        && let Figura::Linea { puntos } = &mut x.figura
    {
        *puntos = vec![a, b];
        x.x = a.x.min(b.x);
        x.y = a.y.min(b.y);
        x.ancho = (a.x - b.x).abs();
        x.alto = (a.y - b.y).abs();
        x.tocar();
    }
    escena.cerrar_paso();
    Ok(id)
}

/// Un ovalo exacto en `centro` con `radio`, trazado con la herramienta
/// Elipse (o Arco).
fn circulo(escena: &mut Escena, mano: Mano, centro: Punto2, radio: f32) -> Result<u64, String> {
    if !(radio.is_finite() && radio > 0.5) {
        return Err("el radio tiene que ser mayor que 0".into());
    }
    let a = Punto2::nuevo(centro.x - radio, centro.y - radio);
    let b = Punto2::nuevo(centro.x + radio, centro.y + radio);
    let id = con_gesto(escena, mano, &[a, a.hacia(b, 0.5), b], |_| {}).ok_or("el circulo no salio")?;
    escena.abrir_paso();
    escena.apuntar_edicion(id);
    if let Some(x) = escena.buscar_mut(id) {
        x.x = a.x;
        x.y = a.y;
        x.ancho = 2.0 * radio;
        x.alto = 2.0 * radio;
        x.tocar();
    }
    escena.cerrar_paso();
    Ok(id)
}

/// El centro del circulo que pasa por tres puntos (`None` si estan en
/// linea).
pub(crate) fn circuncentro(a: Punto2, b: Punto2, c: Punto2) -> Option<Punto2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-3 {
        return None;
    }
    let (a2, b2, c2) = (a.x * a.x + a.y * a.y, b.x * b.x + b.y * b.y, c.x * c.x + c.y * c.y);
    Some(Punto2::nuevo(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    ))
}

/// Los extremos de una recta que ya esta (su primer y ultimo punto).
fn extremos(escena: &Escena, id: u64) -> Result<(Punto2, Punto2), String> {
    let x = escena.buscar(id).ok_or("no esta")?;
    let p = x.puntos().filter(|p| p.len() >= 2).ok_or("eso no es una recta")?;
    Ok((p[0], p[p.len() - 1]))
}

fn es_recta(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Linea { .. } | Figura::Flecha { .. } | Figura::Cota { .. })
}

fn geometria(escena: &mut Escena, e: &Edicion, ctx: &Contexto) -> Result<Hecho, String> {
    let figura = e.texto("figura").unwrap_or("recta");
    let id = match figura {
        "recta" => {
            let (a, b) = match (e.numeros("puntos"), punto(e, "desde", ctx)) {
                (Some(v), _) if v.len() >= 4 => (ctx.a_escena(v[0], v[1]), ctx.a_escena(v[2], v[3])),
                (_, Some(a)) => {
                    // Sin angulo, horizontal hacia la derecha.
                    let ang = e.numero("angulo").unwrap_or(0.0) as f32;
                    let l = e.numero("longitud").ok_or("con «desde» hace falta «longitud»")? as f32;
                    let t = ang.to_radians();
                    // 90 grados es hacia arriba: en pantalla, y negativa.
                    (a, Punto2::nuevo(a.x + l * t.cos(), a.y - l * t.sin()))
                }
                _ => return Err("la recta necesita «puntos» [x1,y1,x2,y2] o «desde» con «angulo» y «longitud»".into()),
            };
            recta(escena, a, b)?
        }
        "circulo" => {
            let (c, r) = match (punto(e, "centro", ctx), e.numero("radio"), e.numeros("puntos")) {
                (Some(c), Some(r), _) => (c, r as f32),
                (_, _, Some(v)) if v.len() >= 6 => {
                    let p = puntos_de(&v, ctx);
                    let c = circuncentro(p[0], p[1], p[2]).ok_or("los tres puntos estan en linea: no hay circulo que pase por ellos")?;
                    (c, c.distancia(p[0]))
                }
                _ => return Err("el circulo necesita «centro» y «radio», o tres «puntos»".into()),
            };
            circulo(escena, Mano::Elipse, c, r)?
        }
        "arco" if e.numeros("puntos").is_some_and(|v| v.len() >= 6) => {
            let v = puntos_de(&e.numeros("puntos").unwrap_or_default(), ctx);
            let a = pixpin_motor2d::geometria_exacta::arco_por_tres_puntos(v[0], v[1], v[2])
                .ok_or("los tres puntos estan en linea: no hay arco que pase por ellos")?;
            escena.abrir_paso();
            let id = escena.anadir(a);
            escena.cerrar_paso();
            id
        }
        "arco" => {
            let c = punto(e, "centro", ctx).ok_or("el arco necesita «centro»")?;
            let r = e.numero("radio").ok_or("el arco necesita «radio»")? as f32;
            let desde = e.numero("inicio").unwrap_or(0.0) as f32;
            let hasta = e.numero("fin").unwrap_or(180.0) as f32;
            if (hasta - desde).abs() < 0.5 {
                return Err("el arco no abarca nada: «inicio» y «fin» son iguales".into());
            }
            let id = circulo(escena, Mano::Arco, c, r)?;
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            if let Some(x) = escena.buscar_mut(id) {
                // En pantalla la y baja: el antihorario del modelo es el
                // horario del ovalo.
                x.figura = Figura::Arco {
                    inicio: -desde.to_radians(),
                    barrido: Some(barrido_de(e, desde as f64, hasta as f64)),
                };
                x.tocar();
            }
            escena.cerrar_paso();
            id
        }
        "perpendicular" | "paralela" => {
            let base = uno(escena, e.texto("a").ok_or("hace falta «a»: la recta que ya esta")?, ctx, Some(("una recta", es_recta)))?;
            let (a, b) = extremos(escena, base)?;
            let d = b.restar(a).unitario();
            // La normal «a la izquierda» de la recta, mirando de a a b.
            let n = Punto2::nuevo(d.y, -d.x);
            if figura == "perpendicular" {
                let por = punto(e, "por", ctx).unwrap_or_else(|| a.hacia(b, 0.5));
                let pie = a.sumar(d.escalar(por.restar(a).producto(d)));
                let l = e.numero("longitud").map(|l| l as f32);
                if pie.distancia(por) > 0.5 && l.is_none() {
                    // Desde el punto hasta la recta: la que cae.
                    recta(escena, por, pie)?
                } else {
                    let l = l.unwrap_or(a.distancia(b) / 2.0).max(1.0);
                    recta(escena, pie.sumar(n.escalar(l / 2.0)), pie.restar(n.escalar(l / 2.0)))?
                }
            } else {
                let desplazar = match (punto(e, "por", ctx), e.numero("distancia")) {
                    (Some(por), _) => n.escalar(por.restar(a).producto(n)),
                    (None, Some(dist)) => n.escalar(dist as f32),
                    _ => return Err("la paralela necesita «por» [x, y] o «distancia»".into()),
                };
                recta(escena, a.sumar(desplazar), b.sumar(desplazar))?
            }
        }
        "semicircunferencia" | "cuadrante" | "sector" | "segmento" | "marca_angulo" | "angulo_recto" | "punto" => {
            let nuevo = de_lamina(escena, figura, e, ctx)?;
            escena.abrir_paso();
            let id = escena.anadir(nuevo);
            escena.cerrar_paso();
            id
        }
        otra => return Err(format!("figura desconocida «{otra}»")),
    };
    rematar_estilo(escena, id, e)?;
    rematar_lamina(escena, id, e)?;
    Ok(hecho(escena, &format!("{figura} trazada"), vec![id], ctx))
}

/// Grados de matematicas (0 a la derecha, 90 arriba, antihorario) a radianes
/// de pantalla (la y baja: el signo cambia).
fn a_pantalla(grados: f64) -> f32 {
    -(grados as f32).to_radians()
}

/// El barrido en pantalla de `inicio` a `fin` (grados de matematicas): con
/// «sentido», la vuelta que toca; sin el, la resta tal cual (negativa =
/// horario), como siempre.
fn barrido_de(e: &Edicion, inicio: f64, fin: f64) -> f32 {
    let d = match e.texto("sentido") {
        Some("horario") => -(inicio - fin).rem_euclid(360.0),
        Some(_) => (fin - inicio).rem_euclid(360.0),
        None => fin - inicio,
    };
    let d = if d == 0.0 && e.texto("sentido").is_some() { 360.0 } else { d };
    -(d as f32).to_radians()
}

/// **Las figuras de lamina** con las piezas del motor
/// (`pixpin_motor2d::geometria_exacta`), las mismas que la herramienta Arco y
/// la marca de angulo del lienzo.
fn de_lamina(escena: &Escena, figura: &str, e: &Edicion, ctx: &Contexto) -> Result<Elemento, String> {
    use pixpin_motor2d::geometria_exacta as ge;
    let puntos = e.numeros("puntos").map(|v| puntos_de(&v, ctx)).unwrap_or_default();
    Ok(match figura {
        "semicircunferencia" => {
            let (Some(&a), Some(&b)) = (puntos.first(), puntos.get(1)) else {
                return Err("la semicircunferencia necesita «puntos» [x1, y1, x2, y2] del diametro".into());
            };
            if a.distancia(b) < 1.0 {
                return Err("el diametro no puede medir 0".into());
            }
            let medio = |s: &Elemento| {
                let (c, r) = ge::centro_y_radio(s).unwrap_or((a, 0.0));
                let Figura::Arco { inicio, barrido: Some(b) } = s.figura else { return c };
                ge::en_la_circunferencia(c, r, inicio + b / 2.0)
            };
            let (h, ah) = (ge::semicircunferencia(a, b, true), ge::semicircunferencia(a, b, false));
            match punto(e, "lado", ctx) {
                Some(l) if medio(&ah).distancia(l) < medio(&h).distancia(l) => ah,
                Some(_) => h,
                // Sin decir, la de arriba (la y de pantalla mas pequena).
                None if medio(&ah).y < medio(&h).y => ah,
                None => h,
            }
        }
        "cuadrante" | "sector" | "segmento" => {
            let c = punto(e, "centro", ctx).ok_or_else(|| format!("el {figura} necesita «centro»"))?;
            let cierre = if figura == "segmento" { ge::CierreArco::Segmento } else { ge::CierreArco::Sector };
            if puntos.len() >= 2 {
                // Por las puntas de sus dos radios: el angulo menor entre ellos.
                let (inicio, barrido) = ge::angulo_menor(c, puntos[0], puntos[1]);
                ge::arco_cerrado(c, c.distancia(puntos[0]), inicio, barrido, cierre)
            } else {
                let r = e.numero("radio").ok_or_else(|| format!("el {figura} necesita «radio» (o «puntos»)"))? as f32;
                if r <= 0.5 {
                    return Err("el radio tiene que ser mayor que 0".into());
                }
                let inicio = e.numero("inicio").unwrap_or(0.0);
                let barrido = if figura == "cuadrante" {
                    let s: f32 = if e.texto("sentido") == Some("horario") { -1.0 } else { 1.0 };
                    -s * std::f32::consts::FRAC_PI_2
                } else {
                    let fin = e.numero("fin").ok_or_else(|| format!("el {figura} necesita «fin»"))?;
                    barrido_de(e, inicio, fin)
                };
                ge::arco_cerrado(c, r, a_pantalla(inicio), barrido, cierre)
            }
        }
        "marca_angulo" | "angulo_recto" => {
            // **Entre dos rectas que ya estan** (18-oct): el vertice y los
            // lados exactos, sin coordenadas a ojo.
            let (v, p1, p2) = match e.texto("entre").filter(|t| !t.trim().is_empty()) {
                Some(entre) => cruce(escena, entre, ctx, punto(e, "cerca", ctx))?,
                None => {
                    let v = punto(e, "vertice", ctx).ok_or("la marca necesita «entre» (dos rectas: «#3 #5») o «vertice» [x, y]")?;
                    if puntos.len() < 2 {
                        return Err("la marca necesita «puntos» [x1, y1, x2, y2]: un punto de cada lado".into());
                    }
                    (v, puntos[0], puntos[1])
                }
            };
            if p1.distancia(v) < 0.5 || p2.distancia(v) < 0.5 {
                return Err("los puntos de los lados no pueden ser el vertice".into());
            }
            let medido = {
                let (a, b) = (p1.restar(v), p2.restar(v));
                ((a.x * b.x + a.y * b.y) / (a.longitud() * b.longitud()).max(1e-6)).clamp(-1.0, 1.0).acos().to_degrees()
            };
            let valor_pedido = e.texto("valor").map(str::trim).filter(|s| !s.is_empty());
            // Un angulo de 90° entre rectas, sin valor propio: su cuadradito.
            let recto = figura == "angulo_recto" || (e.texto("entre").is_some() && (medido - 90.0).abs() < 1.5 && matches!(valor_pedido, None | Some("griega")));
            let valor = match valor_pedido {
                Some("griega") => Some(ge::siguiente_letra_de_angulo(&ge::letras_de_angulo_usadas(&escena.elementos, None))),
                Some("medida") => Some(format!("{}°", medido.round())),
                Some(v) => Some(v.to_string()),
                None => None,
            };
            let mut m = if recto {
                ge::marca_de_angulo_recto(v, p1, p2, e.numero("radio").unwrap_or(14.0) as f32)
            } else {
                let arcos = e.numero("arcos").unwrap_or(1.0).clamp(1.0, 3.0) as u8;
                ge::marca_de_angulo(v, p1, p2, e.numero("radio").unwrap_or(ge::RADIO_MARCA as f64) as f32, arcos)
            };
            if let Some(marca) = m.extras.marca_angulo.as_mut()
                && !recto
            {
                marca.valor = valor;
            }
            m
        }
        "punto" => {
            // En el cruce de dos rectas (exacto), o donde se diga.
            let p = match e.texto("entre").filter(|t| !t.trim().is_empty()) {
                Some(entre) => Some(cruce(escena, entre, ctx, None)?.0),
                None => punto(e, "centro", ctx).or_else(|| puntos.first().copied()),
            }
            .ok_or("el punto necesita «entre» (dos rectas), «puntos» [x, y] o «centro»")?;
            let angulo = match e.texto("letra_hacia") {
                Some(lado) => {
                    let g: f32 = match lado {
                        "derecha" => 0.0,
                        "abajo_derecha" => 45.0,
                        "abajo" => 90.0,
                        "abajo_izquierda" => 135.0,
                        "izquierda" => 180.0,
                        "arriba_izquierda" => 225.0,
                        "arriba" => 270.0,
                        _ => 315.0,
                    };
                    g.to_radians()
                }
                None => pixpin_motor2d::puntos_etiquetados::angulo_libre(p, &escena.elementos, 40.0),
            };
            // «siguiente» (o sin letra con «entre»): la siguiente libre, A, B, C...
            let letra = match e.texto("letra").map(str::trim) {
                Some("siguiente") => pixpin_motor2d::puntos_etiquetados::siguiente_etiqueta(&pixpin_motor2d::puntos_etiquetados::etiquetas_usadas(&escena.elementos), Default::default()),
                None | Some("") if e.texto("entre").is_some() => pixpin_motor2d::puntos_etiquetados::siguiente_etiqueta(&pixpin_motor2d::puntos_etiquetados::etiquetas_usadas(&escena.elementos), Default::default()),
                Some(l) => l.to_string(),
                None => String::new(),
            };
            ge::punto_con_letra(p, &letra, angulo)
        }
        otra => return Err(format!("figura desconocida «{otra}»")),
    })
}

/// **El cruce de dos rectas que ya estan** (18-oct, el usuario: «que los
/// agentes usen la herramienta de puntos en las intersecciones y que el
/// angulo sea mas preciso y facil de usar»): `entre` las nombra («#3 #5»,
/// «la recta roja y la azul»). Devuelve el vertice exacto y un punto de cada
/// lado: el extremo de cada recta que queda del lado de `cerca` (un punto
/// dentro del angulo que se quiere; sin el, el extremo mas lejano). Si los
/// trazos no llegan a tocarse, sus prolongaciones.
fn cruce(escena: &Escena, entre: &str, ctx: &Contexto, cerca: Option<Punto2>) -> Result<(Punto2, Punto2, Punto2), String> {
    use pixpin_motor2d::perimetros::{interseccion, segmentos_de};
    let r = referencia::resolver(escena, entre, ctx, None)?;
    if r.ids.len() != 2 {
        return Err(format!("«entre» tiene que nombrar dos rectas, como «#3 #5» (encaja con {})", r.ids.len()));
    }
    let tramos = |id: u64| -> Result<Vec<(Punto2, Punto2)>, String> {
        let el = escena.buscar(id).ok_or_else(|| format!("no esta el #{id}"))?;
        let s = segmentos_de(el, 2.0);
        if s.is_empty() { Err(format!("el #{id} no tiene lados")) } else { Ok(s) }
    };
    let (sa, sb) = (tramos(r.ids[0])?, tramos(r.ids[1])?);
    let mut encontrado: Option<(Punto2, (Punto2, Punto2), (Punto2, Punto2))> = None;
    'fuera: for a in &sa {
        for b in &sb {
            if let Some(p) = interseccion(a.0, a.1, b.0, b.1) {
                encontrado = Some((p, *a, *b));
                break 'fuera;
            }
        }
    }
    // Sin cruce dentro de los trazos: las rectas prolongadas (de un extremo
    // al otro de cada trazo).
    let (v, la, lb) = match encontrado {
        Some(x) => x,
        None => {
            let (a1, a2) = (sa[0].0, sa[sa.len() - 1].1);
            let (b1, b2) = (sb[0].0, sb[sb.len() - 1].1);
            let (d1, d2) = (a2.restar(a1), b2.restar(b1));
            let den = d1.x * d2.y - d1.y * d2.x;
            if den.abs() < 1e-4 {
                return Err("esas dos rectas son paralelas: no se cruzan".into());
            }
            let t = ((b1.x - a1.x) * d2.y - (b1.y - a1.y) * d2.x) / den;
            (Punto2::nuevo(a1.x + d1.x * t, a1.y + d1.y * t), (a1, a2), (b1, b2))
        }
    };
    let lado = |(q1, q2): (Punto2, Punto2)| -> Punto2 {
        // Los dos sentidos de la recta desde el vertice.
        let (d1, d2) = (q1.restar(v), q2.restar(v));
        let elegir = match cerca {
            Some(c) => {
                let h = c.restar(v);
                (d1.x * h.x + d1.y * h.y) >= (d2.x * h.x + d2.y * h.y)
            }
            None => d1.longitud() >= d2.longitud(),
        };
        let d = if elegir { d1 } else { d2 };
        // (Un extremo que es el propio vertice: el otro sentido.)
        if d.longitud() < 1.0 { v.restar(if elegir { d2 } else { d1 }) } else { v.sumar(d) }
    };
    Ok((v, lado(la), lado(lb)))
}

/// Lo de lamina que se pone despues del estilo: relleno (translucido en la
/// marca), degradado, puntas de flecha y tamano de letra.
fn rematar_lamina(escena: &mut Escena, id: u64, e: &Edicion) -> Result<(), String> {
    let relleno = e
        .texto("relleno")
        .filter(|t| !matches!(*t, "ninguno" | "no" | ""))
        .map(color_rgba)
        .transpose()?;
    let hasta = e.texto("relleno_hasta").map(color_rgba).transpose()?;
    let degradado = e.texto("degradado").and_then(pixpin_motor2d::geometria_exacta::TipoDegradado::desde_palabra);
    let flecha = e.texto("flecha").filter(|f| *f != "ninguna");
    let tam = e.numero("tam_letra").map(|t| (t as f32).clamp(6.0, 120.0));
    let es_marca = escena.buscar(id).is_some_and(|x| x.extras.marca_angulo.is_some());
    if relleno.is_none() && degradado.is_none() && flecha.is_none() && tam.is_none() && !es_marca {
        return Ok(());
    }
    escena.abrir_paso();
    escena.apuntar_edicion(id);
    if let Some(x) = escena.buscar_mut(id) {
        if es_marca {
            // La marca lleva su relleno translucido, del color pedido o del
            // de su trazo.
            let base = relleno.unwrap_or(x.trazo);
            x.relleno = Some(ColorRgba { a: base.a.min(0.35), ..base });
        } else if let Some(r) = relleno {
            x.relleno = Some(r);
            x.estilo_relleno = pixpin_motor2d::relleno::EstiloRelleno::Solido;
        }
        if let Some(t) = degradado {
            let desde = x.relleno.unwrap_or(x.trazo);
            let mut d = pixpin_motor2d::geometria_exacta::Degradado::desde_relleno(t, desde);
            if let Some(h) = hasta {
                d.hasta = h;
            }
            x.extras.degradado = Some(d);
        }
        if let Some(f) = flecha {
            let tipo = if e.texto("punta") == Some("abierta") {
                pixpin_motor2d::formas::TipoPunta::Flecha
            } else {
                pixpin_motor2d::formas::TipoPunta::Triangulo
            };
            if let Figura::Linea { puntos } = &x.figura {
                let puntos = puntos.clone();
                x.figura = Figura::Flecha {
                    puntos,
                    punta_inicio: if matches!(f, "inicio" | "ambas") { tipo } else { Default::default() },
                    punta_fin: if matches!(f, "fin" | "ambas") { tipo } else { Default::default() },
                    codos: false,
                };
            }
        }
        if let Some(t) = tam {
            x.extras.tam_letra = Some(t);
        }
        x.tocar();
    }
    escena.cerrar_paso();
    Ok(())
}

// ---------------------------------------------------------------------------
// Cortar y extender (trim / extend de CAD): `recorte` del motor

fn ids_de(escena: &Escena, que: Option<&str>, ctx: &Contexto, menos: u64) -> Result<Option<Vec<u64>>, String> {
    let Some(q) = que else { return Ok(None) };
    let r = referencia::resolver(escena, q, ctx, None)?;
    let ids: Vec<u64> = r.ids.into_iter().filter(|id| *id != menos).collect();
    if ids.is_empty() {
        return Err(format!("«{q}» es la misma linea: hace falta otra figura"));
    }
    Ok(Some(ids))
}

fn cortar(escena: &mut Escena, e: &Edicion, ctx: &Contexto) -> Result<Hecho, String> {
    let que = e.texto("que").unwrap_or_default();
    let id = uno(escena, que, ctx, Some(("una linea, flecha, trazo, circulo o rectangulo", recorte::se_recorta)))?;
    let contra = ids_de(escena, e.texto("con"), ctx, id)?;
    let victima = escena.buscar(id).cloned().ok_or("no esta")?;
    let otros: Vec<Elemento> = escena
        .visibles()
        .filter(|x| x.id != id && contra.as_ref().is_none_or(|c| c.contains(&x.id)))
        .cloned()
        .collect();
    let p = match punto(e, "punto", ctx) {
        Some(p) => p,
        None => {
            if !recorte::es_lineal(&victima) {
                return Err("para cortar un circulo o un rectangulo di el «punto» del trozo que se quita".into());
            }
            let cortes = recorte::cortes_de(&victima, &otros);
            if cortes.is_empty() {
                return Err(format!(
                    "#{id} no se cruza con {}: no hay donde cortarla (usa extender_linea si se queda corta)",
                    match &contra {
                        Some(c) => c.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(", "),
                        None => "nada".into(),
                    }
                ));
            }
            let total = recorte::largo_del_recorrido(&victima);
            let (primero, ultimo) = (cortes[0], cortes[cortes.len() - 1]);
            let inicio = primero;
            let fin = total - ultimo;
            let d = match e.texto("quitar").unwrap_or("corto") {
                "inicio" => primero / 2.0,
                "fin" => (ultimo + total) / 2.0,
                "medio" if cortes.len() >= 2 => (cortes[0] + cortes[1]) / 2.0,
                "medio" => return Err("para quitar el medio hacen falta dos cruces".into()),
                "largo" if inicio >= fin => primero / 2.0,
                "largo" => (ultimo + total) / 2.0,
                _ if inicio <= fin => primero / 2.0,
                _ => (ultimo + total) / 2.0,
            };
            recorte::punto_del_recorrido(&victima, d).ok_or("no tiene recorrido")?
        }
    };
    let antes = escena.siguiente_id;
    if !recorte::recortar_en_escena(escena, id, p, contra.as_deref()) {
        return Err(format!("no hay nada que cortar de #{id} ahi"));
    }
    let mut ids = vec![id];
    // La mitad que nace al partir una raya en dos.
    ids.extend(escena.elementos.iter().filter(|x| x.id >= antes).map(|x| x.id));
    ids.retain(|i| escena.buscar(*i).is_some_and(|x| !x.borrado));
    Ok(hecho(escena, "cortada", ids, ctx))
}

fn extender(escena: &mut Escena, e: &Edicion, ctx: &Contexto) -> Result<Hecho, String> {
    let que = e.texto("que").unwrap_or_default();
    let id = uno(escena, que, ctx, Some(("una linea o flecha", recorte::es_lineal)))?;
    let hasta = ids_de(escena, e.texto("hasta"), ctx, id)?;
    let (a, b) = {
        let x = escena.buscar(id).ok_or("no esta")?;
        let p = x.puntos().filter(|p| p.len() >= 2).ok_or("no tiene puntas")?;
        (p[0], p[p.len() - 1])
    };
    let intentar = |escena: &mut Escena, punta: Punto2| {
        recorte::extender_en_escena(escena, id, punta, hasta.as_deref(), recorte::ALCANCE_EXTENDER)
    };
    let hecho_ = match e.texto("extremo") {
        Some("inicio") => intentar(escena, a),
        Some(_) => intentar(escena, b),
        // Sin decir cual: primero el fin y, si no topa con nada, el inicio.
        None => intentar(escena, b) || intentar(escena, a),
    };
    if !hecho_ {
        return Err(format!(
            "#{id} no topa con {} en esa direccion: no hay hasta donde extenderla",
            match &hasta {
                Some(h) => h.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(", "),
                None => "nada".into(),
            }
        ));
    }
    Ok(hecho(escena, "extendida", vec![id], ctx))
}

// ---------------------------------------------------------------------------
// Imagenes: pegar (del anfitrion) y recortar (`recorte_imagen` del motor)

fn imagen(escena: &mut Escena, e: &Edicion, ctx: &Contexto, an: &mut dyn Anfitrion) -> Result<Hecho, String> {
    match e.texto("accion").unwrap_or("pegar") {
        "pegar" => {
            let mut el = an.imagen_nueva(e.texto("imagen").unwrap_or_default())?;
            let (w0, h0) = (el.ancho.max(1.0), el.alto.max(1.0));
            let caja = e.numeros("caja").unwrap_or_default();
            let (x, y) = if caja.len() >= 2 {
                (caja[0] as f32 + ctx.marco.0, caja[1] as f32 + ctx.marco.1)
            } else {
                // A la derecha de lo que haya, arriba.
                match escena.caja() {
                    Some(c) if c.0.is_finite() => (c.2 + 40.0, c.1),
                    _ => (ctx.marco.0 + 40.0, ctx.marco.1 + 40.0),
                }
            };
            // Sin tamano: como mucho 400 de lado, sin agrandarla.
            let (w, h) = match (caja.get(2), caja.get(3)) {
                (Some(&w), Some(&h)) if w > 1.0 && h > 1.0 => (w as f32, h as f32),
                (Some(&w), _) if w > 1.0 => (w as f32, w as f32 * h0 / w0),
                _ => {
                    let k = (400.0 / w0.max(h0)).min(1.0);
                    (w0 * k, h0 * k)
                }
            };
            el.x = x;
            el.y = y;
            el.ancho = w;
            el.alto = h;
            escena.abrir_paso();
            let id = escena.anadir(el);
            escena.cerrar_paso();
            Ok(hecho(escena, "imagen pegada", vec![id], ctx))
        }
        "recortar" => {
            let es_imagen = |x: &Elemento| matches!(x.figura, Figura::Imagen { .. });
            let id = match e.texto("que") {
                Some(q) => uno(escena, q, ctx, Some(("una imagen", es_imagen)))?,
                None => {
                    let ultimas: Vec<u64> = ctx
                        .ultimos
                        .iter()
                        .copied()
                        .filter(|i| escena.buscar(*i).is_some_and(|x| es_imagen(x) && !x.borrado))
                        .collect();
                    match ultimas.last() {
                        Some(i) => *i,
                        None => uno(escena, "imagen", ctx, None)?,
                    }
                }
            };
            let conservar = match (e.numeros("conservar"), e.texto("quitar")) {
                (Some(v), _) if v.len() >= 4 => {
                    // Mas de 1: el modelo lo dio en porcentaje.
                    let k = if v.iter().take(4).any(|x| *x > 1.0) { 100.0 } else { 1.0 };
                    (v[0] as f32 / k, v[1] as f32 / k, v[2] as f32 / k, v[3] as f32 / k)
                }
                (_, Some(lado)) => {
                    let mut f = e.numero("cuanto").unwrap_or(0.5) as f32;
                    if f > 1.0 {
                        f /= 100.0;
                    }
                    let f = f.clamp(0.01, 0.99);
                    match lado {
                        "arriba" => (0.0, f, 1.0, 1.0 - f),
                        "abajo" => (0.0, 0.0, 1.0, 1.0 - f),
                        "izquierda" => (f, 0.0, 1.0 - f, 1.0),
                        _ => (0.0, 0.0, 1.0 - f, 1.0),
                    }
                }
                _ => return Err("para recortar di «conservar» [x, y, ancho, alto] en fracciones, o «quitar» un lado".into()),
            };
            let natural = match escena.buscar(id).map(|x| &x.figura) {
                Some(Figura::Imagen { id_objeto }) => an.tamano_de(*id_objeto).map(|(w, h)| (w as f32, h as f32)),
                _ => None,
            };
            if !recorte_imagen::recortar_imagen_en_escena(escena, id, conservar, natural) {
                return Err("ese recorte no deja nada: «conservar» va en fracciones de 0 a 1 y no puede ser vacio".into());
            }
            Ok(hecho(escena, "imagen recortada", vec![id], ctx))
        }
        otra => Err(format!("accion desconocida «{otra}»")),
    }
}
