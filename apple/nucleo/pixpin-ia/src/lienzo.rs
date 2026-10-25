//! **De una llamada validada a elementos del lienzo, en formato Excalidraw.**
//!
//! Excalidraw es el formato que ya comparten el lienzo del PC
//! (`pixpin_motor2d::excalidraw`) y el de Android: lo que dibuja el asistente
//! sale como cualquier otro trazo —se mueve, se cambia de color, se exporta y
//! viaja al movil— y este crate no necesita ver el motor del lienzo. La UI
//! hace `excalidraw::leer(&documento(..))` y anade los elementos a la escena
//! en un solo paso de deshacer.
//!
//! Las coordenadas del modelo son de un lienzo de 800x600 ([`ANCHO_LIENZO`]);
//! aqui solo se desplazan a `origen` (donde mira el usuario).
//!
//! Lo que el motor del lienzo ya sabe hacer —la grafica de una funcion, la
//! ecuacion compuesta, el instrumento, la tabla dibujada— no se rehace aqui:
//! se piden sus `Elemento` a `pixpin_motor2d` y se escriben con su mismo
//! escritor de Excalidraw ([`Fabrica::de_motor`]). Un solo componente.

use serde_json::{Value, json};

use crate::herramientas::{
    ALTO_LIENZO, ANCHO_LIENZO, EstiloFigura, Figura, Grafica, Llamada, TipoFigura, TipoGrafica,
};

/// Colores de las series, de la paleta de Excalidraw (la del lienzo).
const SERIES: [&str; 8] = [
    "#1971c2", "#e03131", "#2f9e44", "#f08c00", "#6741d9", "#0c8599", "#c2255c", "#868e96",
];

/// Fabrica de elementos con identificadores unicos y estables. La comparten
/// todas las herramientas que dibujan (`svg`, `cronograma`, `funciones`...).
pub(crate) struct Fabrica {
    pub(crate) semilla: u64,
    n: u64,
    pub(crate) ox: f64,
    pub(crate) oy: f64,
    pub(crate) salida: Vec<Value>,
    /// El grupo de lo que se esta fabricando (`groupIds`).
    pub(crate) grupo: Option<String>,
}

impl Fabrica {
    pub(crate) fn nueva(origen: (f64, f64), semilla: u64) -> Fabrica {
        Fabrica {
            semilla,
            n: 0,
            ox: origen.0,
            oy: origen.1,
            salida: Vec::new(),
            grupo: None,
        }
    }

    /// Un nombre de grupo unico en esta llamada.
    pub(crate) fn nombre_de_grupo(&mut self, que: &str) -> String {
        self.n += 1;
        format!("ia-{:x}-{que}-{}", self.semilla, self.n)
    }

    pub(crate) fn id(&mut self) -> String {
        self.n += 1;
        format!("ia-{:x}-{}", self.semilla, self.n)
    }

    pub(crate) fn base(&mut self, tipo: &str, [x, y, ancho, alto]: [f64; 4], trazo: &str, relleno: Option<&str>) -> Value {
        let id = self.id();
        json!({
            "id": id,
            "type": tipo,
            "x": self.ox + x,
            "y": self.oy + y,
            "width": ancho,
            "height": alto,
            "angle": 0,
            "strokeColor": trazo,
            "backgroundColor": relleno.unwrap_or("transparent"),
            "fillStyle": "solid",
            "strokeWidth": 2,
            "strokeStyle": "solid",
            "roughness": 1,
            "opacity": 100,
            "seed": self.semilla.wrapping_add(self.n) % 2_000_000_000,
            "version": 1,
            "isDeleted": false,
            "groupIds": self.grupo.iter().collect::<Vec<_>>(),
            "boundElements": null,
            "locked": false,
            "customData": {"origen": "pixpin-ia"},
        })
    }

    pub(crate) fn caja(&mut self, tipo: &str, caja: [f64; 4], trazo: &str, relleno: Option<&str>) {
        let e = self.base(tipo, caja, trazo, relleno);
        self.salida.push(e);
    }

    /// Una polilinea; `puntos` en coordenadas del modelo. Excalidraw los
    /// quiere relativos al primero.
    pub(crate) fn trazo(&mut self, tipo: &str, puntos: &[(f64, f64)], color: &str, relleno: Option<&str>) {
        let (x0, y0) = puntos[0];
        let (mut minx, mut miny, mut maxx, mut maxy) = (x0, y0, x0, y0);
        for &(x, y) in puntos {
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
        }
        let mut e = self.base(tipo, [x0, y0, maxx - minx, maxy - miny], color, relleno);
        e["points"] = Value::Array(puntos.iter().map(|&(x, y)| json!([x - x0, y - y0])).collect());
        if tipo == "arrow" {
            e["startArrowhead"] = Value::Null;
            e["endArrowhead"] = json!("arrow");
        } else {
            e["startArrowhead"] = Value::Null;
            e["endArrowhead"] = Value::Null;
        }
        self.salida.push(e);
    }

    /// Una mancha rellena con agujeros (`pixpin-region`, la del bote de
    /// pintura): el unico poligono que el lienzo del PC y el del movil
    /// rellenan. Sus puntos y los de los agujeros, relativos al primero.
    pub(crate) fn region(&mut self, contorno: &[(f64, f64)], huecos: &[Vec<(f64, f64)>], trazo: &str, relleno: &str) {
        if contorno.len() < 3 {
            return;
        }
        self.trazo("line", contorno, trazo, Some(relleno));
        let (x0, y0) = contorno[0];
        if let Some(e) = self.salida.last_mut() {
            e["type"] = json!("pixpin-region");
            if let Some(m) = e.as_object_mut() {
                m.remove("startArrowhead");
                m.remove("endArrowhead");
            }
            e["huecos"] = Value::Array(
                huecos
                    .iter()
                    .map(|h| Value::Array(h.iter().map(|&(x, y)| json!([x - x0, y - y0])).collect()))
                    .collect(),
            );
        }
    }

    pub(crate) fn texto(&mut self, x: f64, y: f64, texto: &str, tam: f64, color: &str, alinear: &str) {
        // **Texto rico** (`$...$` en linea, `\color{rojo}{X}`, `**negrita**`,
        // `\centro`): la fuente viaja en `textoRico` y `text` lleva su
        // version plana, la que lee quien no la entiende
        // (`pixpin_motor2d::texto_rico`, `docs/texto-rico-del-lienzo.md`).
        let limpio = pixpin_motor2d::texto_rico::sin_barras_dobles(texto);
        if pixpin_motor2d::texto_rico::tiene_marcas(&limpio) {
            use pixpin_motor2d::texto;
            let texto = limpio.as_str();
            let familia = texto::nombre_de_familia(Some(1));
            let (w, h) = texto_rico_medida(texto, tam as f32, familia);
            let plano = pixpin_motor2d::texto_rico::plano(texto);
            let (ancho, alto) = (w as f64, h as f64);
            let x = match alinear {
                "center" => x - ancho / 2.0,
                "right" => x - ancho,
                _ => x,
            };
            let mut e = self.base("text", [x, y, ancho, alto], color, None);
            e["text"] = json!(plano);
            e["originalText"] = json!(plano);
            e["textoRico"] = json!(texto);
            e["fontSize"] = json!(tam);
            e["fontFamily"] = json!(1);
            e["textAlign"] = json!(alinear);
            e["verticalAlign"] = json!("top");
            e["lineHeight"] = json!(1.25);
            e["containerId"] = Value::Null;
            self.salida.push(e);
            return;
        }
        let ancho = tam * 0.6 * texto.chars().count().max(1) as f64;
        let alto = tam * 1.25;
        let x = match alinear {
            "center" => x - ancho / 2.0,
            "right" => x - ancho,
            _ => x,
        };
        let mut e = self.base("text", [x, y, ancho, alto], color, None);
        e["text"] = json!(texto);
        e["originalText"] = json!(texto);
        e["fontSize"] = json!(tam);
        // 1 = Virgil, la letra a mano de Excalidraw (la del lienzo).
        e["fontFamily"] = json!(1);
        e["textAlign"] = json!(alinear);
        e["verticalAlign"] = json!("top");
        e["lineHeight"] = json!(1.25);
        e["containerId"] = Value::Null;
        self.salida.push(e);
    }

    /// **Un trazo a mano alzada** (`freedraw`), con la presion simulada como
    /// el lapiz del lienzo. Con material (grafito con su dureza, tiza...).
    pub(crate) fn lapiz(&mut self, puntos: &[(f64, f64)], color: &str, material: Option<&str>, dureza: Option<&str>) {
        if puntos.len() < 2 {
            return;
        }
        self.trazo("freedraw", puntos, color, None);
        if let Some(e) = self.salida.last_mut() {
            if let Some(m) = e.as_object_mut() {
                m.remove("startArrowhead");
                m.remove("endArrowhead");
            }
            e["pressures"] = json!([]);
            e["simulatePressure"] = json!(true);
            e["roughness"] = json!(0);
            if let Some(m) = material {
                e["material"] = json!(m);
            }
            if let Some(d) = dureza.filter(|d| *d != "HB") {
                e["dureza"] = json!(d);
            }
        }
    }

    /// Pone el estilo pedido en los elementos desde `desde`.
    pub(crate) fn estilo(&mut self, desde: usize, e: &EstiloFigura) {
        for v in self.salida.iter_mut().skip(desde) {
            if let Some(g) = e.grosor {
                v["strokeWidth"] = json!(g);
            }
            if let Some(t) = e.trazo {
                v["strokeStyle"] = json!(t);
            }
            if let Some(o) = e.opacidad {
                v["opacity"] = json!(o.round());
            }
            if let Some(r) = e.relleno {
                v["fillStyle"] = json!(r);
            }
            if let Some(r) = e.rugosidad {
                v["roughness"] = json!(r);
            }
            if v["type"] != "text" {
                if let Some(m) = e.material {
                    v["material"] = json!(m);
                }
                if let Some(d) = e.dureza.filter(|d| *d != "HB") {
                    v["dureza"] = json!(d);
                }
            }
        }
    }

    /// **Elementos del motor del lienzo**, escritos con su propio escritor de
    /// Excalidraw y traidos a esta llamada: corridos al origen, con ids de
    /// esta fabrica (los suyos son de una escena de usar y tirar), sus grupos
    /// renombrados y, si hay, el grupo de lo que se fabrica por fuera.
    pub(crate) fn de_motor(&mut self, elementos: &[pixpin_motor2d::Elemento]) {
        let mut grupos: std::collections::HashMap<String, String> = Default::default();
        for mut v in pixpin_motor2d::excalidraw::elementos_a_json(elementos) {
            v["id"] = json!(self.id());
            for k in ["x", "y"] {
                let d = if k == "x" { self.ox } else { self.oy };
                v[k] = json!(v[k].as_f64().unwrap_or(0.0) + d);
            }
            let mut g: Vec<Value> = v["groupIds"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .filter_map(Value::as_str)
                        .map(|g| {
                            let nuevo = match grupos.get(g) {
                                Some(n) => n.clone(),
                                None => {
                                    let n = format!("ia-{:x}-m{}-{g}", self.semilla, self.n);
                                    grupos.insert(g.to_string(), n.clone());
                                    n
                                }
                            };
                            json!(nuevo)
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Some(gr) = &self.grupo {
                g.push(json!(gr));
            }
            v["groupIds"] = Value::Array(g);
            v["customData"] = json!({"origen": "pixpin-ia"});
            self.salida.push(v);
        }
    }
}

/// Los elementos de Excalidraw de una llamada. El OCR no dibuja nada: su
/// resultado es texto (lo pone el chat).
///
/// `origen`: donde cae el (0,0) del modelo en el lienzo real. `semilla`:
/// cualquier numero distinto en cada llamada (la hora vale), para que los
/// identificadores no choquen con los de otra respuesta.
pub fn elementos(llamada: &Llamada, origen: (f64, f64), semilla: u64) -> Vec<Value> {
    let mut f = Fabrica::nueva(origen, semilla);
    match llamada {
        Llamada::Dibujar(figuras) => {
            // Por capas: las de capa mayor, encima (al final). Estable: a
            // igual capa, el orden de la lista.
            let mut orden: Vec<&Figura> = figuras.iter().collect();
            orden.sort_by_key(|fig| fig.capa);
            orden.into_iter().for_each(|fig| figura(&mut f, fig));
        }
        Llamada::Grafica(g) => grafica(&mut f, g),
        Llamada::Svg(s) => crate::svg::elementos(&mut f, s),
        Llamada::Funciones(p) => crate::funciones::elementos(&mut f, p),
        Llamada::Ecuacion(e) => crate::ecuacion::elementos(&mut f, e),
        Llamada::Cronograma(c) => crate::cronograma::elementos(&mut f, c),
        Llamada::Fisica(d) => crate::fisica::elementos(&mut f, d),
        Llamada::Problema(p) => crate::problema::elementos(&mut f, p),
        // No dibujan nada por si solas: el OCR y la ayuda son texto, la
        // accion de la app es un pedido, el calcado necesita la imagen
        // ([`elementos_de_calcado`]), la biblioteca el fichero
        // ([`elementos_de_figura`]) y buscar, el indice.
        Llamada::Ocr(_)
        | Llamada::Calcar(_)
        | Llamada::App(_)
        | Llamada::Ayuda(_)
        | Llamada::Figuras(_)
        | Llamada::Buscar(_)
        | Llamada::Editar(_) => {}
    }
    f.salida
}

/// **Una figura de la biblioteca**, lista para el lienzo, agrupada (se
/// guardo como una cosa y vuelve como una cosa).
pub fn elementos_de_figura(figura: &pixpin_motor2d::biblioteca::FiguraGuardada, origen: (f64, f64), semilla: u64) -> Vec<Value> {
    let mut f = Fabrica::nueva(origen, semilla);
    f.grupo = Some(f.nombre_de_grupo("figura"));
    f.de_motor(&figura.elementos);
    f.salida
}

/// Un `.excalidraw` completo con esos elementos, listo para
/// `pixpin_motor2d::excalidraw::leer`.
pub fn documento(elementos: Vec<Value>) -> String {
    json!({
        "type": "excalidraw",
        "version": 2,
        "source": "pixpin-ia",
        "elements": elementos,
        "appState": {"viewBackgroundColor": "#ffffff"},
        "files": {},
    })
    .to_string()
}

pub(crate) fn figura(f: &mut Fabrica, fig: &Figura) {
    let trazo = fig.color.0.as_str();
    let relleno = fig.relleno.as_ref().map(|c| c.0.as_str());
    let con_rotulo = !matches!(fig.tipo, TipoFigura::Texto | TipoFigura::Tabla) && !fig.texto.is_empty();
    // El grupo que pidio el modelo; si no, una figura con su rotulo va
    // agrupada con el, para que se muevan juntos.
    f.grupo = match &fig.grupo {
        Some(g) => Some(format!("ia-{:x}-g-{g}", f.semilla)),
        None if con_rotulo => Some(format!("ia-{:x}-r-{}", f.semilla, f.n + 1)),
        None => None,
    };
    let caja = [fig.x, fig.y, fig.ancho, fig.alto];
    // Los puntos (las figuras de antes de `puntos` traen solo los extremos).
    let puntos: Vec<(f64, f64)> = if fig.puntos.is_empty() {
        vec![(fig.x, fig.y), (fig.x2, fig.y2)]
    } else {
        fig.puntos.clone()
    };
    let desde = f.salida.len();
    match fig.tipo {
        TipoFigura::Rectangulo => f.caja("rectangle", caja, trazo, relleno),
        TipoFigura::Lapiz => f.lapiz(&puntos, trazo, fig.estilo.material, fig.estilo.dureza),
        TipoFigura::Punto => {
            f.caja("pixpin-point", [fig.x, fig.y, 0.0, 0.0], trazo, Some(trazo));
            if let Some(e) = f.salida.last_mut() {
                e["text"] = json!(fig.texto);
                e["roughness"] = json!(0);
            }
        }
        TipoFigura::Instrumento => {
            if let Some(i) = &fig.instrumento {
                instrumento(f, caja, i, trazo);
            }
        }
        TipoFigura::Tabla => {
            // La tabla dibujada del motor (la de la hoja al lienzo), con la
            // primera fila de cabecera; las celdas vienen en el texto.
            let filas: Vec<Vec<String>> = fig.texto.lines().map(|l| l.split('\t').map(str::to_string).collect()).collect();
            let color = pixpin_motor2d::excalidraw::color_desde(Some(&json!(trazo))).unwrap_or(pixpin_motor2d::elemento::ColorRgba::opaco(0.12, 0.12, 0.12));
            let tam = fig.tam_letra.clamp(10.0, 28.0).min(20.0) as f32;
            let estilo = pixpin_motor2d::ecuacion::Estilo { color, tam, opacidad: 1.0, familia: "Segoe UI".into() };
            let medir_t = |s: &str, t: f32| medir_con(s, t, "Segoe UI");
            let els = pixpin_motor2d::tabla_dibujada::elementos_de_tabla(
                &filas,
                &estilo,
                pixpin_motor2d::vector::Punto2::nuevo(fig.x as f32, fig.y as f32),
                &medir_t,
                filas.len() > 1,
            );
            if f.grupo.is_none() {
                f.grupo = Some(f.nombre_de_grupo("tabla"));
            }
            // `de_motor` ya suma el origen de la fabrica.
            f.de_motor(&els);
        }
        TipoFigura::Elipse => f.caja("ellipse", caja, trazo, relleno),
        TipoFigura::Rombo => f.caja("diamond", caja, trazo, relleno),
        TipoFigura::Linea => f.trazo("line", &puntos, trazo, None),
        TipoFigura::Flecha => f.trazo("arrow", &puntos, trazo, None),
        // Un texto que se quedo sin texto (ver `colores_del_pedido`) no se pinta.
        TipoFigura::Texto if fig.texto.is_empty() => {}
        TipoFigura::Texto => f.texto(fig.x, fig.y, &fig.texto, fig.tam_letra, trazo, "left"),
        TipoFigura::Curva => {
            f.trazo("line", &puntos, trazo, None);
            // `roundness` en una linea: la curva suave que pasa por sus
            // puntos (la del lienzo y la del movil).
            if let Some(e) = f.salida.last_mut() {
                e["roundness"] = json!({"type": 2});
            }
        }
        // Lleno: la mancha rellena del bote (un `line` cerrado no se
        // rellena en el lienzo del PC). Vacio: una linea cerrada.
        TipoFigura::Poligono => match relleno {
            Some(r) => f.region(&puntos, &[], trazo, r),
            None => {
                let mut cerrado = puntos.clone();
                cerrado.push(puntos[0]);
                f.trazo("line", &cerrado, trazo, None);
            }
        },
    }
    f.estilo(desde, &fig.estilo);
    let con_rotulo = con_rotulo && !matches!(fig.tipo, TipoFigura::Punto | TipoFigura::Instrumento);
    if con_rotulo {
        let negro = "#1e1e1e";
        match fig.tipo {
            // Encima de la flecha o la linea, en su punto medio.
            TipoFigura::Linea | TipoFigura::Flecha | TipoFigura::Curva => {
                let (a, b) = (puntos[0], puntos[puntos.len() - 1]);
                let tam = fig.tam_letra.min(20.0);
                f.texto((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0 - tam * 1.25 - 4.0, &fig.texto, tam, negro, "center");
            }
            // Dentro, centrado, y que quepa.
            _ => {
                // La caja de verdad: en un poligono (triangulo,
                // paralelogramo) la de sus puntos, que su primer y ultimo
                // punto no son esquinas opuestas.
                let (x, y, ancho, alto) = if fig.tipo == TipoFigura::Poligono && puntos.len() >= 3 {
                    let x0 = puntos.iter().map(|p| p.0).fold(f64::MAX, f64::min);
                    let y0 = puntos.iter().map(|p| p.1).fold(f64::MAX, f64::min);
                    let x1 = puntos.iter().map(|p| p.0).fold(f64::MIN, f64::max);
                    let y1 = puntos.iter().map(|p| p.1).fold(f64::MIN, f64::max);
                    (x0, y0, x1 - x0, y1 - y0)
                } else {
                    (fig.x, fig.y, fig.ancho, fig.alto)
                };
                let letras = fig.texto.chars().count().max(1) as f64;
                let tam = fig.tam_letra.min(ancho.abs() * 0.75 / (0.6 * letras)).max(8.0);
                let (cx, cy) = (x + ancho / 2.0, y + alto / 2.0);
                let color = if relleno.is_some_and(|r| es_oscuro(r)) { "#ffffff" } else { negro };
                f.texto(cx, cy - tam * 1.25 / 2.0, &fig.texto, tam, color, "center");
            }
        }
    }
    f.grupo = None;
}

/// **El instrumento de graficar con los puntos de su tabla**: el instrumento
/// del motor (`instrumento::Instrumento`), cada punto de la tabla como un
/// punto con su letra donde cae en el instrumento (`instrumento::punto`), y
/// la tabla de coordenadas dibujada al lado (`tabla_dibujada`), todo en un
/// grupo. Las tablas de la escena (`escena.tablas`) cuentan en pixeles o en
/// la escala calibrada, no en las unidades del instrumento: por eso los
/// puntos van como elementos que caen sobre sus rayas.
fn instrumento(f: &mut Fabrica, [x, y, w, h]: [f64; 4], p: &crate::herramientas::PedidoInstrumento, color: &str) {
    use pixpin_motor2d::instrumento as ins;
    let tinta = pixpin_motor2d::excalidraw::color_desde(Some(&json!(color)))
        .unwrap_or(pixpin_motor2d::ColorRgba::opaco(0.12, 0.12, 0.12));
    let mut e = ins::elemento_de_fabrica(p.ejes, tinta, 1.5, 1.0);
    if let pixpin_motor2d::elemento::Figura::Instrumento(i) = &mut e.figura {
        i.unidad = Some(p.unidad);
        // Los numeros, que no se amontonen: uno cada 1, 2 o 5 unidades.
        let paso = ins::paso_util(1.0, p.unidad, 34.0);
        i.paso_numeros = Some(paso);
    }
    e.x = x as f32;
    e.y = y as f32;
    e.ancho = w.max(40.0) as f32;
    e.alto = if p.ejes == 1 { (h.max(ins::ALTO_DE_LA_RECTA as f64)) as f32 } else { h.max(40.0) as f32 };
    let i = *ins::de(&e).expect("es un instrumento");
    let mut piezas = vec![e.clone()];
    let letras = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    for (k, &(px, py)) in p.tabla.iter().enumerate() {
        let en = ins::punto(&e, &i, px, py, 0.0);
        let letra = letras.chars().nth(k % 26).map(String::from).unwrap_or_default();
        piezas.push(pixpin_motor2d::Elemento {
            figura: pixpin_motor2d::elemento::Figura::Punto {
                letra,
                angulo: -std::f32::consts::FRAC_PI_4,
                radio: 14.0,
            },
            x: en.x,
            y: en.y,
            trazo: tinta,
            relleno: Some(tinta),
            grosor: 2.0,
            rugosidad: 0.0,
            ..Default::default()
        });
    }
    if !p.tabla.is_empty() {
        let estilo = pixpin_motor2d::ecuacion::Estilo {
            color: tinta,
            tam: 16.0,
            opacidad: 1.0,
            familia: "Nunito".into(),
        };
        let filas: Vec<Vec<String>> = std::iter::once(vec![String::new(), "x".into(), "y".into()])
            .chain(p.tabla.iter().enumerate().map(|(k, &(px, py))| {
                vec![
                    letras.chars().nth(k % 26).map(String::from).unwrap_or_default(),
                    ins::escrito(px),
                    ins::escrito(py),
                ]
            }))
            .collect();
        let origen = pixpin_motor2d::vector::Punto2::nuevo(e.x + e.ancho + 24.0, e.y);
        piezas.extend(pixpin_motor2d::tabla_dibujada::elementos_de_tabla(&filas, &estilo, origen, &medir, true));
    }
    let antes = f.grupo.clone();
    if f.grupo.is_none() {
        f.grupo = Some(f.nombre_de_grupo("instrumento"));
    }
    // `de_motor` corre al origen; el instrumento ya esta en coordenadas
    // del modelo, asi que se escribe tal cual y se corre una vez.
    f.de_motor(&piezas);
    f.grupo = antes;
}

/// Lo que mide un texto: DirectWrite si la app puso su medidor, a ojo si no
/// (las pruebas).
pub(crate) fn medir(texto: &str, tam: f32) -> (f32, f32) {
    medir_con(texto, tam, "Nunito")
}

/// Lo que mide un texto rico compuesto (con DirectWrite si lo hay; si no,
/// con la estimacion del motor).
pub(crate) fn texto_rico_medida(fuente: &str, tam: f32, familia: &str) -> (f32, f32) {
    pixpin_motor2d::texto_rico::medida(fuente, tam, familia, pixpin_motor2d::texto::EstiloDeTexto::default())
}

pub(crate) fn medir_con(texto: &str, tam: f32, familia: &str) -> (f32, f32) {
    use pixpin_motor2d::texto;
    if texto::hay_medidor() {
        return texto::medida(texto, tam, familia, texto::EstiloDeTexto::default());
    }
    // Sin DirectWrite (pruebas, banco): los anchos de letra del exportador,
    // que son los de la letra con la que sale el PDF; la estimacion de a ojo
    // deja una letra de holgura y separaba las piezas de una ecuacion.
    let renglones = texto.split('\n').count().max(1) as f32;
    let ancho = texto
        .split('\n')
        .map(|r| pixpin_motor2d::exportar::ancho_de_texto(r, tam))
        .fold(0.0, f32::max);
    (ancho, renglones * tam * texto::INTERLINEADO)
}

/// Si un `#rrggbb` es oscuro (para escribir en blanco encima).
fn es_oscuro(hex: &str) -> bool {
    let h = hex.trim_start_matches('#');
    let c = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255) as f64;
    0.299 * c(0) + 0.587 * c(2) + 0.114 * c(4) < 110.0
}

/// **Los elementos de un calcado** (`calcar::calcar`), todos en un grupo
/// para moverlos de una vez (se desagrupa para tocar una pieza). Lisos:
/// rugosidad 0, que un calco a mano alzada ya no se pareceria al original.
pub fn elementos_de_calcado(c: &crate::calcar::Calcado, origen: (f64, f64), semilla: u64) -> Vec<Value> {
    use crate::calcar::Trazo;
    let mut f = Fabrica {
        semilla,
        n: 0,
        ox: origen.0,
        oy: origen.1,
        salida: Vec::new(),
        grupo: Some(format!("ia-{semilla:x}-calco")),
    };
    for t in &c.trazos {
        match t {
            Trazo::Fondo { caja, color } => f.caja("rectangle", *caja, color, Some(color)),
            Trazo::Mancha { contorno, huecos, color } => f.region(contorno, huecos, color, color),
            Trazo::Linea { puntos, color, .. } => f.trazo("line", puntos, color, None),
            Trazo::Raya { puntos, color, grosor } => {
                f.trazo("line", puntos, color, None);
                if let Some(e) = f.salida.last_mut() {
                    e["strokeWidth"] = json!(((grosor * 2.0).round() / 2.0).max(0.5));
                }
            }
        }
        if let Some(e) = f.salida.last_mut() {
            e["roughness"] = json!(0);
            if !matches!(t, Trazo::Linea { .. } | Trazo::Raya { .. }) {
                // 2: el borde del mismo color tapa la rendija que deja el
                // simplificado entre dos manchas vecinas.
                e["strokeWidth"] = json!(2);
            }
        }
    }
    f.salida
}

/// El hueco de la grafica dentro del lienzo de 800x600.
const MARGEN: f64 = 60.0;

fn grafica(f: &mut Fabrica, g: &Grafica) {
    let negro = "#1e1e1e";
    let arriba = if g.titulo.is_empty() {
        MARGEN
    } else {
        f.texto(ANCHO_LIENZO / 2.0, 16.0, &g.titulo, 28.0, negro, "center");
        MARGEN + 20.0
    };
    let (x0, x1) = (MARGEN + 20.0, ANCHO_LIENZO - MARGEN);
    let (y0, y1) = (arriba, ALTO_LIENZO - MARGEN);
    match g.tipo {
        TipoGrafica::Pastel => pastel(f, g, (x0 + x1) / 2.0, (y0 + y1) / 2.0, ((y1 - y0) / 2.0 - 30.0).clamp(60.0, 200.0)),
        TipoGrafica::Barras | TipoGrafica::Lineas => {
            let min = g.valores.iter().copied().fold(0.0_f64, f64::min);
            let max = g.valores.iter().copied().fold(0.0_f64, f64::max);
            // Marcas «redondas» en el eje y (1, 2 o 5 por potencia de 10),
            // no solo el maximo: el eje llega a la marca de encima.
            let paso = paso_redondo((max - min).max(1e-9) / 5.0);
            let (min, max) = ((min / paso).floor() * paso, (max / paso).ceil() * paso);
            let rango = if max - min > 0.0 { max - min } else { 1.0 };
            let a_y = |v: f64| y1 - (v - min) / rango * (y1 - y0);
            let cero = a_y(0.0);
            // Ejes.
            f.trazo("line", &[(x0, y0), (x0, y1)], negro, None);
            f.trazo("line", &[(x0, cero), (x1, cero)], negro, None);
            let mut v = min;
            while v <= max + paso * 1e-6 {
                let y = a_y(v);
                f.trazo("line", &[(x0 - 5.0, y), (x0, y)], negro, None);
                if (v).abs() > paso * 1e-6 {
                    f.trazo("line", &[(x0, y), (x1, y)], "#dee2e6", None);
                }
                f.texto(x0 - 8.0, y - 10.0, &bonito((v / paso).round() * paso), 16.0, negro, "right");
                v += paso;
            }
            let n = g.valores.len() as f64;
            let paso = (x1 - x0) / n;
            let mut puntos = Vec::new();
            for (i, (v, etiqueta)) in g.valores.iter().zip(&g.etiquetas).enumerate() {
                let centro = x0 + paso * (i as f64 + 0.5);
                let y = a_y(*v);
                if g.tipo == TipoGrafica::Barras {
                    let ancho = paso * 0.6;
                    let color = SERIES[i % SERIES.len()];
                    f.caja("rectangle", [centro - ancho / 2.0, y.min(cero), ancho, (cero - y).abs().max(1.0)], color, Some(color));
                } else {
                    puntos.push((centro, y));
                }
                f.texto(centro, y.min(cero) - 24.0, &bonito(*v), 16.0, negro, "center");
                f.texto(centro, y1 + 8.0, etiqueta, 16.0, negro, "center");
            }
            if puntos.len() >= 2 {
                f.trazo("line", &puntos, SERIES[0], None);
            } else if let Some(&(x, y)) = puntos.first() {
                f.caja("ellipse", [x - 5.0, y - 5.0, 10.0, 10.0], SERIES[0], Some(SERIES[0]));
            }
        }
    }
}

fn pastel(f: &mut Fabrica, g: &Grafica, cx: f64, cy: f64, r: f64) {
    let total: f64 = g.valores.iter().sum();
    let mut angulo = -std::f64::consts::FRAC_PI_2;
    for (i, (v, etiqueta)) in g.valores.iter().zip(&g.etiquetas).enumerate() {
        let barrido = v / total * std::f64::consts::TAU;
        if barrido <= 0.0 {
            continue;
        }
        let color = SERIES[i % SERIES.len()];
        // La cuna como poligono cerrado: centro, arco muestreado, centro.
        let pasos = ((barrido / 0.1).ceil() as usize).max(2);
        let mut puntos = vec![(cx, cy)];
        for k in 0..=pasos {
            let a = angulo + barrido * k as f64 / pasos as f64;
            puntos.push((cx + r * a.cos(), cy + r * a.sin()));
        }
        // Como REGION: una `line` cerrada el lienzo del PC no la rellena y
        // el pastel salia sin porciones, solo con los rotulos (banco 15-oct).
        f.region(&puntos[..puntos.len() - 1], &[], "#ffffff", color);
        let medio = angulo + barrido / 2.0;
        let porcentaje = v / total * 100.0;
        // El modelo a veces ya pone el % en la etiqueta («Otros 5%»): sin
        // repetirlo («Otros 5% (5%)»).
        let rotulo = if etiqueta.contains('%') { etiqueta.clone() } else { format!("{etiqueta} ({porcentaje:.0}%)") };
        // Pegado al borde, hacia fuera: a la derecha del pastel alineado a
        // la izquierda y al reves, que centrado se metia en las porciones.
        let (c, s) = (medio.cos(), medio.sin());
        let lado = if c > 0.25 { "left" } else if c < -0.25 { "right" } else { "center" };
        f.texto(cx + (r + 14.0) * c, cy + (r + 14.0) * s - 10.0 + 10.0 * s, &rotulo, 16.0, "#1e1e1e", lado);
        angulo += barrido;
    }
}

/// El paso de marcas «redondo» (1, 2, 2,5 o 5 por potencia de 10) mas cerca
/// por encima de `bruto`.
pub(crate) fn paso_redondo(bruto: f64) -> f64 {
    if !(bruto.is_finite() && bruto > 0.0) {
        return 1.0;
    }
    let p = 10f64.powf(bruto.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].into_iter().map(|k| k * p).find(|&s| s >= bruto * 0.999).unwrap_or(10.0 * p)
}

/// Un numero sin decimales de sobra.
fn bonito(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::herramientas::{catalogo, validar};
    use crate::motor::LlamadaCruda;
    use pixpin_motor2d::elemento::Figura as FiguraLienzo;

    fn llamada(nombre: &str, args: &str) -> Llamada {
        validar(
            &LlamadaCruda {
                id: "1".into(),
                nombre: nombre.into(),
                argumentos: args.into(),
            },
            &catalogo(),
        )
        .unwrap()
        .llamada
    }

    #[test]
    fn el_dibujo_lo_lee_el_lienzo_de_verdad() {
        let l = llamada(
            "dibujar_en_lienzo",
            r##"{"elementos":[
                {"tipo":"elipse","x":40,"y":60,"ancho":150,"alto":150,"color":"rojo","relleno":"rojo"},
                {"tipo":"rectangulo","x":280,"y":60,"ancho":160,"alto":160,"color":"azul"},
                {"tipo":"linea","x":40,"y":300,"x2":470,"y2":300,"color":"verde"},
                {"tipo":"flecha","x":10,"y":10,"x2":60,"y2":40},
                {"tipo":"rombo","x":500,"y":60},
                {"tipo":"texto","x":60,"y":315,"texto":"HOLA PIXPIN 2026"}]}"##,
        );
        let doc = documento(elementos(&l, (1000.0, 2000.0), 7));
        let lienzo = pixpin_motor2d::excalidraw::leer(&doc).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 0, "todo tiene que ser editable");
        let els = lienzo.elementos();
        assert_eq!(els.len(), 6);
        assert!(matches!(els[0].figura, FiguraLienzo::Elipse));
        assert!(matches!(els[1].figura, FiguraLienzo::Rectangulo));
        assert!(matches!(els[2].figura, FiguraLienzo::Linea { .. }));
        assert!(matches!(els[3].figura, FiguraLienzo::Flecha { .. }));
        assert!(matches!(els[4].figura, FiguraLienzo::Rombo));
        match &els[5].figura {
            FiguraLienzo::Texto { texto, .. } => assert_eq!(texto, "HOLA PIXPIN 2026"),
            otra => panic!("{otra:?}"),
        }
        // Desplazado al origen pedido.
        let v: Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(v["elements"][0]["x"], 1040.0);
        assert_eq!(v["elements"][2]["points"][1], json!([430.0, 0.0]));
    }

    /// El texto rico del asistente: formulas en linea y colores, con las
    /// barras de mas de los modelos, y su texto plano de reserva.
    #[test]
    fn el_texto_rico_del_asistente_llega_compuesto_al_lienzo() {
        let l = llamada(
            "dibujar_en_lienzo",
            r##"{"elementos":[{"tipo":"texto","x":40,"y":60,"texto":"Nos piden $m\\\\sphericalangle COD = \\\\color{rojo}{X}$\n\\\\centro $\\\\overline{AB}$"},
                {"tipo":"texto","x":40,"y":200,"texto":"cuesta $5"}]}"##,
        );
        let doc = documento(elementos(&l, (0.0, 0.0), 7));
        let els = pixpin_motor2d::excalidraw::leer(&doc).unwrap().elementos();
        let rico = els[0].extras.rico.as_deref().expect("es rico");
        assert_eq!(rico, "Nos piden $m\\sphericalangle COD = \\color{rojo}{X}$\n\\centro $\\overline{AB}$");
        assert!(matches!(&els[0].figura, FiguraLienzo::Texto { texto, .. } if texto == "Nos piden m∢COD = X\nA̅B̅"));
        // Lo que no lleva marcas sigue como siempre.
        assert_eq!(els[1].extras.rico, None);
    }

    #[test]
    fn los_identificadores_no_se_repiten() {
        let l = llamada("grafica", r#"{"tipo":"barras","etiquetas":["a","b","c"],"valores":[1,2,3],"titulo":"t"}"#);
        let els = elementos(&l, (0.0, 0.0), 99);
        let mut ids: Vec<&str> = els.iter().map(|e| e["id"].as_str().unwrap()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn las_tres_graficas_salen_y_se_leen() {
        for (args, minimo) in [
            (r#"{"tipo":"barras","etiquetas":["Enero","Febrero","Marzo"],"valores":[120,95,140],"titulo":"Ventas"}"#, 3 + 3 * 3),
            (r#"{"tipo":"lineas","etiquetas":["a","b","c","d"],"valores":[1,-2,3.5,0]}"#, 3 + 4 * 2 + 1),
            (r#"{"tipo":"pastel","etiquetas":["si","no"],"valores":[3,1]}"#, 4),
        ] {
            let els = elementos(&llamada("grafica", args), (0.0, 0.0), 1);
            assert!(els.len() >= minimo, "{args}: {}", els.len());
            let lienzo = pixpin_motor2d::excalidraw::leer(&documento(els)).unwrap();
            assert_eq!(lienzo.cuantos_ajenos(), 0, "{args}");
        }
    }

    /// Banco 15-oct: el pastel salia sin porciones (lineas cerradas que el
    /// lienzo no rellena) y con «Otros 5% (5%)».
    #[test]
    fn el_pastel_tiene_porciones_rellenas_y_un_solo_porcentaje() {
        let args = r#"{"tipo":"pastel","etiquetas":["Materiales 45%","Otros"],"valores":[45,5]}"#;
        let els = elementos(&llamada("grafica", args), (0.0, 0.0), 1);
        let lienzo = pixpin_motor2d::excalidraw::leer(&documento(els)).unwrap();
        let els = lienzo.elementos();
        let regiones = els.iter().filter(|e| matches!(e.figura, FiguraLienzo::Region { .. }) && e.relleno.is_some()).count();
        assert_eq!(regiones, 2);
        let textos: Vec<String> = els
            .iter()
            .filter_map(|e| match &e.figura {
                FiguraLienzo::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        assert!(textos.contains(&"Materiales 45%".to_string()), "{textos:?}");
        assert!(textos.contains(&"Otros (10%)".to_string()), "{textos:?}");
    }

    /// Banco 15-oct: «haz una tabla de los planetas» acababa en una nota (no
    /// habia tabla en el lienzo), y el «Leer N» del flujo era un rectangulo.
    #[test]
    fn la_tabla_y_el_paralelogramo_se_dibujan() {
        let l = llamada(
            "dibujar_en_lienzo",
            r#"{"elementos":[
                {"tipo":"tabla","x":40,"y":40,"filas":[["Planeta","UA","Lunas"],["Mercurio","0.39","0"],["Venus",0.72,0]]},
                {"tipo":"paralelogramo","caja":[100,300,200,60],"texto":"Leer N"}]}"#,
        );
        let els = elementos(&l, (0.0, 0.0), 3);
        let lienzo = pixpin_motor2d::excalidraw::leer(&documento(els)).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 0);
        let els = lienzo.elementos();
        let textos: Vec<String> = els
            .iter()
            .filter_map(|e| match &e.figura {
                FiguraLienzo::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        for t in ["Planeta", "Mercurio", "0.72", "Lunas", "Leer N"] {
            assert!(textos.iter().any(|x| x == t), "{t}: {textos:?}");
        }
        // El rotulo del paralelogramo, dentro de su caja.
        let leer = els.iter().find(|e| matches!(&e.figura, FiguraLienzo::Texto { texto, .. } if texto == "Leer N")).unwrap();
        let (x0, y0, x1, y1) = leer.caja();
        assert!(x0 >= 100.0 && x1 <= 300.0 && y0 >= 300.0 && y1 <= 360.0, "{:?}", leer.caja());
        // Una tabla sin filas se rechaza con el motivo.
        let e = validar(
            &LlamadaCruda { id: "1".into(), nombre: "dibujar_en_lienzo".into(), argumentos: r#"{"elementos":[{"tipo":"tabla","x":1,"y":1}]}"#.into() },
            &catalogo(),
        )
        .unwrap_err();
        assert!(e.to_string().contains("filas"), "{e}");
    }

    #[test]
    fn las_barras_llevan_varias_marcas_en_el_eje() {
        let args = r#"{"tipo":"barras","etiquetas":["Ana","Luis"],"valores":[15,18]}"#;
        let els = elementos(&llamada("grafica", args), (0.0, 0.0), 1);
        let marcas: Vec<&str> = els.iter().filter_map(|e| e["text"].as_str()).filter(|t| t.parse::<f64>().is_ok()).collect();
        for m in ["0", "5", "10", "15", "20"] {
            assert!(marcas.contains(&m), "{marcas:?}");
        }
        assert_eq!(paso_redondo(3.6), 5.0);
        assert_eq!(paso_redondo(0.18), 0.2);
    }

    #[test]
    fn el_ocr_no_dibuja() {
        assert!(elementos(&llamada("ocr", "{}"), (0.0, 0.0), 1).is_empty());
    }

    #[test]
    fn numeros_bonitos() {
        assert_eq!(bonito(140.0), "140");
        assert_eq!(bonito(3.5), "3.5");
        assert_eq!(bonito(-2.0), "-2");
    }

    #[test]
    fn las_figuras_nuevas_las_lee_el_lienzo() {
        let l = llamada(
            "dibujar_en_lienzo",
            r#"{"elementos":[
                {"tipo":"triangulo","caja":[280,170,240,110],"color":"rojo","relleno":"rojo","grupo":"casa"},
                {"tipo":"rectangulo","caja":[300,280,200,160],"color":"marron","grupo":"casa"},
                {"tipo":"poligono","puntos":[10,10,60,10,35,50],"color":"verde"},
                {"tipo":"curva","puntos":[0,300,100,250,200,350,300,300],"color":"azul"},
                {"tipo":"rectangulo","caja":[60,260,160,70],"texto":"Inicio","relleno":"azul"},
                {"tipo":"flecha","puntos":[220,295,320,295],"texto":"si"}]}"#,
        );
        let els = elementos(&l, (0.0, 0.0), 5);
        let doc = documento(els.clone());
        let lienzo = pixpin_motor2d::excalidraw::leer(&doc).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 0);
        let leidos = lienzo.elementos();
        // triangulo, casa, poligono, curva, caja + rotulo, flecha + rotulo.
        assert_eq!(leidos.len(), 8);
        assert!(matches!(&leidos[0].figura, FiguraLienzo::Region { contorno, .. } if contorno.len() == 3));
        assert!(leidos[0].relleno.is_some());
        assert_eq!(leidos[0].grupos, leidos[1].grupos);
        assert!(!leidos[0].grupos.is_empty());
        // Sin relleno: linea cerrada (vuelve al primer punto).
        assert!(matches!(&leidos[2].figura, FiguraLienzo::Linea { puntos } if puntos.len() == 4 && puntos[0] == puntos[3]));
        // La curva: linea redondeada.
        assert!(matches!(&leidos[3].figura, FiguraLienzo::Linea { puntos } if puntos.len() == 4));
        assert!(leidos[3].redondo);
        // El rotulo, en blanco sobre azul, centrado y agrupado con su caja.
        match &leidos[5].figura {
            FiguraLienzo::Texto { texto, .. } => assert_eq!(texto, "Inicio"),
            otra => panic!("{otra:?}"),
        }
        assert_eq!(els[5]["strokeColor"], "#ffffff");
        assert_eq!(leidos[4].grupos, leidos[5].grupos);
        assert!(matches!(&leidos[7].figura, FiguraLienzo::Texto { texto, .. } if texto == "si"));
    }

    #[test]
    fn un_calcado_lo_lee_el_lienzo_en_un_grupo() {
        let (w, h) = (300u32, 200u32);
        let mut px = vec![255u8; (w * h * 4) as usize];
        for y in 50..150 {
            for x in 50..150 {
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 3].copy_from_slice(&[200, 20, 20]);
            }
        }
        // Un anillo azul (con su agujero).
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f64 - 230.0).powi(2) + (y as f64 - 100.0).powi(2)).sqrt();
                if (25.0..=50.0).contains(&d) {
                    let i = ((y * w + x) * 4) as usize;
                    px[i..i + 3].copy_from_slice(&[20, 40, 200]);
                }
            }
        }
        for p in [crate::calcar::Parametros::default(), crate::calcar::Parametros { solo_contornos: true, ..Default::default() }] {
            let c = crate::calcar::calcar(&px, w, h, &p);
            let els = elementos_de_calcado(&c, (0.0, 0.0), 9);
            assert!(els.len() >= 2, "{p:?}: {}", els.len());
            let lienzo = pixpin_motor2d::excalidraw::leer(&documento(els)).unwrap();
            assert_eq!(lienzo.cuantos_ajenos(), 0);
            let leidos = lienzo.elementos();
            assert!(leidos.iter().all(|e| e.grupos == leidos[0].grupos && !e.grupos.is_empty()));
            if !p.solo_contornos {
                let anillo = leidos
                    .iter()
                    .find_map(|e| match &e.figura {
                        FiguraLienzo::Region { huecos, .. } if !huecos.is_empty() => Some(huecos.len()),
                        _ => None,
                    })
                    .expect("el anillo con su agujero");
                assert_eq!(anillo, 1);
            }
        }
    }
}
