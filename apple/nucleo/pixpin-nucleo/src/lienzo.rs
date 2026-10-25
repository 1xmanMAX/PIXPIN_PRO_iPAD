//! El lienzo cruzando a Swift como **un solo bufer plano**.
//!
//! `pixpin_motor2d::pintado` ya convierte una escena en ordenes de dibujo
//! (poligonos, polilineas, textos, imagenes) sin pintar nada. Aqui esas
//! ordenes se empaquetan en un unico bloque de numeros para que Swift las
//! recorra en una pasada y construya sus `CGPath`: **una llamada al puente
//! por escena o por fotograma**, nunca un objeto por figura (§4bis.1 del
//! plan). Un `Vec<Orden>` cruzado tal cual seria un objeto de Swift por
//! orden y por punto; esto es una copia de memoria.
//!
//! # Por que bytes y no `Vec<f32>`
//!
//! UniFFI pasa un `Vec<f32>` numero a numero (con su cambio de orden de
//! bytes en cada uno), pero un `Vec<u8>` llega a Swift como `Data` de una
//! sola copia. Asi que [`LienzoPlano::datos`] son `f32` en **little-endian**
//! (el orden de todos los procesadores de Apple) pegados uno detras de otro:
//! en Swift basta `datos.withUnsafeBytes { $0.bindMemory(to: Float.self) }`
//! y se leen sin convertir nada. Los textos y los ids de imagen no caben
//! exactos en un `f32` y van en dos tablas aparte, citados por su indice.
//!
//! # El formato, version [`VERSION_PLANO`]
//!
//! Todo son `f32`. Los enteros (indices, cuentas, codigos) son exactos hasta
//! 2^24, de sobra para un lienzo.
//!
//! **Cabecera** (`CABECERA` = 12 numeros):
//!
//! | pos | que |
//! |---|---|
//! | 0 | version del formato (`1`). Si no es la que Swift conoce, no se lee. |
//! | 1 | largo de la cabecera (`12`): una version nueva puede alargarla y una lectura vieja salta lo que no conoce. |
//! | 2 | cuantas ordenes siguen |
//! | 3..7 | color del papel `r g b a` (0..1) |
//! | 7..11 | caja de lo dibujado `x0 y0 x1 y1` en unidades del mundo (`NaN` si esta vacio) |
//! | 11 | largo total del bufer en numeros (para comprobar) |
//!
//! **Cada orden** empieza por `codigo, largo` (`largo` cuenta tambien esos
//! dos), asi que una orden de un codigo desconocido se salta entera. Los
//! puntos son `n` y despues `x0 y0 x1 y1 ...` en unidades del mundo.
//!
//! | codigo | orden | despues de `codigo, largo` |
//! |---|---|---|
//! | 1 | `Poligono` (rellenar, regla par-impar) | `r g b a, n, puntos` |
//! | 2 | `Tinta` (contorno de un trazo a mano: rellenar con curvas por puntos medios, regla *winding*) | `r g b a, n, puntos` |
//! | 3 | `Polilinea` (linea abierta) | `r g b a, grosor, estilo, n, puntos` — estilo 0 solido, 1 discontinuo, 2 punteado |
//! | 4 | `Relleno` (interior de una figura) | `r g b a, n, puntos` |
//! | 5 | `Velo` (oscurecer todo menos el hueco) | `r g b a, n, puntos` |
//! | 6 | `Texto` | `r g b a, x, y, tam, ancho_max, caras, texto, familia` — caras: bit 0 negrita, bit 1 cursiva; texto y familia son indices en `textos` |
//! | 7 | `Rotulo` (numero de una cota, girado y con halo) | `r g b a, halo r g b a, x, y, tam, grosor_halo, cx, cy, angulo, texto, familia` |
//! | 8 | `Imagen` | `x, y, ancho, alto, opacidad, angulo, imagen, con_recorte, rx, ry, rancho, ralto, ancho_natural, alto_natural` — imagen es indice en `imagenes` (el `id_objeto`); el recorte solo vale si `con_recorte` es 1 |
//!
//! [`decodificar`] es la lectura de referencia (la usan las pruebas): la de
//! Swift tiene que hacer exactamente lo mismo.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use pixpin_motor2d::Escena;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::elemento::{ColorRgba, EstiloTrazo, RecorteImagen};
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::vector::Punto2;

use crate::{ErrorNucleo, Nucleo};

/// La version del formato del bufer. Se sube al cambiar algo que una
/// lectura vieja leeria mal (anadir un codigo nuevo NO la sube: se salta).
pub const VERSION_PLANO: u32 = 1;
/// Cuantos numeros tiene la cabecera.
pub const CABECERA: usize = 12;

pub const POLIGONO: u32 = 1;
pub const TINTA: u32 = 2;
pub const POLILINEA: u32 = 3;
pub const RELLENO: u32 = 4;
pub const VELO: u32 = 5;
pub const TEXTO: u32 = 6;
pub const ROTULO: u32 = 7;
pub const IMAGEN: u32 = 8;

/// Las ordenes de dibujo de una escena, empaquetadas (ver la doc del
/// modulo).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LienzoPlano {
    /// Los `f32` en little-endian.
    pub datos: Vec<u8>,
    /// Los textos y las familias de letra, cada uno una vez.
    pub textos: Vec<String>,
    /// Los `id_objeto` de las imagenes de este bufer.
    pub imagenes: Vec<u64>,
}

/// La caja que abarca lo dibujado, en unidades del mundo.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CajaLienzo {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// Una imagen del lienzo: su `id_objeto` (el de las ordenes) y su fichero,
/// relativo a la carpeta del chat (`imagenes/<id>`). Swift la lee con
/// ImageIO y la guarda en su cache por `id_objeto`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImagenDelLienzo {
    pub id_objeto: u64,
    pub ruta: String,
}

/// **Un lienzo leido y listo para pintar.** Se lee una vez (el JSON es lo
/// caro) y se piden sus ordenes tantas veces como haga falta: al mover o
/// acercar la vista, cada fotograma es una llamada a
/// [`Lienzo::plano_de_vista`], que solo mira lo que cae en pantalla.
#[derive(uniffi::Object)]
pub struct Lienzo {
    escena: Escena,
    imagenes: Vec<ImagenDelLienzo>,
}

impl Lienzo {
    fn de_lienzo(l: &excalidraw::Lienzo) -> Lienzo {
        Lienzo {
            escena: excalidraw::a_escena(l),
            imagenes: excalidraw::ficheros(l)
                .into_iter()
                .map(|(id_objeto, ruta)| ImagenDelLienzo { id_objeto, ruta })
                .collect(),
        }
    }

    fn plano_de(&self, ordenes: &[Orden]) -> LienzoPlano {
        empaquetar(ordenes, self.escena.fondo, self.escena.caja())
    }

    /// La escena leida (para compartir sus hojas, `compartir.rs`).
    pub(crate) fn escena(&self) -> &Escena {
        &self.escena
    }

    /// Un lienzo de una escena hecha aqui, sin fotos (una figura de la
    /// biblioteca, `galeria_figuras.rs`).
    pub(crate) fn de_escena(escena: Escena) -> Lienzo {
        Lienzo {
            escena,
            imagenes: Vec::new(),
        }
    }
}

#[uniffi::export]
impl Lienzo {
    /// Un lienzo desde el texto de un `.excalidraw`.
    #[uniffi::constructor]
    pub fn de_json(texto: String) -> Result<Arc<Lienzo>, ErrorNucleo> {
        let l = excalidraw::leer(&texto).map_err(|e| ErrorNucleo::Lienzo {
            detalle: e.to_string(),
        })?;
        Ok(Arc::new(Lienzo::de_lienzo(&l)))
    }

    /// Un lienzo desde un fichero `.excalidraw` o `.pixpin2d` (el dibujo
    /// propio de PixPin, `pixpin_motor2d::formato`; se mira la extension).
    /// Lo usa tambien la vista previa de Quick Look.
    #[uniffi::constructor]
    pub fn de_fichero(ruta: String) -> Result<Arc<Lienzo>, ErrorNucleo> {
        leer_fichero(Path::new(&ruta))
    }

    /// Todas las ordenes de la escena, sin simplificar (para exportar o para
    /// una miniatura).
    pub fn plano(&self) -> LienzoPlano {
        self.plano_de(&crate::grafito_plano::ordenes_de_escena(&self.escena))
    }

    /// Las ordenes de lo que se ve con la camara en `(x, y)` (el punto del
    /// mundo en la esquina de arriba a la izquierda) y `zoom` pixeles por
    /// unidad, en una ventana de `ancho_px` x `alto_px`. Ni mira lo que cae
    /// fuera, ni dibuja el detalle que a ese aumento no se distingue: es la
    /// via de cada fotograma.
    pub fn plano_de_vista(
        &self,
        x: f32,
        y: f32,
        zoom: f32,
        ancho_px: f32,
        alto_px: f32,
    ) -> LienzoPlano {
        let camara = Camara { x, y, zoom };
        self.plano_de(&pintado::ordenes_de_escena_vista(
            &self.escena,
            &camara,
            ancho_px,
            alto_px,
        ))
    }

    /// La caja de lo dibujado, para encajar la vista. `None` si esta vacio.
    pub fn caja(&self) -> Option<CajaLienzo> {
        self.escena
            .caja()
            .map(|(x0, y0, x1, y1)| CajaLienzo { x0, y0, x1, y1 })
    }

    /// Las imagenes que usa, con su fichero. Se piden una vez, no por
    /// fotograma.
    pub fn imagenes(&self) -> Vec<ImagenDelLienzo> {
        self.imagenes.clone()
    }

    /// Cuantos elementos visibles tiene.
    pub fn cuantos_elementos(&self) -> u64 {
        self.escena.a_la_vista().count() as u64
    }
}

fn leer_fichero(ruta: &Path) -> Result<Arc<Lienzo>, ErrorNucleo> {
    let texto = match std::fs::read_to_string(ruta) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ErrorNucleo::NoExiste {
                que: ruta.display().to_string(),
            });
        }
        Err(e) => return Err(e.into()),
    };
    let es_pixpin2d = ruta
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(pixpin_motor2d::formato::EXTENSION));
    if es_pixpin2d {
        return Lienzo::de_pixpin2d(&texto);
    }
    Lienzo::de_json(texto)
}

impl Lienzo {
    /// Un `.pixpin2d` (el JSON de la `Escena`, como `formato::cargar`). No
    /// lleva fotos aparte.
    fn de_pixpin2d(texto: &str) -> Result<Arc<Lienzo>, ErrorNucleo> {
        let escena: Escena = serde_json::from_str(texto).map_err(|e| ErrorNucleo::Lienzo {
            detalle: e.to_string(),
        })?;
        Ok(Arc::new(Lienzo {
            escena,
            imagenes: Vec::new(),
        }))
    }
}

#[uniffi::export]
impl Nucleo {
    /// El lienzo `dibujo` de un chat (`proyectos/<chat>/lienzos/<dibujo>.excalidraw`,
    /// lo que apunta la `referencia` de un mensaje `DIBUJO`).
    pub fn abrir_lienzo(&self, chat: String, dibujo: String) -> Result<Arc<Lienzo>, ErrorNucleo> {
        for parte in [&chat, &dibujo] {
            if parte.is_empty() || parte.contains(['/', '\\', '\0']) || parte.as_str() == ".." {
                return Err(ErrorNucleo::Argumento {
                    detalle: format!("nombre no valido: {parte:?}"),
                });
            }
        }
        leer_fichero(&pixpin_proyecto::almacen::lienzo(
            self.ruta(),
            &chat,
            &dibujo,
        ))
    }
}

/// Quien escribe el bufer: `f32` en little-endian directamente en bytes,
/// sin un `Vec<f32>` intermedio que doblaria la memoria.
struct Escritor {
    datos: Vec<u8>,
    textos: Vec<String>,
    indice_textos: HashMap<String, u32>,
    imagenes: Vec<u64>,
}

impl Escritor {
    fn numero(&mut self, v: f32) {
        self.datos.extend_from_slice(&v.to_le_bytes());
    }

    fn entero(&mut self, v: usize) {
        self.numero(v as f32);
    }

    fn poner(&mut self, pos: usize, v: f32) {
        self.datos[pos * 4..pos * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn largo(&self) -> usize {
        self.datos.len() / 4
    }

    fn color(&mut self, c: ColorRgba) {
        for v in [c.r, c.g, c.b, c.a] {
            self.numero(v);
        }
    }

    fn puntos(&mut self, p: &[Punto2]) {
        self.entero(p.len());
        self.datos.reserve(p.len() * 8);
        for q in p {
            self.numero(q.x);
            self.numero(q.y);
        }
    }

    /// El indice de un texto en la tabla, sin repetirlo: la familia de letra
    /// es la misma en casi todos.
    fn texto(&mut self, t: &str) -> f32 {
        if let Some(&i) = self.indice_textos.get(t) {
            return i as f32;
        }
        let i = self.textos.len() as u32;
        self.textos.push(t.to_string());
        self.indice_textos.insert(t.to_string(), i);
        i as f32
    }

    /// Abre una orden: escribe `codigo` y un hueco para el largo.
    fn abrir(&mut self, codigo: u32) -> usize {
        let inicio = self.largo();
        self.numero(codigo as f32);
        self.numero(0.0);
        inicio
    }

    fn cerrar(&mut self, inicio: usize) {
        let largo = (self.largo() - inicio) as f32;
        self.poner(inicio + 1, largo);
    }
}

fn estilo_a_numero(e: EstiloTrazo) -> f32 {
    match e {
        EstiloTrazo::Solido => 0.0,
        EstiloTrazo::Discontinuo => 1.0,
        EstiloTrazo::Punteado => 2.0,
        EstiloTrazo::RayaPunto => 3.0,
    }
}

/// Lo que mide de antemano el bufer de unas ordenes (para reservar una vez).
fn largo_estimado(ordenes: &[Orden]) -> usize {
    CABECERA
        + ordenes
            .iter()
            .map(|o| match o {
                Orden::Poligono { puntos, .. }
                | Orden::Tinta {
                    contorno: puntos, ..
                }
                | Orden::Relleno { puntos, .. }
                | Orden::Velo { hueco: puntos, .. } => 7 + puntos.len() * 2,
                Orden::Polilinea { puntos, .. } => 9 + puntos.len() * 2,
                Orden::Texto { .. } => 13,
                Orden::Rotulo { .. } => 19,
                Orden::Imagen { .. } => 16,
            })
            .sum::<usize>()
}

/// Empaqueta `ordenes` en el formato de la doc del modulo.
pub fn empaquetar(
    ordenes: &[Orden],
    fondo: ColorRgba,
    caja: Option<(f32, f32, f32, f32)>,
) -> LienzoPlano {
    let mut w = Escritor {
        datos: Vec::with_capacity(largo_estimado(ordenes) * 4),
        textos: Vec::new(),
        indice_textos: HashMap::new(),
        imagenes: Vec::new(),
    };
    w.numero(VERSION_PLANO as f32);
    w.entero(CABECERA);
    w.entero(ordenes.len());
    w.color(fondo);
    let (x0, y0, x1, y1) = caja.unwrap_or((f32::NAN, f32::NAN, f32::NAN, f32::NAN));
    for v in [x0, y0, x1, y1] {
        w.numero(v);
    }
    w.numero(0.0); // largo total, al final
    debug_assert_eq!(w.largo(), CABECERA);
    for o in ordenes {
        match o {
            Orden::Poligono { puntos, color } => {
                let i = w.abrir(POLIGONO);
                w.color(*color);
                w.puntos(puntos);
                w.cerrar(i);
            }
            Orden::Tinta { contorno, color } => {
                let i = w.abrir(TINTA);
                w.color(*color);
                w.puntos(contorno);
                w.cerrar(i);
            }
            Orden::Polilinea {
                puntos,
                color,
                grosor,
                estilo,
            } => {
                let i = w.abrir(POLILINEA);
                w.color(*color);
                w.numero(*grosor);
                w.numero(estilo_a_numero(*estilo));
                w.puntos(puntos);
                w.cerrar(i);
            }
            Orden::Relleno { puntos, color } => {
                let i = w.abrir(RELLENO);
                w.color(*color);
                w.puntos(puntos);
                w.cerrar(i);
            }
            Orden::Velo { hueco, color } => {
                let i = w.abrir(VELO);
                w.color(*color);
                w.puntos(hueco);
                w.cerrar(i);
            }
            Orden::Texto {
                texto,
                x,
                y,
                tam,
                familia,
                color,
                ancho_max,
                negrita,
                cursiva,
            } => {
                let i = w.abrir(TEXTO);
                w.color(*color);
                for v in [*x, *y, *tam, *ancho_max] {
                    w.numero(v);
                }
                w.numero(f32::from(u8::from(*negrita) | (u8::from(*cursiva) << 1)));
                let t = w.texto(texto);
                w.numero(t);
                let f = w.texto(familia);
                w.numero(f);
                w.cerrar(i);
            }
            Orden::Rotulo {
                texto,
                x,
                y,
                tam,
                familia,
                color,
                halo,
                grosor_halo,
                centro,
                angulo,
            } => {
                let i = w.abrir(ROTULO);
                w.color(*color);
                w.color(*halo);
                for v in [*x, *y, *tam, *grosor_halo, centro.x, centro.y, *angulo] {
                    w.numero(v);
                }
                let t = w.texto(texto);
                w.numero(t);
                let f = w.texto(familia);
                w.numero(f);
                w.cerrar(i);
            }
            Orden::Imagen {
                id_objeto,
                x,
                y,
                ancho,
                alto,
                opacidad,
                recorte,
                angulo,
            } => {
                let i = w.abrir(IMAGEN);
                for v in [*x, *y, *ancho, *alto, *opacidad, *angulo] {
                    w.numero(v);
                }
                let n = w.imagenes.len();
                w.imagenes.push(*id_objeto);
                w.entero(n);
                match recorte {
                    Some(r) => {
                        w.numero(1.0);
                        for v in [r.x, r.y, r.ancho, r.alto, r.ancho_natural, r.alto_natural] {
                            w.numero(v);
                        }
                    }
                    None => {
                        for _ in 0..7 {
                            w.numero(0.0);
                        }
                    }
                }
                w.cerrar(i);
            }
        }
    }
    let total = w.largo() as f32;
    w.poner(CABECERA - 1, total);
    LienzoPlano {
        datos: w.datos,
        textos: w.textos,
        imagenes: w.imagenes,
    }
}

/// Lo que dice la cabecera de un bufer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cabecera {
    pub version: u32,
    pub ordenes: usize,
    pub fondo: ColorRgba,
    pub caja: Option<(f32, f32, f32, f32)>,
}

/// La lectura de referencia del bufer: lo contrario de [`empaquetar`]. Las
/// ordenes de un codigo desconocido se saltan, como tiene que hacer Swift.
pub fn decodificar(plano: &LienzoPlano) -> Result<(Cabecera, Vec<Orden>), String> {
    if plano.datos.len() % 4 != 0 {
        return Err("el bufer no es de f32".into());
    }
    let n: Vec<f32> = plano
        .datos
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    if n.len() < 3 {
        return Err("sin cabecera".into());
    }
    let version = n[0] as u32;
    if version != VERSION_PLANO {
        return Err(format!("version {version} desconocida"));
    }
    let cabecera = n[1] as usize;
    if cabecera < CABECERA || n.len() < cabecera {
        return Err("cabecera corta".into());
    }
    if n[11] as usize != n.len() {
        return Err("largo total distinto".into());
    }
    let color = |i: usize| ColorRgba {
        r: n[i],
        g: n[i + 1],
        b: n[i + 2],
        a: n[i + 3],
    };
    let caja = (!n[7].is_nan()).then(|| (n[7], n[8], n[9], n[10]));
    let cab = Cabecera {
        version,
        ordenes: n[2] as usize,
        fondo: color(3),
        caja,
    };
    let texto = |v: f32| -> Result<String, String> {
        plano
            .textos
            .get(v as usize)
            .cloned()
            .ok_or_else(|| "texto fuera de la tabla".to_string())
    };
    let puntos = |i: usize| -> Result<(Vec<Punto2>, usize), String> {
        let c = *n.get(i).ok_or("puntos cortados")? as usize;
        let fin = i + 1 + c * 2;
        let trozo = n.get(i + 1..fin).ok_or("puntos cortados")?;
        Ok((
            trozo
                .chunks_exact(2)
                .map(|p| Punto2 { x: p[0], y: p[1] })
                .collect(),
            fin,
        ))
    };
    let mut ordenes = Vec::with_capacity(cab.ordenes);
    let mut i = cabecera;
    while i < n.len() {
        let codigo = n[i] as u32;
        let largo = n.get(i + 1).copied().ok_or("orden cortada")? as usize;
        if largo < 2 || i + largo > n.len() {
            return Err(format!("largo {largo} imposible en {i}"));
        }
        let p = i + 2;
        let orden = match codigo {
            POLIGONO | TINTA | RELLENO | VELO => {
                let c = color(p);
                let (pts, _) = puntos(p + 4)?;
                Some(match codigo {
                    POLIGONO => Orden::Poligono {
                        puntos: pts,
                        color: c,
                    },
                    TINTA => Orden::Tinta {
                        contorno: pts,
                        color: c,
                    },
                    RELLENO => Orden::Relleno {
                        puntos: pts,
                        color: c,
                    },
                    _ => Orden::Velo {
                        hueco: pts,
                        color: c,
                    },
                })
            }
            POLILINEA => {
                let (pts, _) = puntos(p + 6)?;
                Some(Orden::Polilinea {
                    color: color(p),
                    grosor: n[p + 4],
                    estilo: match n[p + 5] as u32 {
                        1 => EstiloTrazo::Discontinuo,
                        2 => EstiloTrazo::Punteado,
                        _ => EstiloTrazo::Solido,
                    },
                    puntos: pts,
                })
            }
            TEXTO => {
                let caras = n[p + 8] as u32;
                Some(Orden::Texto {
                    color: color(p),
                    x: n[p + 4],
                    y: n[p + 5],
                    tam: n[p + 6],
                    ancho_max: n[p + 7],
                    negrita: caras & 1 != 0,
                    cursiva: caras & 2 != 0,
                    texto: texto(n[p + 9])?,
                    familia: texto(n[p + 10])?,
                })
            }
            ROTULO => Some(Orden::Rotulo {
                color: color(p),
                halo: color(p + 4),
                x: n[p + 8],
                y: n[p + 9],
                tam: n[p + 10],
                grosor_halo: n[p + 11],
                centro: Punto2 {
                    x: n[p + 12],
                    y: n[p + 13],
                },
                angulo: n[p + 14],
                texto: texto(n[p + 15])?,
                familia: texto(n[p + 16])?,
            }),
            IMAGEN => Some(Orden::Imagen {
                x: n[p],
                y: n[p + 1],
                ancho: n[p + 2],
                alto: n[p + 3],
                opacidad: n[p + 4],
                angulo: n[p + 5],
                id_objeto: *plano
                    .imagenes
                    .get(n[p + 6] as usize)
                    .ok_or("imagen fuera de la tabla")?,
                recorte: (n[p + 7] == 1.0).then(|| RecorteImagen {
                    x: n[p + 8],
                    y: n[p + 9],
                    ancho: n[p + 10],
                    alto: n[p + 11],
                    ancho_natural: n[p + 12],
                    alto_natural: n[p + 13],
                }),
            }),
            // Un codigo de una version mas nueva: se salta entero.
            _ => None,
        };
        ordenes.extend(orden);
        i += largo;
    }
    Ok((cab, ordenes))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn p(x: f32, y: f32) -> Punto2 {
        Punto2 { x, y }
    }

    const ROJO: ColorRgba = ColorRgba {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Una orden de cada clase: todas tienen que volver iguales.
    #[test]
    fn todas_las_ordenes_van_y_vuelven() {
        let ordenes = vec![
            Orden::Poligono {
                puntos: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
                color: ROJO,
            },
            Orden::Tinta {
                contorno: vec![p(2.5, 3.5), p(4.0, 5.0)],
                color: ROJO,
            },
            Orden::Polilinea {
                puntos: vec![p(-1.0, -2.0), p(3.0, 4.0)],
                color: ROJO,
                grosor: 2.5,
                estilo: EstiloTrazo::Punteado,
            },
            Orden::Relleno {
                puntos: vec![],
                color: ROJO,
            },
            Orden::Velo {
                hueco: vec![p(1.0, 1.0)],
                color: ROJO,
            },
            Orden::Texto {
                texto: "Hola ñandú".into(),
                x: 1.0,
                y: 2.0,
                tam: 20.0,
                familia: "Excalifont".into(),
                color: ROJO,
                ancho_max: f32::MAX,
                negrita: true,
                cursiva: false,
            },
            Orden::Rotulo {
                texto: "5,00 cm".into(),
                x: 1.0,
                y: 2.0,
                tam: 12.0,
                familia: "Excalifont".into(),
                color: ROJO,
                halo: ColorRgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.5,
                },
                grosor_halo: 3.0,
                centro: p(7.0, 8.0),
                angulo: 0.75,
            },
            Orden::Imagen {
                id_objeto: u64::MAX - 3,
                x: 0.0,
                y: 0.0,
                ancho: 100.0,
                alto: 50.0,
                opacidad: 0.8,
                recorte: Some(RecorteImagen {
                    x: 1.0,
                    y: 2.0,
                    ancho: 3.0,
                    alto: 4.0,
                    ancho_natural: 300.0,
                    alto_natural: 400.0,
                }),
                angulo: 0.1,
            },
            Orden::Imagen {
                id_objeto: 7,
                x: 0.0,
                y: 0.0,
                ancho: 10.0,
                alto: 5.0,
                opacidad: 1.0,
                recorte: None,
                angulo: 0.0,
            },
        ];
        let plano = empaquetar(&ordenes, ROJO, Some((0.0, 1.0, 2.0, 3.0)));
        // La familia, una sola vez en la tabla.
        assert_eq!(plano.textos, vec!["Hola ñandú", "Excalifont", "5,00 cm"]);
        assert_eq!(plano.imagenes, vec![u64::MAX - 3, 7]);
        let (cab, vuelta) = decodificar(&plano).unwrap();
        assert_eq!(cab.version, VERSION_PLANO);
        assert_eq!(cab.ordenes, ordenes.len());
        assert_eq!(cab.caja, Some((0.0, 1.0, 2.0, 3.0)));
        assert_eq!(vuelta, ordenes);
        // La estimacion reserva justo lo que se escribe.
        assert_eq!(plano.datos.len(), largo_estimado(&ordenes) * 4);
    }

    #[test]
    fn un_codigo_desconocido_se_salta_entero() {
        let ordenes = vec![
            Orden::Relleno {
                puntos: vec![p(1.0, 2.0)],
                color: ROJO,
            },
            Orden::Velo {
                hueco: vec![p(3.0, 4.0)],
                color: ROJO,
            },
        ];
        let mut plano = empaquetar(&ordenes, ROJO, None);
        // La primera orden pasa a ser de un codigo que no existe (99).
        let pos = CABECERA * 4;
        plano.datos[pos..pos + 4].copy_from_slice(&99f32.to_le_bytes());
        let (cab, vuelta) = decodificar(&plano).unwrap();
        assert_eq!(cab.caja, None);
        assert_eq!(vuelta, ordenes[1..].to_vec());
    }

    #[test]
    fn una_version_desconocida_no_se_lee() {
        let mut plano = empaquetar(&[], ROJO, None);
        plano.datos[0..4].copy_from_slice(&2f32.to_le_bytes());
        assert!(decodificar(&plano).is_err());
    }

    #[test]
    fn un_pixpin2d_se_lee_por_su_extension() {
        let dir = std::env::temp_dir().join(format!("pixpin-nucleo-p2d-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let l = excalidraw::leer(
            r#"{"elements":[{"id":"x","type":"rectangle","x":0,"y":0,"width":80,"height":40}]}"#,
        )
        .unwrap();
        let escena = excalidraw::a_escena(&l);
        let ruta = dir.join("dibujo.pixpin2d");
        pixpin_motor2d::formato::guardar(&ruta, &escena).unwrap();
        let leido = Lienzo::de_fichero(ruta.display().to_string()).unwrap();
        assert_eq!(leido.cuantos_elementos(), 1);
        assert!(leido.imagenes().is_empty());
        assert!(decodificar(&leido.plano()).is_ok());
        // Caso negativo: un .pixpin2d roto da error de lienzo, sin panico.
        let roto = dir.join("roto.pixpin2d");
        std::fs::write(&roto, b"{roto").unwrap();
        assert!(matches!(
            Lienzo::de_fichero(roto.display().to_string()),
            Err(ErrorNucleo::Lienzo { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
