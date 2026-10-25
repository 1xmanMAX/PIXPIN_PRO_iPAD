//! **Elegir lo que ya hay en el lienzo por como lo dice el modelo.**
//!
//! El modelo no ve la escena: ve el resumen (`#12 recta roja ...`) y habla
//! como el usuario. Aqui se entiende, en este orden:
//!
//! 1. **`#id`** (uno o varios): exactos. Un id que no existe es un error con
//!    la lista de lo que hay, no una suposicion.
//! 2. **«lo elegido»** (lo que el usuario tiene seleccionado) y **«lo
//!    ultimo»** (lo que creo o toco la llamada anterior; si no hay, lo de
//!    encima del todo). **«todo»**.
//! 3. **Una descripcion**: tipo («recta», «circulo», «texto», «foto»...),
//!    color («rojo»), texto entre comillas o «que dice ...», y un
//!    calificativo de sitio o tamano («de arriba», «de la izquierda», «la mas
//!    grande», «la primera»). «El titulo» es el texto de letra mas grande.
//!
//! **Singular y plural** deciden que pasa si encajan varios: «las rectas
//! rojas» son todas; «la recta roja» con dos rojas es ambigua y el modelo
//! recibe la lista corta para elegir por su `#id` (nunca se elige una al
//! azar: tocar la que no era es peor que preguntar).

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{Elemento, Figura};

use super::{Contexto, listar, nombre_de, nombre_de_color};
use crate::acciones::normalizar;

/// Lo que encajo con una referencia.
#[derive(Debug, Clone, PartialEq)]
pub struct Encontrado {
    pub ids: Vec<u64>,
    /// La referencia hablaba en plural («las rectas», «todos»).
    pub plural: bool,
}

/// Que tipos nombra cada palabra (singular sin la `s` final).
fn tipo_de_palabra(p: &str) -> Option<fn(&Elemento) -> bool> {
    let raiz = p.trim_end_matches('s');
    let raiz = raiz.strip_suffix("e").filter(|r| r.ends_with("l") || r.ends_with("n")).unwrap_or(raiz);
    Some(match raiz {
        "recta" | "linea" | "raya" | "segmento" => |e| matches!(e.figura, Figura::Linea { .. } | Figura::Cota { .. }),
        "flecha" => |e| matches!(e.figura, Figura::Flecha { .. }),
        "circulo" | "circunferencia" | "elipse" | "ovalo" => |e| matches!(e.figura, Figura::Elipse),
        "arco" => |e| matches!(e.figura, Figura::Arco { .. }),
        "rectangulo" | "cuadrado" | "caja" | "recuadro" => |e| matches!(e.figura, Figura::Rectangulo),
        "rombo" => |e| matches!(e.figura, Figura::Rombo),
        "texto" | "titulo" | "letrero" | "palabra" | "frase" | "etiqueta" | "rotulo" | "resultado" => {
            |e| matches!(e.figura, Figura::Texto { .. })
        }
        "imagen" | "image" | "foto" | "fotografia" | "captura" => |e| matches!(e.figura, Figura::Imagen { .. }),
        "trazo" | "lapiz" | "garabato" | "dibujo" => |e| matches!(e.figura, Figura::Lapiz { .. }),
        "grafito" => |e| matches!(e.figura, Figura::Lapiz { .. }) && e.material.es_grafito(),
        "resaltado" | "marcador" => |e| matches!(e.figura, Figura::Resaltador { .. }),
        "punto" => |e| matches!(e.figura, Figura::Punto { .. }),
        "triangulo" | "poligono" | "estrella" | "polilinea" => {
            |e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() > 3)
        }
        "cota" => |e| matches!(e.figura, Figura::Cota { .. }),
        _ => return None,
    })
}

/// Las palabras de color que se entienden (singular masculino).
fn color_de_palabra(p: &str) -> Option<&'static str> {
    let c = crate::herramientas::color(p)?;
    let rgb = u32::from_str_radix(c.0.trim_start_matches('#'), 16).ok()?;
    let col = pixpin_motor2d::ColorRgba::opaco(
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
    );
    Some(nombre_de_color(col))
}

/// Los calificativos de sitio y tamano: con varios que encajan, se queda con
/// el extremo.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Extremo {
    Arriba,
    Abajo,
    Izquierda,
    Derecha,
    Grande,
    Pequeno,
    Primero,
    Segundo,
}

fn extremo_de(p: &str) -> Option<Extremo> {
    Some(match p {
        "arriba" | "superior" | "alto" | "alta" => Extremo::Arriba,
        "abajo" | "inferior" | "bajo" | "baja" => Extremo::Abajo,
        "izquierda" | "izquierdo" => Extremo::Izquierda,
        "derecha" | "derecho" => Extremo::Derecha,
        "grande" | "mayor" | "larga" | "largo" | "gorda" | "gordo" => Extremo::Grande,
        "pequena" | "pequeno" | "menor" | "corta" | "corto" | "chica" | "chico" => Extremo::Pequeno,
        "primera" | "primero" | "primer" => Extremo::Primero,
        "segunda" | "segundo" => Extremo::Segundo,
        _ => return None,
    })
}

/// El texto citado: entre comillas de cualquier clase, o tras «que dice».
fn citado(q: &str) -> Option<String> {
    for (abre, cierra) in [('"', '"'), ('«', '»'), ('“', '”'), ('\'', '\'')] {
        if let Some(i) = q.find(abre) {
            let resto = &q[i + abre.len_utf8()..];
            if let Some(j) = resto.find(cierra) {
                let t = resto[..j].trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    for marca in ["que dice ", "que pone ", "texto:", "con el texto "] {
        if let Some(i) = q.find(marca) {
            let t = q[i + marca.len()..].trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn texto_de(e: &Elemento) -> Option<&str> {
    match &e.figura {
        Figura::Texto { texto, .. } => Some(texto),
        _ => None,
    }
}

/// Los ids `#12` (o `12` suelto) de una referencia.
fn ids_escritos(q: &str) -> Vec<u64> {
    let mut v = Vec::new();
    for t in q.split(|c: char| !(c.is_ascii_alphanumeric() || c == '#')) {
        let t = t.trim_start_matches('#');
        let t = t.strip_prefix('e').filter(|r| r.chars().all(|c| c.is_ascii_digit())).unwrap_or(t);
        if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) && t.len() < 12 {
            if let Ok(n) = t.parse() {
                v.push(n);
            }
        }
    }
    v
}

/// **Resuelve una referencia** contra la escena. `zona` (en la escena) deja
/// solo lo que cae dentro. `Err` lleva el motivo para el modelo, con la lista
/// corta de lo que hay.
pub fn resolver(
    escena: &Escena,
    que: &str,
    ctx: &Contexto,
    zona: Option<(f32, f32, f32, f32)>,
) -> Result<Encontrado, String> {
    let vivos: Vec<&Elemento> = escena.a_la_vista().filter(|e| !e.extras.referencia).collect();
    if vivos.is_empty() {
        return Err("el lienzo esta vacio: no hay nada que elegir".into());
    }
    let q = normalizar(que.trim());
    let en_zona = |e: &Elemento| {
        zona.is_none_or(|(x, y, w, h)| {
            let (x0, y0, x1, y1) = e.caja();
            x0 >= x - 1.0 && y0 >= y - 1.0 && x1 <= x + w + 1.0 && y1 <= y + h + 1.0
        })
    };
    let todo_lo_que_hay = || listar(escena, &vivos.iter().rev().take(12).map(|e| e.id).collect::<Vec<_>>(), ctx.marco);

    // 1. Los #id.
    let tiene_almohadilla = q.contains('#');
    let escritos = ids_escritos(&q);
    let solo_numeros = q
        .split(|c: char| c.is_whitespace() || c == ',' || c == 'y')
        .filter(|t| !t.is_empty())
        .all(|t| t.trim_start_matches('#').chars().all(|c| c.is_ascii_digit()));
    if !escritos.is_empty() && (tiene_almohadilla || solo_numeros) {
        let faltan: Vec<String> = escritos
            .iter()
            .filter(|id| !vivos.iter().any(|e| e.id == **id))
            .map(|id| format!("#{id}"))
            .collect();
        // Cuatro numeros que no son ids: una caja [x, y, ancho, alto] (un 2B
        // la copia del resumen en vez del #id). Lo que cae dentro.
        if !faltan.is_empty() && escritos.len() == 4 {
            let n: Vec<f32> = escritos.iter().map(|v| *v as f32).collect();
            let (x, y, w, h) = (n[0] + ctx.marco.0, n[1] + ctx.marco.1, n[2], n[3]);
            let margen = 0.1 * w.max(h) + 4.0;
            let dentro: Vec<u64> = vivos
                .iter()
                .filter(|e| {
                    let (x0, y0, x1, y1) = e.caja();
                    x0 >= x - margen && y0 >= y - margen && x1 <= x + w + margen && y1 <= y + h + margen
                })
                .map(|e| e.id)
                .collect();
            if !dentro.is_empty() {
                return Ok(Encontrado { ids: dentro, plural: false });
            }
        }
        if !faltan.is_empty() {
            return Err(format!(
                "no existe {} en el lienzo. Lo que hay:\n{}",
                faltan.join(", "),
                todo_lo_que_hay()
            ));
        }
        return Ok(Encontrado {
            plural: escritos.len() > 1,
            ids: escritos,
        });
    }

    let palabras: Vec<&str> = q.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
    let dice = |w: &str| palabras.contains(&w);

    // 2. Lo elegido, lo ultimo, todo.
    if dice("elegido") || dice("elegida") || dice("elegidos") || dice("seleccionado") || dice("seleccionada") || q.contains("seleccion") {
        let ids: Vec<u64> = ctx.elegidos.iter().copied().filter(|id| vivos.iter().any(|e| e.id == *id)).collect();
        if ids.is_empty() {
            return Err("el usuario no tiene nada elegido en el lienzo: nombralo por su #id o descripcion".into());
        }
        return Ok(Encontrado { plural: ids.len() > 1, ids });
    }
    let plural_ultimo = dice("ultimos") || dice("ultimas");
    if (dice("ultimo") || dice("ultima") || plural_ultimo) && palabras.iter().all(|p| tipo_de_palabra(p).is_none()) {
        let ids: Vec<u64> = ctx.ultimos.iter().copied().filter(|id| vivos.iter().any(|e| e.id == *id)).collect();
        if !ids.is_empty() {
            return Ok(Encontrado { plural: ids.len() > 1, ids });
        }
        let ultimo = vivos.last().map(|e| e.id).into_iter().collect();
        return Ok(Encontrado { ids: ultimo, plural: false });
    }
    let hay_tipo = palabras.iter().any(|p| tipo_de_palabra(p).is_some());
    let hay_color = palabras.iter().any(|p| color_de_palabra(p).is_some());
    let todos = dice("todo") || dice("todos") || dice("todas");
    if todos && !hay_tipo && !hay_color && citado(que).is_none() {
        let ids: Vec<u64> = vivos.iter().filter(|e| en_zona(e)).map(|e| e.id).collect();
        return Ok(Encontrado { ids, plural: true });
    }

    // 3. Una descripcion. Plural: «todos», o un tipo en plural («rectas»).
    let tipos: Vec<fn(&Elemento) -> bool> = palabras.iter().filter_map(|p| tipo_de_palabra(p)).collect();
    let plural = todos || palabras.iter().any(|p| p.len() > 3 && p.ends_with('s') && tipo_de_palabra(p).is_some());
    let colores: Vec<&str> = palabras.iter().filter_map(|p| color_de_palabra(p)).collect();
    let cita = citado(que).map(|c| normalizar(&c));
    let extremo = palabras.iter().rev().find_map(|p| extremo_de(p));
    let titulo = dice("titulo") || dice("encabezado");

    let mut candidatos: Vec<&Elemento> = vivos
        .iter()
        .copied()
        .filter(|e| en_zona(e))
        .filter(|e| tipos.is_empty() || tipos.iter().any(|t| t(e)))
        .filter(|e| colores.is_empty() || colores.iter().any(|c| *c == nombre_de_color(e.trazo) || e.relleno.is_some_and(|r| *c == nombre_de_color(r))))
        .filter(|e| cita.as_ref().is_none_or(|c| texto_de(e).is_some_and(|t| normalizar(t).contains(c.as_str()))))
        .collect();

    if tipos.is_empty() && colores.is_empty() && cita.is_none() && zona.is_none() && extremo.is_none() {
        return Err(format!(
            "no entiendo «{que}» como algo del lienzo: nombralo por su #id, tipo o color. Lo que hay:\n{}",
            todo_lo_que_hay()
        ));
    }
    if candidatos.is_empty() {
        return Err(format!("no hay nada que encaje con «{que}» en el lienzo. Lo que hay:\n{}", todo_lo_que_hay()));
    }

    // «El titulo»: el texto de letra mas grande (y, a igual letra, el de
    // mas arriba).
    if titulo && !plural && candidatos.len() > 1 {
        let tam = |e: &Elemento| match &e.figura {
            Figura::Texto { tam, .. } => *tam,
            _ => 0.0,
        };
        let mayor = candidatos.iter().map(|e| tam(e)).fold(0.0, f32::max);
        candidatos.retain(|e| tam(e) >= mayor - 0.5);
        candidatos.sort_by(|a, b| a.caja().1.partial_cmp(&b.caja().1).unwrap_or(std::cmp::Ordering::Equal));
        candidatos.truncate(1);
    }

    // «recta_2», «la recta 2»: un numero suelto junto a un tipo es su #id si
    // es de uno de ellos, o su numero de orden.
    if !tipos.is_empty() && candidatos.len() > 1 && !plural
        && let Some(n) = palabras.iter().find_map(|p| p.parse::<u64>().ok())
    {
        if let Some(e) = candidatos.iter().find(|e| e.id == n).copied() {
            candidatos = vec![e];
        } else if let Some(e) = candidatos.get((n as usize).wrapping_sub(1)).copied() {
            candidatos = vec![e];
        }
    }

    if let Some(x) = extremo.filter(|_| candidatos.len() > 1 && !plural) {
        let medida = |e: &Elemento| -> f32 {
            let (x0, y0, x1, y1) = e.caja();
            match x {
                Extremo::Arriba => (y0 + y1) / 2.0,
                Extremo::Abajo => -(y0 + y1) / 2.0,
                Extremo::Izquierda => (x0 + x1) / 2.0,
                Extremo::Derecha => -(x0 + x1) / 2.0,
                Extremo::Grande => -((x1 - x0).powi(2) + (y1 - y0).powi(2)),
                Extremo::Pequeno => (x1 - x0).powi(2) + (y1 - y0).powi(2),
                Extremo::Primero | Extremo::Segundo => 0.0,
            }
        };
        let elegido = match x {
            Extremo::Primero => candidatos.first().copied(),
            Extremo::Segundo => candidatos.get(1).copied(),
            _ => candidatos
                .iter()
                .copied()
                .min_by(|a, b| medida(a).partial_cmp(&medida(b)).unwrap_or(std::cmp::Ordering::Equal)),
        };
        candidatos = elegido.into_iter().collect();
    }

    Ok(Encontrado {
        ids: candidatos.iter().map(|e| e.id).collect(),
        plural,
    })
}

/// **Uno solo**: la referencia tiene que dar exactamente un elemento (que
/// cumpla `vale`, si se da). Si da varios, el error lleva la lista corta para
/// que el modelo elija por su `#id`.
pub fn uno(
    escena: &Escena,
    que: &str,
    ctx: &Contexto,
    vale: Option<(&str, fn(&Elemento) -> bool)>,
) -> Result<u64, String> {
    let mut e = resolver(escena, que, ctx, None)?;
    if let Some((_, f)) = vale {
        let ok: Vec<u64> = e.ids.iter().copied().filter(|id| escena.buscar(*id).is_some_and(f)).collect();
        if !ok.is_empty() {
            e.ids = ok;
        } else if let Some((que_tiene_que_ser, _)) = vale {
            let n = e.ids.first().and_then(|id| escena.buscar(*id)).map(nombre_de).unwrap_or("figura");
            return Err(format!("«{que}» es un {n}: aqui hace falta {que_tiene_que_ser}"));
        }
    }
    match e.ids.len() {
        1 => Ok(e.ids[0]),
        0 => Err(format!("no hay nada que encaje con «{que}»")),
        n => Err(format!(
            "«{que}» es ambiguo: encajan {n}. No elijas tu: pregunta al usuario cual (con esta lista), o usa el #id si ya lo dijo:\n{}",
            listar(escena, &e.ids, ctx.marco)
        )),
    }
}

/// **Uno o varios**: para lo que se puede hacer a muchos a la vez (mover,
/// color, borrar). Un singular que encaja con varios es ambiguo.
pub fn varios(escena: &Escena, que: &str, ctx: &Contexto) -> Result<Vec<u64>, String> {
    let e = resolver(escena, que, ctx, None)?;
    if e.ids.len() > 1 && !e.plural {
        return Err(format!(
            "«{que}» es ambiguo: encajan {}. No elijas tu: pregunta al usuario cual (con esta lista), o usa el #id si ya lo dijo (en plural si son todos):\n{}",
            e.ids.len(),
            listar(escena, &e.ids, ctx.marco)
        ));
    }
    if e.ids.is_empty() {
        return Err(format!("no hay nada que encaje con «{que}»"));
    }
    Ok(e.ids)
}
