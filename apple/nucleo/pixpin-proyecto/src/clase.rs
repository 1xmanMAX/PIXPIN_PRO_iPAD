//! **Estudiar una clase grabada** (25-oct-2026, el usuario: «una ventana
//! especial … donde se vea la grabacion del video con su transcripcion al
//! lado; al otro lado las capturas, los marcadores … y que se pueda hacer
//! scroll de las capturas y del texto, que una siga a la otra»).
//!
//! Lo puro de esa ventana, sin pintar nada, para probarlo en milisegundos y
//! para que el movil haga las mismas cuentas:
//!
//! - **Los marcadores** de una nota de voz: `resto["marcadores"]`, una lista
//!   `[{ms, texto}]` en orden. Es distinta de las banderitas del movil
//!   (`resto["marcas"]`, solo milisegundos): un marcador lleva su nota.
//!   Vacia, **la clave se quita**, como las banderitas: el resumen de
//!   sincronizar es un hash del JSON y asi una nota que nunca tuvo
//!   marcadores sigue siendo byte a byte la de antes.
//! - **Que toca en un milisegundo**: la linea del texto, la captura.
//! - **El scroll que se sigue**: de un punto de una lista a su milisegundo,
//!   y de un milisegundo al punto de la otra. Es lo que hace que el texto y
//!   las capturas se muevan juntos.

use serde_json::{Map, Value};

/// La clave de los marcadores en `resto`.
pub const MARCADORES: &str = "marcadores";

/// Lo que mide como mucho la nota de un marcador, en caracteres: es una
/// nota al margen, no un documento.
pub const NOTA_MAXIMA: usize = 200;

/// Un marcador puesto al estudiar: en que milisegundo del audio y que dice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marcador {
    pub ms: i64,
    pub texto: String,
}

/// Los marcadores de una nota (su `resto`), en orden. Lo que no se entiende
/// se salta sin llevarse lo demas.
pub fn marcadores_de(resto: &Map<String, Value>) -> Vec<Marcador> {
    let mut v: Vec<Marcador> = resto
        .get(MARCADORES)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    Some(Marcador {
                        ms: m.get("ms")?.as_i64()?.max(0),
                        texto: m
                            .get("texto")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|m| m.ms);
    v
}

fn escribir(resto: &mut Map<String, Value>, v: &[Marcador]) {
    if v.is_empty() {
        resto.remove(MARCADORES);
        return;
    }
    resto.insert(
        MARCADORES.into(),
        Value::Array(
            v.iter()
                .map(|m| serde_json::json!({ "ms": m.ms, "texto": m.texto }))
                .collect(),
        ),
    );
}

/// Pone un marcador en `ms` con `texto` (recortado y sin saltos de linea).
/// Si ya habia uno en ese mismo milisegundo, se le cambia la nota: dos
/// marcadores en el mismo sitio no se distinguen en la barra.
pub fn poner_marcador(resto: &mut Map<String, Value>, ms: i64, texto: &str) {
    let ms = ms.max(0);
    let texto: String = texto
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(NOTA_MAXIMA)
        .collect();
    let mut v = marcadores_de(resto);
    match v.iter_mut().find(|m| m.ms == ms) {
        Some(m) => m.texto = texto,
        None => v.push(Marcador { ms, texto }),
    }
    v.sort_by_key(|m| m.ms);
    escribir(resto, &v);
}

/// Quita el marcador de `ms`. Devuelve si habia alguno.
pub fn quitar_marcador(resto: &mut Map<String, Value>, ms: i64) -> bool {
    let mut v = marcadores_de(resto);
    let antes = v.len();
    v.retain(|m| m.ms != ms);
    let cambio = v.len() != antes;
    if cambio {
        escribir(resto, &v);
    }
    cambio
}

// --- Que toca en un milisegundo --------------------------------------------

/// El ultimo de `tiempos` (en orden) que ya paso en `ms`. Antes del primero,
/// el primero: en una lista de capturas algo hay que ensenar. `None` si la
/// lista esta vacia.
pub fn ultimo_antes(tiempos: &[i64], ms: i64) -> Option<usize> {
    if tiempos.is_empty() {
        return None;
    }
    Some(tiempos.iter().rposition(|t| *t <= ms).unwrap_or(0))
}

/// La linea del texto que suena en `ms`: la ultima con minuto que ya paso.
/// `None` antes de la primera o si ninguna lleva minuto (resaltar una al
/// azar seria mentir).
pub fn linea_en(lineas: &[Option<i64>], ms: i64) -> Option<usize> {
    lineas
        .iter()
        .rposition(|t| t.is_some_and(|t| (0..=ms).contains(&t)))
}

/// Una fila de la columna del texto: una linea de la transcripcion o un
/// marcador, por su sitio en su lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fila {
    Linea(usize),
    Marcador(usize),
}

/// **El texto con los marcadores metidos en su minuto.** Las lineas
/// conservan su orden; cada marcador va delante de la primera linea con
/// minuto posterior a el. Una linea sin minuto no mueve a nadie.
pub fn mezclar(lineas: &[Option<i64>], marcadores: &[i64]) -> Vec<Fila> {
    let mut v = Vec::with_capacity(lineas.len() + marcadores.len());
    let mut m = 0;
    for (i, t) in lineas.iter().enumerate() {
        if let Some(t) = t {
            while m < marcadores.len() && marcadores[m] < *t {
                v.push(Fila::Marcador(m));
                m += 1;
            }
        }
        v.push(Fila::Linea(i));
    }
    v.extend((m..marcadores.len()).map(Fila::Marcador));
    v
}

/// El milisegundo de cada fila para seguir el scroll: el suyo, o el de la
/// anterior si no lo tiene (0 al principio). Nunca baja: una lista que
/// retrocede en el tiempo no se puede seguir.
pub fn tiempos_de_filas(filas: &[Fila], lineas: &[Option<i64>], marcadores: &[i64]) -> Vec<i64> {
    let mut ultimo = 0i64;
    filas
        .iter()
        .map(|f| {
            let t = match f {
                Fila::Linea(i) => lineas.get(*i).copied().flatten(),
                Fila::Marcador(j) => marcadores.get(*j).copied(),
            };
            ultimo = t.unwrap_or(ultimo).max(ultimo);
            ultimo
        })
        .collect()
}

// --- El scroll que se sigue ------------------------------------------------

/// Lo que mide una lista de filas una debajo de otra.
pub fn alto_total(altos: &[f32]) -> f32 {
    altos.iter().sum()
}

/// Lo mas abajo que se puede bajar una lista en una vista de `alto_vista`.
pub fn tope(altos: &[f32], alto_vista: f32) -> f32 {
    (alto_total(altos) - alto_vista).max(0.0)
}

/// **El milisegundo de un punto `y` de una lista** (en coordenadas de la
/// lista, sin scroll). Dentro de una fila se reparte hasta el milisegundo
/// de la siguiente: asi una fila larga avanza poco a poco y la otra lista
/// se mueve suave, no a saltos. `None` si la lista esta vacia.
pub fn ms_en(tiempos: &[i64], altos: &[f32], y: f32) -> Option<i64> {
    let n = tiempos.len().min(altos.len());
    if n == 0 {
        return None;
    }
    let mut arriba = 0.0f32;
    for i in 0..n {
        let alto = altos[i].max(0.0);
        if y < arriba + alto || i + 1 == n {
            let t0 = tiempos[i];
            let t1 = if i + 1 < n { tiempos[i + 1] } else { t0 };
            let f = if alto > 0.0 {
                ((y - arriba) / alto).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return Some(t0 + ((t1 - t0) as f32 * f).round() as i64);
        }
        arriba += alto;
    }
    None
}

/// **El punto `y` de una lista donde cae `ms`** (lo contrario de
/// [`ms_en`]). Con `suave`, dentro de la fila que toca segun lo que falta
/// para la siguiente; sin el, en lo alto de la fila. `None` si esta vacia.
pub fn y_de(tiempos: &[i64], altos: &[f32], ms: i64, suave: bool) -> Option<f32> {
    let n = tiempos.len().min(altos.len());
    if n == 0 {
        return None;
    }
    let i = ultimo_antes(&tiempos[..n], ms)?;
    let arriba: f32 = altos[..i].iter().map(|a| a.max(0.0)).sum();
    if !suave || i + 1 >= n {
        return Some(arriba);
    }
    let (t0, t1) = (tiempos[i], tiempos[i + 1]);
    if t1 <= t0 || ms <= t0 {
        return Some(arriba);
    }
    let f = ((ms - t0) as f32 / (t1 - t0) as f32).clamp(0.0, 1.0);
    Some(arriba + altos[i].max(0.0) * f)
}

/// Donde se lee una lista que sigue al tiempo: a un tercio de su alto. Lo
/// que viene despues es lo siguiente que se va a oir, y se ve.
pub const ANCLA: f32 = 0.3;

/// **El scroll de una lista para que `ms` quede en su ancla**, sin salirse.
pub fn scroll_para(tiempos: &[i64], altos: &[f32], alto_vista: f32, ms: i64, suave: bool) -> f32 {
    match y_de(tiempos, altos, ms, suave) {
        Some(y) => (y - alto_vista * ANCLA).clamp(0.0, tope(altos, alto_vista)),
        None => 0.0,
    }
}

/// **El milisegundo que se esta leyendo** en una lista con `scroll`: el de
/// su ancla. Al bajar del todo, el ancla baja con la vista hasta el final:
/// si no, las ultimas filas no se podrian «leer» nunca y la otra lista no
/// llegaria a su final.
pub fn ms_leido(tiempos: &[i64], altos: &[f32], alto_vista: f32, scroll: f32) -> Option<i64> {
    let tope = tope(altos, alto_vista);
    let ancla = if tope > 0.0 && scroll >= tope - 0.5 {
        alto_vista
    } else {
        alto_vista * ANCLA
    };
    ms_en(tiempos, altos, scroll + ancla)
}

/// **El scroll de la otra lista** cuando el usuario mueve una: la otra pone
/// en su ancla el milisegundo que la movida tiene en la suya.
#[allow(clippy::too_many_arguments)]
pub fn seguir(
    de_tiempos: &[i64],
    de_altos: &[f32],
    de_vista: f32,
    de_scroll: f32,
    a_tiempos: &[i64],
    a_altos: &[f32],
    a_vista: f32,
) -> Option<f32> {
    let ms = ms_leido(de_tiempos, de_altos, de_vista, de_scroll)?;
    if a_tiempos.is_empty() {
        return None;
    }
    // Al final de la que se mueve, la otra tambien al final.
    if de_scroll >= tope(de_altos, de_vista) - 0.5 && tope(de_altos, de_vista) > 0.0 {
        return Some(tope(a_altos, a_vista));
    }
    Some(scroll_para(a_tiempos, a_altos, a_vista, ms, true))
}

/// Acerca `actual` a `destino` un trozo cada fotograma (`k` de 0 a 1), para
/// que la lista se deslice en vez de saltar. Ya cerca, llega.
pub fn acercar(actual: f32, destino: f32, k: f32) -> f32 {
    let d = destino - actual;
    if d.abs() < 1.0 {
        destino
    } else {
        actual + d * k.clamp(0.0, 1.0)
    }
}

// --- La barra del tiempo ---------------------------------------------------

/// El milisegundo de un punto de la barra: `x` desde su izquierda.
pub fn ms_de_la_barra(x: f32, ancho: f32, duracion_ms: i64) -> i64 {
    if ancho <= 0.0 || duracion_ms <= 0 {
        return 0;
    }
    ((x / ancho).clamp(0.0, 1.0) * duracion_ms as f32).round() as i64
}

/// La x de un milisegundo en la barra.
pub fn x_en_la_barra(ms: i64, ancho: f32, duracion_ms: i64) -> f32 {
    if duracion_ms <= 0 {
        return 0.0;
    }
    (ms as f32 / duracion_ms as f32).clamp(0.0, 1.0) * ancho
}

/// **Un clic en la barra cerca de una senal se queda en la senal**: las
/// capturas y los marcadores son a donde se quiere ir, y acertar con el
/// raton en un palito de dos pixeles es imposible. `tolerancia` en pixeles.
pub fn pegar_a_senal(
    x: f32,
    ancho: f32,
    duracion_ms: i64,
    senales: &[i64],
    tolerancia: f32,
) -> i64 {
    let ms = ms_de_la_barra(x, ancho, duracion_ms);
    senales
        .iter()
        .map(|s| (*s, (x_en_la_barra(*s, ancho, duracion_ms) - x).abs()))
        .filter(|(_, d)| *d <= tolerancia)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(ms, |(s, _)| s)
}

/// **El milisegundo bajo el raton en la barra**, desde el raton en
/// pixeles de PANTALLA (`raton_x`) y la esquina de la ventana DE VERDAD
/// (`origen_x`, la que Windows dice ahora). `barra_x` y `barra_ancho` van en
/// pixeles de la ventana, ya multiplicados por la escala del monitor.
///
/// POR QUE el origen va aparte (25-oct-2026, el usuario: «el clic esta
/// desplazado a la derecha, solo en esta linea de tiempo»): la ventana
/// restaba al raton la esquina con la que se CREO, no la que tenia. Movida
/// o estirada por fuera (AltSnap, Win+flecha, otro monitor con otro DPI) la
/// diferencia caia entera en todos los clics; en los botones no se notaba,
/// en una barra de seis pixeles de alto y un tirador de doce, si.
pub fn ms_del_raton(
    raton_x: i32,
    origen_x: i32,
    barra_x: f32,
    barra_ancho: f32,
    duracion_ms: i64,
) -> i64 {
    ms_de_la_barra(
        (raton_x - origen_x) as f32 - barra_x,
        barra_ancho,
        duracion_ms,
    )
}

/// Lo que se ensena como posicion mientras se arrastra la barra: donde esta
/// el raton, no donde dice el video. El video, al buscar, cae en el
/// fotograma clave mas cercano, y en una grabacion de pantalla (que solo
/// guarda fotogramas cuando algo cambia) dos claves pueden estar a medio
/// video: el tirador saltaba lejos del raton.
pub fn posicion_a_la_vista(medio_ms: i64, arrastrando: Option<i64>) -> i64 {
    arrastrando.unwrap_or(medio_ms)
}

// --- Las capturas de la nota -----------------------------------------------

/// La clave de las capturas de una clase en `resto`: `[{ms, id}]`.
pub const CAPTURAS: &str = "capturas";

/// Las capturas `(ms, id)` de una nota (su `resto`), en orden.
pub fn capturas_de(resto: &Map<String, Value>) -> Vec<(i64, String)> {
    let mut v: Vec<(i64, String)> = resto
        .get(CAPTURAS)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| Some((c.get("ms")?.as_i64()?, c.get("id")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|(ms, _)| *ms);
    v
}

/// **Anade una captura a la clase** (la de un fotograma del video anotado
/// al estudiar): `id` del mensaje de la imagen en `ms`. Un id que ya estaba
/// no se repite; lo demas que lleve cada captura se respeta.
pub fn poner_captura(resto: &mut Map<String, Value>, ms: i64, id: &str) {
    let mut lista: Vec<Value> = resto
        .get(CAPTURAS)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if lista
        .iter()
        .any(|c| c.get("id").and_then(Value::as_str) == Some(id))
    {
        return;
    }
    lista.push(serde_json::json!({ "ms": ms.max(0), "id": id }));
    lista.sort_by_key(|c| c.get("ms").and_then(Value::as_i64).unwrap_or(0));
    resto.insert(CAPTURAS.into(), Value::Array(lista));
}

/// Salta `delta` desde `ms` sin salirse de la nota.
pub fn saltar(ms: i64, delta: i64, duracion_ms: i64) -> i64 {
    let fin = if duracion_ms > 0 {
        duracion_ms
    } else {
        i64::MAX
    };
    ms.saturating_add(delta).clamp(0, fin)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_marcadores_van_y_vuelven_en_orden_y_vacios_quitan_la_clave() {
        let mut r = Map::new();
        poner_marcador(&mut r, 9_000, "  ojo   con\nesto ");
        poner_marcador(&mut r, 2_000, "definicion");
        assert_eq!(
            marcadores_de(&r),
            vec![
                Marcador {
                    ms: 2_000,
                    texto: "definicion".into()
                },
                Marcador {
                    ms: 9_000,
                    texto: "ojo con esto".into()
                },
            ]
        );
        // El mismo milisegundo cambia la nota, no anade otro.
        poner_marcador(&mut r, 2_000, "otra");
        assert_eq!(marcadores_de(&r).len(), 2);
        assert_eq!(marcadores_de(&r)[0].texto, "otra");
        assert!(quitar_marcador(&mut r, 2_000));
        assert!(quitar_marcador(&mut r, 9_000));
        // Caso negativo: sin marcadores la clave se va, y quitar uno que no
        // esta no cambia nada.
        assert!(!r.contains_key(MARCADORES));
        assert!(!quitar_marcador(&mut r, 1));
    }

    #[test]
    fn un_marcador_roto_se_salta_y_la_nota_se_corta() {
        let mut r = Map::new();
        r.insert(
            MARCADORES.into(),
            serde_json::json!([{ "ms": "x" }, { "ms": 5 }, { "ms": -3, "texto": "a" }]),
        );
        let v = marcadores_de(&r);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].ms, 0, "un milisegundo negativo es el principio");
        assert_eq!(v[1].texto, "", "sin texto es un marcador sin nota");
        poner_marcador(&mut r, 1, &"a".repeat(500));
        assert_eq!(
            marcadores_de(&r)
                .iter()
                .find(|m| m.ms == 1)
                .unwrap()
                .texto
                .len(),
            NOTA_MAXIMA
        );
    }

    #[test]
    fn la_linea_y_la_captura_del_momento() {
        let lineas = [Some(0), Some(5_000), None, Some(12_000)];
        assert_eq!(linea_en(&lineas, 4_999), Some(0));
        assert_eq!(linea_en(&lineas, 5_000), Some(1));
        assert_eq!(linea_en(&lineas, 11_000), Some(1));
        assert_eq!(linea_en(&lineas, 60_000), Some(3));
        // Caso negativo: sin minutos no se sabe por donde va.
        assert_eq!(linea_en(&[None, None], 1_000), None);
        let capturas = [2_000, 9_000];
        assert_eq!(ultimo_antes(&capturas, 0), Some(0));
        assert_eq!(ultimo_antes(&capturas, 9_000), Some(1));
        assert_eq!(ultimo_antes(&[], 9_000), None);
    }

    #[test]
    fn los_marcadores_entran_en_su_minuto_del_texto() {
        let lineas = [Some(0), Some(10_000), None, Some(20_000)];
        let marcadores = [3_000, 15_000, 99_000];
        let f = mezclar(&lineas, &marcadores);
        assert_eq!(
            f,
            vec![
                Fila::Linea(0),
                Fila::Marcador(0),
                Fila::Linea(1),
                Fila::Linea(2),
                Fila::Marcador(1),
                Fila::Linea(3),
                Fila::Marcador(2),
            ]
        );
        let t = tiempos_de_filas(&f, &lineas, &marcadores);
        assert_eq!(t, vec![0, 3_000, 10_000, 10_000, 15_000, 20_000, 99_000]);
        // Caso negativo: sin texto, solo los marcadores.
        assert_eq!(
            mezclar(&[], &[1]),
            vec![Fila::Marcador(0)],
            "sin lineas los marcadores siguen"
        );
    }

    #[test]
    fn de_un_punto_al_milisegundo_y_vuelta() {
        let tiempos = [0, 10_000, 20_000];
        let altos = [100.0, 50.0, 100.0];
        assert_eq!(ms_en(&tiempos, &altos, 0.0), Some(0));
        assert_eq!(ms_en(&tiempos, &altos, 50.0), Some(5_000));
        assert_eq!(ms_en(&tiempos, &altos, 125.0), Some(15_000));
        assert_eq!(ms_en(&tiempos, &altos, 9_999.0), Some(20_000));
        assert_eq!(y_de(&tiempos, &altos, 5_000, true), Some(50.0));
        assert_eq!(y_de(&tiempos, &altos, 5_000, false), Some(0.0));
        assert_eq!(y_de(&tiempos, &altos, 15_000, true), Some(125.0));
        // Ida y vuelta: el mismo punto.
        for y in [0.0, 30.0, 110.0, 149.0] {
            let ms = ms_en(&tiempos, &altos, y).unwrap();
            let y2 = y_de(&tiempos, &altos, ms, true).unwrap();
            assert!((y - y2).abs() < 1.0, "{y} -> {ms} -> {y2}");
        }
        // Caso negativo: una lista vacia no tiene tiempo.
        assert_eq!(ms_en(&[], &[], 10.0), None);
        assert_eq!(y_de(&[], &[], 10, true), None);
    }

    #[test]
    fn mover_una_lista_mueve_la_otra_al_mismo_minuto() {
        // El texto: 10 filas de 40 (una cada 6 s). Las capturas: 4 de 120.
        let t_texto: Vec<i64> = (0..10).map(|i| i * 6_000).collect();
        let a_texto = vec![40.0; 10];
        let t_fotos = vec![0, 18_000, 36_000, 54_000];
        let a_fotos = vec![120.0; 4];
        // El texto bajado hasta que su ancla (a 30 de 100) lee el minuto 18.
        let scroll = 3.0 * 40.0 - 100.0 * ANCLA;
        assert_eq!(
            ms_leido(&t_texto, &a_texto, 100.0, scroll),
            Some(18_000),
            "el ancla lee la fila 3"
        );
        let otra = seguir(&t_texto, &a_texto, 100.0, scroll, &t_fotos, &a_fotos, 200.0).unwrap();
        // La captura del minuto 18 en el ancla de las fotos.
        assert!((otra - (120.0 - 200.0 * ANCLA)).abs() < 1.0, "{otra}");
        // Al final de una, la otra al final.
        let fin = tope(&a_texto, 100.0);
        assert_eq!(
            seguir(&t_texto, &a_texto, 100.0, fin, &t_fotos, &a_fotos, 200.0),
            Some(tope(&a_fotos, 200.0))
        );
        // Caso negativo: sin nada que seguir, nada.
        assert_eq!(
            seguir(&t_texto, &a_texto, 100.0, 0.0, &[], &[], 200.0),
            None
        );
    }

    #[test]
    fn el_scroll_que_sigue_no_se_sale() {
        let t = [0, 1_000, 2_000];
        let a = [50.0, 50.0, 50.0];
        assert_eq!(scroll_para(&t, &a, 100.0, 0, false), 0.0);
        assert_eq!(scroll_para(&t, &a, 100.0, 2_000, false), 50.0, "el tope");
        assert_eq!(scroll_para(&t, &a, 400.0, 2_000, false), 0.0, "todo cabe");
        assert_eq!(acercar(0.0, 100.0, 0.5), 50.0);
        assert_eq!(acercar(99.5, 100.0, 0.5), 100.0);
    }

    #[test]
    fn el_clic_en_la_barra_cae_bajo_el_raton_a_cualquier_escala() {
        // La barra como la pone la ventana: a 16 logicos del borde y de
        // 600 de ancho, a 100 %, 150 % y 175 %.
        for e in [1.0f32, 1.5, 1.75] {
            let (x, ancho) = (16.0 * e, 600.0 * e);
            let origen = 300;
            // El raton en la mitad de la barra, en pixeles de pantalla.
            let mitad = origen + (x + ancho / 2.0) as i32;
            assert_eq!(ms_del_raton(mitad, origen, x, ancho, 60_000), 30_000, "{e}");
            // Ida y vuelta: donde se pinta el tirador es donde se pulso.
            let ms = ms_del_raton(mitad, origen, x, ancho, 27_000);
            let pintado = origen as f32 + x + x_en_la_barra(ms, ancho, 27_000);
            assert!((pintado - mitad as f32).abs() <= 1.0, "{e}: {pintado}");
            // Los extremos.
            assert_eq!(ms_del_raton(origen + x as i32, origen, x, ancho, 60_000), 0);
            assert_eq!(
                ms_del_raton(origen + (x + ancho) as i32 + 40, origen, x, ancho, 60_000),
                60_000
            );
        }
        // Caso negativo, el fallo de antes: restar la esquina con la que se
        // creo (300) a una ventana que AltSnap llevo a 240 corre el clic 60
        // pixeles; con la esquina de verdad, no.
        let (x, ancho) = (24.0, 900.0);
        let raton = 240 + 24 + 450;
        let viejo = ms_del_raton(raton, 300, x, ancho, 90_000);
        let bueno = ms_del_raton(raton, 240, x, ancho, 90_000);
        assert_eq!(bueno, 45_000);
        assert_ne!(viejo, bueno, "con la esquina vieja el clic se corre");
    }

    #[test]
    fn al_arrastrar_se_ve_donde_esta_el_raton() {
        assert_eq!(posicion_a_la_vista(27_000, Some(4_000)), 4_000);
        // Caso negativo: sin arrastrar manda el video.
        assert_eq!(posicion_a_la_vista(27_000, None), 27_000);
    }

    #[test]
    fn una_captura_nueva_entra_en_su_minuto_sin_repetirse() {
        let mut r = Map::new();
        r.insert(
            CAPTURAS.into(),
            serde_json::json!([{ "ms": 9_000, "id": "b" }, { "ms": 1_000, "id": "a" }]),
        );
        poner_captura(&mut r, 5_000, "c");
        assert_eq!(
            capturas_de(&r),
            vec![
                (1_000, "a".to_string()),
                (5_000, "c".to_string()),
                (9_000, "b".to_string())
            ]
        );
        // Caso negativo: el mismo id no entra dos veces.
        poner_captura(&mut r, 7_000, "c");
        assert_eq!(capturas_de(&r).len(), 3);
        // Una nota sin capturas estrena la lista.
        let mut vacia = Map::new();
        poner_captura(&mut vacia, -5, "x");
        assert_eq!(capturas_de(&vacia), vec![(0, "x".to_string())]);
    }

    #[test]
    fn la_barra_se_pega_a_las_capturas_y_marcadores() {
        assert_eq!(ms_de_la_barra(50.0, 100.0, 60_000), 30_000);
        assert_eq!(ms_de_la_barra(-5.0, 100.0, 60_000), 0);
        assert_eq!(ms_de_la_barra(500.0, 100.0, 60_000), 60_000);
        assert_eq!(x_en_la_barra(15_000, 200.0, 60_000), 50.0);
        // Cerca de una senal, la senal; lejos, donde se pulso.
        assert_eq!(pegar_a_senal(51.0, 100.0, 60_000, &[30_000], 3.0), 30_000);
        assert_eq!(pegar_a_senal(60.0, 100.0, 60_000, &[30_000], 3.0), 36_000);
        // Caso negativo: sin duracion, el principio.
        assert_eq!(ms_de_la_barra(50.0, 100.0, 0), 0);
        assert_eq!(saltar(2_000, -5_000, 60_000), 0);
        assert_eq!(saltar(58_000, 5_000, 60_000), 60_000);
        assert_eq!(saltar(58_000, 5_000, 0), 63_000, "sin duracion no hay fin");
    }
}
