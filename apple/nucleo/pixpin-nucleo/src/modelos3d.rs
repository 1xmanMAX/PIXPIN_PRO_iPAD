//! **Los modelos 3D para Metal**: IFC, Revit, LandXML de Civil 3D, ficheros
//! de puntos y un DWG/DXF visto en 3D (`pixpin_bim::convertir_fichero`, lo
//! mismo que lee el visor 3D de Windows). Cruza a Swift como en
//! [`crate::planos`]: buferes de bytes en little-endian que Metal sube tal
//! cual.
//!
//! - `vertices`: `V3` de 24 bytes: `pos 3 f32` (respecto al origen del
//!   modelo, Z arriba), `normal u32` (3 bytes con signo x, y, z; el byte alto
//!   lleva la esquina de un punto y las marcas del croquis), `color u32`
//!   (RGBA, rojo en el byte bajo) y `elemento u32`.
//! - los indices ([`Indices3d`]), `u32`: triangulos opacos y transparentes
//!   (de tres en tres), aristas vivas y rayas (de dos en dos) y puntos (seis
//!   por punto: dos triangulos que el sombreador abre en pantalla).
//!
//! Lo que se ve se filtra **rehaciendo solo los indices**
//! ([`Modelo3dApple::visibles`], `Modelo3d::visibles`): los vertices se suben
//! una vez. La camara es la de Windows ([`pixpin_cad::orbita::Orbita`]): sus
//! funciones van sueltas ([`camara_matriz`]...) y son baratas.
//!
//! Elegir lo que hay bajo el raton se hace aqui, en la CPU: un rayo de la
//! camara ([`camara_rayo`]) contra los triangulos que se ven
//! ([`Modelo3dApple::elegir`]). Asi no hace falta otra pasada de la tarjeta
//! ni leer un pixel de vuelta.
//!
//! El terreno de Civil 3D ya viene coloreado por cota y con sus curvas de
//! nivel (`pixpin_bim::civil`): Metal solo pinta los colores que trae.

use std::path::Path;
use std::sync::Arc;

use pixpin_cad::modelo3d::{Modelo3d, V3, tipo_legible};
use pixpin_cad::orbita::Orbita;

use crate::{ErrorNucleo, Nucleo};

/// Un punto (o una direccion) del modelo, respecto a su origen.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Punto3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Una caja, respecto al origen del modelo.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Caja3d {
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
}

/// Lo que se sabe del modelo al abrirlo.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ResumenModelo3d {
    /// El origen de las coordenadas `f32` (en el mundo del fichero).
    pub origen: Punto3d,
    pub caja: Caja3d,
    /// Sin los pocos puntos sueltos muy lejos: para encuadrar.
    pub caja_util: Caja3d,
    /// 1 si las medidas estan en metros.
    pub metros: f32,
    pub vertices: u64,
    pub elementos: u32,
    pub vacio: bool,
}

/// Los indices de lo que se ve (ver la doc del modulo).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Indices3d {
    pub opacos: Vec<u8>,
    pub transparentes: Vec<u8>,
    pub aristas: Vec<u8>,
    pub lineas: Vec<u8>,
    pub puntos: Vec<u8>,
}

/// Un nivel (planta) del edificio.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Nivel3d {
    pub indice: u32,
    pub nombre: String,
    /// La cota, en metros.
    pub cota: f64,
    /// Cuantos elementos tiene.
    pub elementos: u32,
}

/// Una categoria: los elementos de una clase (`IfcWall`, «Curvas de nivel»).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Categoria3d {
    /// La clase tal cual: lo que se pasa en [`Filtro3d::tipos_ocultos`].
    pub tipo: String,
    /// En espanol (`Muro`).
    pub nombre: String,
    pub elementos: u32,
}

/// Lo que no se ve.
#[derive(Debug, Clone, PartialEq, Eq, Default, uniffi::Record)]
pub struct Filtro3d {
    /// Los niveles ocultos (su indice).
    pub niveles_ocultos: Vec<u32>,
    /// Las clases ocultas ([`Categoria3d::tipo`]).
    pub tipos_ocultos: Vec<String>,
    /// Si hay uno aislado: solo se ve ese.
    pub aislado: Option<u32>,
}

/// Lo que se cuenta de un elemento al tocarlo.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct InfoElemento3d {
    pub id: u32,
    pub tipo: String,
    pub categoria: String,
    pub nombre: String,
    pub nivel: Option<String>,
    /// Lo que mide, de su malla (en las unidades del modelo).
    pub volumen: f32,
    pub planta: f32,
    pub alzado: f32,
    pub superficie: f32,
    /// Si la malla es cerrada: solo entonces el volumen es exacto.
    pub cerrado: bool,
}

/// Lo que hay bajo el raton.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Eleccion3d {
    pub elemento: u32,
    /// El punto tocado, respecto al origen del modelo.
    pub punto: Punto3d,
    pub distancia: f64,
}

/// La camara del visor 3D (`pixpin_cad::orbita::Orbita`).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Camara3d {
    pub objetivo: Punto3d,
    /// Alrededor de Z, en radianes.
    pub rumbo: f64,
    /// Sobre el horizonte, en radianes.
    pub altura: f64,
    pub dist: f64,
}

// ------------------------------------------------------------- bytes

fn v3(v: &V3) -> [u32; 6] {
    [
        v.pos[0].to_bits(),
        v.pos[1].to_bits(),
        v.pos[2].to_bits(),
        v.normal,
        v.color,
        v.elemento,
    ]
}

fn indices(v: &[u32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(v.len() * 4);
    for i in v {
        b.extend_from_slice(&i.to_le_bytes());
    }
    b
}

/// Los vertices de un modelo, en bytes (24 por vertice).
pub fn vertices_en_bytes(v: &[V3]) -> Vec<u8> {
    let mut b = Vec::with_capacity(v.len() * 24);
    for x in v {
        for w in v3(x) {
            b.extend_from_slice(&w.to_le_bytes());
        }
    }
    b
}

fn caja3d(c: [f32; 6]) -> Caja3d {
    Caja3d {
        x0: c[0],
        y0: c[1],
        z0: c[2],
        x1: c[3],
        y1: c[4],
        z1: c[5],
    }
}

fn de_caja3d(c: Caja3d) -> [f32; 6] {
    [c.x0, c.y0, c.z0, c.x1, c.y1, c.z1]
}

// ------------------------------------------------------------- el modelo

/// **Un modelo 3D abierto**, listo para Metal.
#[derive(uniffi::Object)]
pub struct Modelo3dApple {
    m: Modelo3d,
    /// Lo que pide un croquis del movil (su fondo, su tema, que encuadrar y
    /// desde donde mira). `None` en un modelo de un fichero.
    croquis: Option<ExtrasCroquis>,
    /// Las vistas guardadas del croquis (las congeladas en el movil): con
    /// las teclas 1 a 9 (`Extras::vistas` de Windows). Vacio en un modelo.
    vistas: Vec<pixpin_cad::croquis::Vista3D>,
}

/// Lo que se abre como modelo 3D o como plano (para el chat y el Finder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ClaseDeModelo {
    /// Un DWG o un DXF: el visor de planos.
    Plano,
    /// IFC, Revit, LandXML o un fichero de puntos: el visor 3D.
    Modelo3d,
    /// Otra cosa.
    Ninguna,
}

/// **Que visor abre este fichero**: por la extension (DWG, DXF, IFC, RVT)
/// o, para un `.xml`, `.csv` o `.txt`, mirando el principio del contenido
/// (un LandXML, coordenadas de puntos). Lee como mucho 8 KB.
#[uniffi::export]
pub fn clase_de_modelo(ruta: String) -> ClaseDeModelo {
    let ruta = Path::new(&ruta);
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(ext.as_str(), "dwg" | "dxf") {
        ClaseDeModelo::Plano
    } else if pixpin_bim::se_abre(&ext)
        || (ext == "landxml" && pixpin_bim::civil::es_landxml(ruta))
        || pixpin_bim::es_de_civil(ruta)
    {
        ClaseDeModelo::Modelo3d
    } else {
        ClaseDeModelo::Ninguna
    }
}

impl Modelo3dApple {
    fn ver(&self, filtro: &Filtro3d) -> impl Fn(u32) -> bool + '_ {
        let f = filtro.clone();
        move |e: u32| {
            if let Some(a) = f.aislado {
                return e == a;
            }
            let Some(el) = self.m.elementos.get(e as usize) else {
                return true;
            };
            !el.nivel.is_some_and(|n| f.niveles_ocultos.contains(&n))
                && !f.tipos_ocultos.contains(&el.tipo)
        }
    }

    /// Para las pruebas y para quien ya tiene el modelo leido.
    pub fn de_modelo(m: Modelo3d) -> Arc<Modelo3dApple> {
        Arc::new(Modelo3dApple {
            m,
            croquis: None,
            vistas: Vec::new(),
        })
    }
}

#[uniffi::export]
impl Modelo3dApple {
    /// **Abre un modelo 3D**: IFC, Revit, LandXML, puntos, o un DWG/DXF en
    /// 3D. Bloquea (un IFC grande tarda): Swift lo llama desde una `Task`.
    /// Leer un IFC usa un hilo aparte con pila grande (sus booleanas
    /// recurren hondo), que termina al terminar de leer.
    #[uniffi::constructor]
    pub fn abrir(ruta: String) -> Result<Arc<Modelo3dApple>, ErrorNucleo> {
        Ok(Modelo3dApple::de_modelo(leer_modelo(Path::new(&ruta))?))
    }

    /// Lo que pide un croquis (ver [`ExtrasCroquis`]); `None` si no lo es.
    pub fn extras_croquis(&self) -> Option<ExtrasCroquis> {
        self.croquis
    }

    /// **Los nombres de las vistas guardadas** del croquis (1 a 9, en su
    /// orden). Vacio en un modelo de un fichero.
    pub fn vistas_guardadas(&self) -> Vec<String> {
        self.vistas.iter().map(|v| v.nombre.clone()).collect()
    }

    /// **La camara de la vista guardada `indice`** (desde 0) en una ventana
    /// de `ancho` x `alto` pixeles con `escala` pixeles por punto, como la
    /// mira el movil (`croquis::orbita_de` de Windows). `None` si no hay.
    pub fn camara_de_vista(
        &self,
        indice: u32,
        ancho: u32,
        alto: u32,
        escala: f64,
    ) -> Option<Camara3d> {
        let v = self.vistas.get(indice as usize)?;
        let e = if escala.is_finite() && escala > 0.0 {
            escala
        } else {
            1.0
        };
        Some(camara(pixpin_cad::croquis::orbita_de(
            v,
            self.m.origen,
            ancho,
            alto,
            e,
        )))
    }

    pub fn resumen(&self) -> ResumenModelo3d {
        let o = self.m.origen;
        ResumenModelo3d {
            origen: Punto3d {
                x: o[0],
                y: o[1],
                z: o[2],
            },
            caja: caja3d(self.m.caja),
            caja_util: caja3d(self.m.caja_util()),
            metros: self.m.metros,
            vertices: self.m.vertices.len() as u64,
            elementos: self.m.elementos.len() as u32,
            vacio: self.m.vacio(),
        }
    }

    /// Los vertices (24 bytes cada uno). Se suben una vez.
    pub fn vertices(&self) -> Vec<u8> {
        vertices_en_bytes(&self.m.vertices)
    }

    /// Todos los indices.
    pub fn indices(&self) -> Indices3d {
        Indices3d {
            opacos: indices(&self.m.opacos),
            transparentes: indices(&self.m.transparentes),
            aristas: indices(&self.m.aristas),
            lineas: indices(&self.m.lineas),
            puntos: indices(&self.m.puntos),
        }
    }

    /// Los indices de lo que se ve con `filtro`.
    pub fn visibles(&self, filtro: Filtro3d) -> Indices3d {
        let v = self.m.visibles(&self.ver(&filtro));
        Indices3d {
            opacos: indices(&v.opacos),
            transparentes: indices(&v.transparentes),
            aristas: indices(&v.aristas),
            lineas: indices(&v.lineas),
            puntos: indices(&v.puntos),
        }
    }

    /// La clave estable de un elemento (GlobalId o `~tipo|nombre|k`), la
    /// de `aislado_clave` en los marcadores de Windows y Android.
    pub fn clave_de_elemento(&self, id: u32) -> Option<String> {
        self.m.clave_de_elemento(id)
    }

    /// El elemento de esa clave en este modelo.
    pub fn elemento_de_clave(&self, clave: String) -> Option<u32> {
        self.m.elemento_de_clave(&clave)
    }

    /// La clave estable del nivel `k` (`n:=<clave>` en los marcadores).
    pub fn clave_de_nivel(&self, k: u32) -> Option<String> {
        self.m.clave_de_nivel(k)
    }

    /// El nivel de esa clave en este modelo.
    pub fn nivel_de_clave(&self, clave: String) -> Option<u32> {
        self.m.nivel_de_clave(&clave)
    }

    /// Los niveles, de abajo arriba (vacio si el modelo no los tiene).
    pub fn niveles(&self) -> Vec<Nivel3d> {
        let mut cuantos = vec![0u32; self.m.niveles.len()];
        for e in &self.m.elementos {
            if let Some(n) = e.nivel.and_then(|n| cuantos.get_mut(n as usize)) {
                *n += 1;
            }
        }
        self.m
            .niveles
            .iter()
            .enumerate()
            .map(|(i, n)| Nivel3d {
                indice: i as u32,
                nombre: n.nombre.clone(),
                cota: n.cota,
                elementos: cuantos[i],
            })
            .collect()
    }

    /// Las categorias, por su nombre en espanol.
    pub fn categorias(&self) -> Vec<Categoria3d> {
        let mut por_tipo: std::collections::BTreeMap<&str, u32> = Default::default();
        for e in &self.m.elementos {
            *por_tipo.entry(e.tipo.as_str()).or_default() += 1;
        }
        let mut v: Vec<Categoria3d> = por_tipo
            .into_iter()
            .map(|(t, n)| Categoria3d {
                tipo: t.to_string(),
                nombre: tipo_legible(t),
                elementos: n,
            })
            .collect();
        v.sort_by(|a, b| {
            a.nombre
                .to_lowercase()
                .cmp(&b.nombre.to_lowercase())
                .then(a.tipo.cmp(&b.tipo))
        });
        v
    }

    /// Lo que se cuenta del elemento `id` (el `elemento` de sus vertices).
    pub fn elemento(&self, id: u32) -> Option<InfoElemento3d> {
        let e = self.m.elementos.get(id as usize)?;
        let md = e.medidas;
        Some(InfoElemento3d {
            id,
            tipo: e.tipo.clone(),
            categoria: tipo_legible(&e.tipo),
            nombre: e.nombre.clone(),
            nivel: e
                .nivel
                .and_then(|n| self.m.niveles.get(n as usize))
                .map(|n| n.nombre.clone()),
            volumen: md.volumen,
            planta: md.planta,
            alzado: md.alzado,
            superficie: md.superficie,
            cerrado: md.cerrado,
        })
    }

    /// **Lo que hay bajo el raton**: el triangulo mas cercano que corta el
    /// rayo desde `origen` hacia `direccion` (respecto al origen del modelo;
    /// ver [`camara_ojo`] y [`camara_rayo`]), entre lo que se ve con
    /// `filtro` y, con caja de seccion, solo dentro de ella.
    pub fn elegir(
        &self,
        origen: Punto3d,
        direccion: Punto3d,
        filtro: Filtro3d,
        corte: Option<Caja3d>,
    ) -> Option<Eleccion3d> {
        let ver = self.ver(&filtro);
        let o = [origen.x, origen.y, origen.z];
        let d = [direccion.x, direccion.y, direccion.z];
        let corte = corte.map(de_caja3d);
        let mut mejor: Option<(f64, u32)> = None;
        let vs = &self.m.vertices;
        for lista in [&self.m.opacos, &self.m.transparentes] {
            for t in lista.chunks_exact(3) {
                let (Some(a), Some(b), Some(c)) = (
                    vs.get(t[0] as usize),
                    vs.get(t[1] as usize),
                    vs.get(t[2] as usize),
                ) else {
                    continue;
                };
                if !ver(a.elemento) {
                    continue;
                }
                let Some(dist) = cortar_triangulo(o, d, a.pos, b.pos, c.pos) else {
                    continue;
                };
                if mejor.is_some_and(|(m, _)| dist >= m) {
                    continue;
                }
                if let Some(k) = corte {
                    let p = [o[0] + d[0] * dist, o[1] + d[1] * dist, o[2] + d[2] * dist];
                    if (0..3).any(|i| p[i] < k[i] as f64 || p[i] > k[i + 3] as f64) {
                        continue;
                    }
                }
                mejor = Some((dist, a.elemento));
            }
        }
        let (dist, e) = mejor?;
        Some(Eleccion3d {
            elemento: e,
            punto: Punto3d {
                x: o[0] + d[0] * dist,
                y: o[1] + d[1] * dist,
                z: o[2] + d[2] * dist,
            },
            distancia: dist,
        })
    }
}

/// Moller-Trumbore, por las dos caras: a que distancia corta el rayo al
/// triangulo (con `d` unitario), o `None`.
fn cortar_triangulo(
    o: [f64; 3],
    d: [f64; 3],
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) -> Option<f64> {
    let a = [a[0] as f64, a[1] as f64, a[2] as f64];
    let e1 = [b[0] as f64 - a[0], b[1] as f64 - a[1], b[2] as f64 - a[2]];
    let e2 = [c[0] as f64 - a[0], c[1] as f64 - a[1], c[2] as f64 - a[2]];
    let cruz = |u: [f64; 3], v: [f64; 3]| {
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    };
    let punto = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let p = cruz(d, e2);
    let det = punto(e1, p);
    if det.abs() < 1e-18 {
        return None;
    }
    let inv = 1.0 / det;
    let s = [o[0] - a[0], o[1] - a[1], o[2] - a[2]];
    let u = punto(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cruz(s, e1);
    let v = punto(d, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = punto(e2, q) * inv;
    (t > 0.0).then_some(t)
}

// ------------------------------------------------------------- la camara

fn orbita(c: Camara3d) -> Orbita {
    Orbita {
        objetivo: [c.objetivo.x, c.objetivo.y, c.objetivo.z],
        rumbo: c.rumbo,
        altura: c.altura,
        dist: c.dist,
    }
}

fn camara(o: Orbita) -> Camara3d {
    Camara3d {
        objetivo: p3(o.objetivo),
        rumbo: o.rumbo,
        altura: o.altura,
        dist: o.dist,
    }
}

fn p3(p: [f64; 3]) -> Punto3d {
    Punto3d {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}

fn de_p3(p: Punto3d) -> [f64; 3] {
    [p.x, p.y, p.z]
}

/// La caja entera en la ventana, vista desde el sureste y algo de arriba.
#[uniffi::export]
pub fn camara_encuadrar(caja: Caja3d, ancho: u32, alto: u32) -> Camara3d {
    camara(Orbita::encuadrar(de_caja3d(caja), ancho, alto))
}

/// De mundo (respecto al origen del modelo) a recorte: 16 `f32` **por
/// filas** (`p * M`, el convenio de D3D). Cargados tal cual en un
/// `float4x4` de Metal (que va por columnas), `mvp * float4(p, 1)` da lo
/// mismo. La profundidad va de 0 a 1, como en Metal.
#[uniffi::export]
pub fn camara_matriz(camara: Camara3d, ancho: u32, alto: u32, radio: f64) -> Vec<f32> {
    orbita(camara)
        .matriz(ancho, alto, radio)
        .iter()
        .flatten()
        .copied()
        .collect()
}

/// Donde esta el ojo.
#[uniffi::export]
pub fn camara_ojo(camara: Camara3d) -> Punto3d {
    p3(orbita(camara).ojo())
}

/// La direccion (unitaria) del rayo que sale del ojo por el pixel (x, y)
/// (desde arriba a la izquierda).
#[uniffi::export]
pub fn camara_rayo(camara: Camara3d, x: f64, y: f64, ancho: u32, alto: u32) -> Punto3d {
    p3(orbita(camara).rayo(x, y, ancho, alto))
}

/// Acercar `factor` (<1 acerca) dejando quieto `fijo` en la pantalla.
#[uniffi::export]
pub fn camara_zoom(camara: Camara3d, factor: f64, fijo: Punto3d) -> Camara3d {
    self::camara(orbita(camara).zoom(factor, de_p3(fijo)))
}

/// Girar alrededor de `pivote` (rumbo y altura en radianes).
#[uniffi::export]
pub fn camara_girar(camara: Camara3d, d_rumbo: f64, d_altura: f64, pivote: Punto3d) -> Camara3d {
    self::camara(orbita(camara).girar(d_rumbo, d_altura, de_p3(pivote)))
}

/// Mover la camara `dx`, `dy` pixeles (el modelo sigue al dedo).
#[uniffi::export]
pub fn camara_mover(camara: Camara3d, dx: f64, dy: f64, alto: u32) -> Camara3d {
    self::camara(orbita(camara).mover(dx, dy, alto))
}

/// Un paso de la animacion hacia `destino` (`k` de 0 a 1).
#[uniffi::export]
pub fn camara_hacia(camara: Camara3d, destino: Camara3d, k: f64) -> Camara3d {
    self::camara(orbita(camara).hacia(&orbita(destino), k))
}

/// Si ya casi se llego (para acabar la animacion).
#[uniffi::export]
pub fn camara_cerca_de(camara: Camara3d, destino: Camara3d) -> bool {
    orbita(camara).cerca_de(&orbita(destino))
}

/// **Lee un modelo 3D** (IFC, Revit, LandXML, puntos, DWG/DXF en 3D).
fn leer_modelo(ruta: &Path) -> Result<Modelo3d, ErrorNucleo> {
    if !ruta.is_file() {
        return Err(ErrorNucleo::NoExiste {
            que: ruta.display().to_string(),
        });
    }
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    // Un `.landxml`: `convertir_fichero` solo mira la extension `xml`.
    let m = if ext == "landxml" {
        let t = std::fs::read(ruta)?;
        pixpin_bim::civil::de_landxml(&String::from_utf8_lossy(&t))
    } else {
        pixpin_bim::convertir_fichero(ruta)
    }
    .map_err(|e| ErrorNucleo::Argumento { detalle: e })?;
    if m.vacio() {
        return Err(ErrorNucleo::Argumento {
            detalle: format!("{}: no hay nada que dibujar", ruta.display()),
        });
    }
    Ok(m)
}

// ------------------------------------------------------------- la cache

/// Cuantos modelos leidos se guardan (los mas recientes), como Windows.
const EN_CACHE: usize = 16;

/// **El modelo de `ruta`, de la cache o leyendolo** (`modelo_bim::cargar`
/// de Windows): `<raiz>/cache/bim/<clave>.px3d`, con la clave de los planos
/// (si el fichero cambia, cambia). Solo vive en este Mac.
pub(crate) fn modelo_con_cache(raiz: &Path, ruta: &Path) -> Result<Modelo3d, ErrorNucleo> {
    let carpeta = raiz.join("cache").join("bim");
    let destino = crate::planos::clave(ruta).map(|k| carpeta.join(format!("{k}.px3d")));
    if let Some(d) = &destino
        && let Ok(b) = std::fs::read(d)
        && let Some(m) = Modelo3d::de_bytes(&b)
    {
        // Tocarlo: es de los recientes.
        if let Ok(f) = std::fs::File::options().append(true).open(d) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
        return Ok(m);
    }
    let m = leer_modelo(ruta)?;
    if let Some(d) = destino
        && std::fs::create_dir_all(&carpeta).is_ok()
    {
        let tmp = d.with_extension("tmp");
        if std::fs::write(&tmp, m.a_bytes()).is_ok() && std::fs::rename(&tmp, &d).is_ok() {
            podar(&carpeta);
        }
    }
    Ok(m)
}

/// Deja los `EN_CACHE` modelos mas recientes.
fn podar(carpeta: &Path) {
    let Ok(d) = std::fs::read_dir(carpeta) else {
        return;
    };
    let mut v: Vec<(std::time::SystemTime, std::path::PathBuf)> = d
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "px3d"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (_, p) in v.into_iter().skip(EN_CACHE) {
        let _ = std::fs::remove_file(p);
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Abre un modelo 3D con la cache** de esta carpeta de datos: la
    /// segunda vez el mismo fichero se abre casi al momento. Bloquea.
    pub fn abrir_modelo(&self, ruta: String) -> Result<Arc<Modelo3dApple>, ErrorNucleo> {
        Ok(Modelo3dApple::de_modelo(modelo_con_cache(
            self.ruta(),
            Path::new(&ruta),
        )?))
    }
}

// ------------------------------------------------------------- croquis

/// La clase de un mensaje con un croquis 3D del movil (`Clase.CROQUIS`).
pub const CLASE_CROQUIS: &str = "CROQUIS";

/// Un color en tanto por uno.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Color3d {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// **Lo que un croquis del movil le dice al visor** (`croquis3d::extras_de`
/// de Windows): su fondo si eligio uno, si abre en claro, que encuadrar (lo
/// dibujado, sin el suelo ni las hojas sin fin) y desde donde mira de
/// fabrica (`Camara3D()` del movil).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ExtrasCroquis {
    pub fondo: Option<Color3d>,
    pub claro: bool,
    /// Respecto al origen del modelo. `None` si no hay nada dibujado.
    pub encuadre: Option<Caja3d>,
    pub rumbo: f64,
    pub altura: f64,
}

/// Lo que hace falta de un croquis leido para el visor.
pub fn extras_de(c: &pixpin_cad::croquis::Croquis, origen: [f64; 3]) -> ExtrasCroquis {
    use pixpin_cad::croquis::{Camara3D, malla};
    let fabrica = Camara3D::default();
    ExtrasCroquis {
        fondo: malla::fondo_propio(c).map(|[r, g, b, a]| Color3d { r, g, b, a }),
        claro: malla::abre_claro(c),
        encuadre: malla::caja_de_lo_dibujado(c, origen).map(caja3d),
        rumbo: -fabrica.giro - std::f64::consts::FRAC_PI_2,
        altura: fabrica.inclinacion,
    }
}

/// Donde esta en este Mac el croquis `id` de un chat:
/// `croquis3d/<id>.croquis.gz` de su carpeta del movil, donde lo deja la
/// sincronizacion. `None` si no esta o si el id intenta salirse.
pub fn fichero_del_croquis(raiz: &Path, chat: &str, id: &str) -> Option<std::path::PathBuf> {
    let id = id.trim();
    if id.is_empty() || id.contains(['/', '\\', '\0']) || id.contains("..") {
        return None;
    }
    pixpin_proyecto::vista::ruta_real(
        raiz,
        chat,
        &format!("pixpin:files/croquis3d/{id}.croquis.gz"),
    )
    .filter(|r| r.is_file())
}

/// **Lee un croquis y lo pasa a triangulos**. Sus fotos (`pixpin:files/…`)
/// se buscan como las de cualquier mensaje de `chat`.
pub fn modelo_de_croquis(raiz: &Path, chat: &str, bytes: &[u8]) -> Result<Modelo3dApple, String> {
    use pixpin_cad::croquis::{self, malla};
    let c = croquis::leer(bytes)?;
    let fotos = |r: &str| -> Option<malla::Foto> {
        let ruta = pixpin_proyecto::vista::ruta_real(raiz, chat, r)?;
        let im = pixpin_codec::imagen::cargar(&ruta).ok()?;
        Some(malla::Foto {
            ancho: im.ancho,
            alto: im.alto,
            rgba: im.pixeles,
        })
    };
    let m = malla::a_modelo(&c, &fotos);
    let extras = extras_de(&c, m.origen);
    Ok(Modelo3dApple {
        m,
        croquis: Some(extras),
        vistas: c.vistas,
    })
}

/// La camara que encuadra `caja` mirando desde `rumbo` y `altura` (la de
/// fabrica de un croquis), lo bastante lejos para que quepa entera: la
/// esfera que la envuelve, en el lado mas corto de la ventana.
#[uniffi::export]
pub fn camara_encuadrar_desde(
    caja: Caja3d,
    ancho: u32,
    alto: u32,
    rumbo: f64,
    altura: f64,
) -> Camara3d {
    // El campo de vision de `Orbita` (45 grados en vertical).
    const FOV: f64 = std::f64::consts::FRAC_PI_4;
    let mut o = Orbita::encuadrar(de_caja3d(caja), ancho, alto);
    o.rumbo = rumbo;
    o.altura = altura.clamp(-1.5697, 1.5697);
    let d = [
        f64::from(caja.x1 - caja.x0),
        f64::from(caja.y1 - caja.y0),
        f64::from(caja.z1 - caja.z0),
    ];
    let radio = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() / 2.0;
    let mitad_v = FOV / 2.0;
    let mitad_h = (mitad_v.tan() * f64::from(ancho.max(1)) / f64::from(alto.max(1))).atan();
    let lado = mitad_v.min(mitad_h).sin().max(1e-3);
    o.dist = (radio / lado / 0.94).max(1e-3);
    camara(o)
}

#[uniffi::export]
impl Nucleo {
    /// **El fichero del croquis 3D `referencia` de un chat**, si ya llego a
    /// este Mac (`croquis3d::fichero` de Windows).
    pub fn croquis_del_chat(&self, chat: String, referencia: String) -> Option<String> {
        crate::chats::validar_chat(&chat).ok()?;
        fichero_del_croquis(self.ruta(), &chat, &referencia).map(|r| r.display().to_string())
    }

    /// **Abre el croquis 3D `referencia` de un chat** para el visor: lo lee
    /// y lo pasa a triangulos (con sus fotos). Solo se ve: el fichero no se
    /// escribe nunca. Bloquea: Swift lo llama desde una `Task`.
    pub fn abrir_croquis(
        &self,
        chat: String,
        referencia: String,
    ) -> Result<Arc<Modelo3dApple>, ErrorNucleo> {
        crate::chats::validar_chat(&chat)?;
        let ruta =
            fichero_del_croquis(self.ruta(), &chat, &referencia).ok_or(ErrorNucleo::NoExiste {
                que: format!("croquis {referencia}"),
            })?;
        let bytes = std::fs::read(&ruta)?;
        let m = modelo_de_croquis(self.ruta(), &chat, &bytes)
            .map_err(|detalle| ErrorNucleo::Argumento { detalle })?;
        Ok(Arc::new(m))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_cad::modelo3d::Constructor3d;

    fn puntos() -> Arc<Modelo3dApple> {
        let texto =
            "1,100,200,10,A\n2,100,210,11,B\n3,110,200,12,C\n4,110,210,10,D\n5,105,205,15,E\n";
        Modelo3dApple::de_modelo(pixpin_bim::civil::de_puntos(texto, "PNEZD.csv").unwrap())
    }

    #[test]
    fn los_vertices_van_en_24_bytes_con_la_disposicion_de_repr_c() {
        use std::mem::{offset_of, size_of};
        let v = V3 {
            pos: [1.0, 2.0, 3.0],
            normal: 0x00_7f_00_00,
            color: 0xff00_00ff,
            elemento: 7,
        };
        let b = vertices_en_bytes(&[v]);
        assert_eq!(b.len(), size_of::<V3>());
        assert_eq!(
            &b[offset_of!(V3, normal)..][..4],
            &0x00_7f_00_00u32.to_le_bytes()
        );
        assert_eq!(
            &b[offset_of!(V3, color)..][..4],
            &0xff00_00ffu32.to_le_bytes()
        );
        assert_eq!(&b[offset_of!(V3, elemento)..][..4], &7u32.to_le_bytes());
        assert_eq!(&b[8..12], &3.0f32.to_le_bytes());
    }

    #[test]
    fn un_fichero_de_puntos_da_terreno_curvas_y_puntos() {
        let m = puntos();
        let r = m.resumen();
        assert!(!r.vacio && r.vertices > 0);
        let i = m.indices();
        assert!(!i.opacos.is_empty() && i.opacos.len() % 12 == 0);
        assert!(!i.lineas.is_empty(), "sin curvas de nivel");
        assert_eq!(i.puntos.len(), 5 * 6 * 4);
        assert_eq!(m.vertices().len() as u64, r.vertices * 24);
        // Ocultar una clase quita sus indices y no los demas.
        let cats = m.categorias();
        assert!(cats.len() >= 2, "{cats:?}");
        let curvas = cats
            .iter()
            .find(|c| c.tipo == "Curvas de nivel")
            .expect("curvas");
        let v = m.visibles(Filtro3d {
            tipos_ocultos: vec![curvas.tipo.clone()],
            ..Default::default()
        });
        assert!(v.lineas.len() < i.lineas.len());
        assert_eq!(v.opacos, i.opacos);
        // Sin filtro, todo.
        assert_eq!(m.visibles(Filtro3d::default()), i);
    }

    #[test]
    fn el_rayo_desde_arriba_toca_el_terreno_y_respeta_el_filtro_y_el_corte() {
        let m = puntos();
        let c = m.resumen().caja;
        let centro = Punto3d {
            x: ((c.x0 + c.x1) / 2.0) as f64,
            y: ((c.y0 + c.y1) / 2.0) as f64,
            z: c.z1 as f64 + 100.0,
        };
        let abajo = Punto3d {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };
        let e = m
            .elegir(centro, abajo, Filtro3d::default(), None)
            .expect("toca el terreno");
        assert!(e.distancia > 99.0 && e.punto.z <= c.z1 as f64 + 1e-3);
        let info = m.elemento(e.elemento).unwrap();
        assert!(info.superficie > 0.0, "{info:?}");
        // Aislado otro elemento: nada.
        let otro = (0..m.resumen().elementos)
            .find(|&k| k != e.elemento)
            .unwrap();
        assert!(
            m.elegir(
                centro,
                abajo,
                Filtro3d {
                    aislado: Some(otro),
                    ..Default::default()
                },
                None
            )
            .is_none()
        );
        // Con una caja de seccion por encima del terreno: nada.
        let alto = Caja3d {
            x0: c.x0,
            y0: c.y0,
            z0: c.z1 + 50.0,
            x1: c.x1,
            y1: c.y1,
            z1: c.z1 + 60.0,
        };
        assert!(
            m.elegir(centro, abajo, Filtro3d::default(), Some(alto))
                .is_none()
        );
        // Mirando hacia arriba: nada.
        assert!(
            m.elegir(
                centro,
                Punto3d {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0
                },
                Filtro3d::default(),
                None
            )
            .is_none()
        );
        assert!(m.elemento(9999).is_none());
    }

    #[test]
    fn los_niveles_cuentan_sus_elementos() {
        let mut c = Constructor3d::nuevo();
        let a = c.elemento("IfcWall", "Muro 1");
        let b = c.elemento("IfcSlab", "Losa");
        c.niveles(
            vec![pixpin_cad::modelo3d::Nivel {
                nombre: "1ER PISO".into(),
                cota: 0.0,
                clave: String::new(),
            }],
            &[(a, 0)],
        );
        c.malla(
            a,
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            None,
            &[0, 1, 2],
            [0.8, 0.8, 0.8, 1.0],
        );
        c.malla(
            b,
            &[[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
            None,
            &[0, 1, 2],
            [0.8, 0.8, 0.8, 1.0],
        );
        let m = Modelo3dApple::de_modelo(c.terminar());
        let n = m.niveles();
        assert_eq!((n.len(), n[0].elementos), (1, 1));
        assert_eq!(m.elemento(a).unwrap().nivel.as_deref(), Some("1ER PISO"));
        assert_eq!(m.elemento(b).unwrap().nivel, None);
        assert_eq!(m.elemento(a).unwrap().categoria, "Muro");
        // Ocultar el nivel quita el muro y deja la losa (sin nivel).
        let v = m.visibles(Filtro3d {
            niveles_ocultos: vec![0],
            ..Default::default()
        });
        assert_eq!(v.opacos.len(), 3 * 4);
    }

    #[test]
    fn la_camara_encuadra_proyecta_y_elige_el_centro() {
        let caja = Caja3d {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            x1: 40.0,
            y1: 10.0,
            z1: 12.0,
        };
        let c = camara_encuadrar(caja, 800, 600);
        let m = camara_matriz(c, 800, 600, 25.0);
        assert_eq!(m.len(), 16);
        // El objetivo cae en el centro: p * M por filas.
        let p = [
            c.objetivo.x as f32,
            c.objetivo.y as f32,
            c.objetivo.z as f32,
            1.0,
        ];
        let clip: Vec<f32> = (0..4)
            .map(|j| (0..4).map(|i| p[i] * m[i * 4 + j]).sum())
            .collect();
        assert!((clip[0] / clip[3]).abs() < 1e-4 && (clip[1] / clip[3]).abs() < 1e-4);
        assert!(clip[2] / clip[3] > 0.0 && clip[2] / clip[3] < 1.0);
        // El rayo por el centro apunta al objetivo.
        let ojo = camara_ojo(c);
        let r = camara_rayo(c, 400.0, 300.0, 800, 600);
        let d = [
            c.objetivo.x - ojo.x,
            c.objetivo.y - ojo.y,
            c.objetivo.z - ojo.z,
        ];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((r.x - d[0] / l).abs() < 1e-9 && (r.z - d[2] / l).abs() < 1e-9);
        // Acercar, mover, girar y la animacion.
        let z = camara_zoom(c, 0.5, c.objetivo);
        assert!((z.dist - c.dist * 0.5).abs() < 1e-9);
        let g = camara_girar(c, 0.3, 0.1, c.objetivo);
        assert!((g.rumbo - c.rumbo - 0.3).abs() < 1e-9);
        assert!(camara_mover(c, 10.0, 0.0, 600).objetivo != c.objetivo);
        assert!(camara_cerca_de(camara_hacia(c, z, 1.0), z));
        assert!(!camara_cerca_de(c, z));
    }

    #[test]
    fn las_vistas_guardadas_del_croquis_dan_su_camara() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-vistas-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let json = br##"{"colorDelFondo":"#202830","trazos":[{"id":"t","puntos":[{"x":1,"y":2,"z":3},{"x":5,"y":2,"z":3}],"color":"#ffffff","grosor":4}],
                 "vistas":[{"id":"v","nombre":"Frente","camara":{"giro":0.0,"inclinacion":0.0}},{"id":"w","nombre":"Arriba","camara":{"giro":1.0,"inclinacion":1.2}}]}"##;
        let m = modelo_de_croquis(&d, "chat", json).unwrap();
        assert_eq!(
            m.vistas_guardadas(),
            vec!["Frente".to_string(), "Arriba".to_string()]
        );
        let a = m.camara_de_vista(0, 800, 600, 2.0).unwrap();
        let b = m.camara_de_vista(1, 800, 600, 2.0).unwrap();
        assert!(a.dist > 0.0 && b.dist > 0.0);
        // Cada vista mira desde su sitio (el giro del movil es el rumbo).
        assert!((a.rumbo - b.rumbo).abs() > 0.5 && b.altura > a.altura);
        // Casos negativos: una vista que no hay, y un modelo sin vistas.
        assert!(m.camara_de_vista(2, 800, 600, 2.0).is_none());
        assert!(
            puntos().vistas_guardadas().is_empty()
                && puntos().camara_de_vista(0, 800, 600, 2.0).is_none()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_modelo_leido_queda_en_la_cache_y_uno_roto_se_rehace() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-cache3d-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let csv = d.join("puntos.csv");
        std::fs::write(
            &csv,
            "1,100,200,10,A\n2,100,210,11,B\n3,110,200,12,C\n4,110,210,10,D\n5,105,205,15,E\n",
        )
        .unwrap();
        let n = Nucleo::abrir(d.join("datos").display().to_string(), "Mac".into()).unwrap();
        let a = n.abrir_modelo(csv.display().to_string()).unwrap();
        let cache = d.join("datos").join("cache").join("bim");
        let guardado = std::fs::read_dir(&cache)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .next()
            .unwrap();
        assert_eq!(guardado.extension().unwrap(), "px3d");
        // La segunda vez, de la cache: el mismo modelo.
        let b = n.abrir_modelo(csv.display().to_string()).unwrap();
        assert_eq!(a.resumen(), b.resumen());
        // Roto: se vuelve a leer.
        std::fs::write(&guardado, b"roto").unwrap();
        assert_eq!(
            n.abrir_modelo(csv.display().to_string()).unwrap().resumen(),
            a.resumen()
        );
        // Caso negativo: uno que no existe.
        assert!(matches!(
            n.abrir_modelo(d.join("no.ifc").display().to_string()),
            Err(ErrorNucleo::NoExiste { .. })
        ));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_clase_sale_de_la_extension_o_del_contenido() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-clase-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let csv = d.join("puntos.csv");
        std::fs::write(
            &csv,
            "1,100,200,10,A\n2,100,210,11,B\n3,110,200,12,C\n4,110,210,10,D\n5,105,205,15,E\n",
        )
        .unwrap();
        let xml = d.join("otra.xml");
        std::fs::write(&xml, "<?xml version=\"1.0\"?><nada/>").unwrap();
        assert_eq!(
            clase_de_modelo(d.join("a.DWG").display().to_string()),
            ClaseDeModelo::Plano
        );
        assert_eq!(
            clase_de_modelo(d.join("a.ifc").display().to_string()),
            ClaseDeModelo::Modelo3d
        );
        assert_eq!(
            clase_de_modelo(csv.display().to_string()),
            ClaseDeModelo::Modelo3d
        );
        // Casos negativos: un XML cualquiera y un PDF no.
        assert_eq!(
            clase_de_modelo(xml.display().to_string()),
            ClaseDeModelo::Ninguna
        );
        assert_eq!(
            clase_de_modelo(d.join("a.pdf").display().to_string()),
            ClaseDeModelo::Ninguna
        );
        // Abrir los puntos de verdad, y uno que no existe.
        assert!(Modelo3dApple::abrir(csv.display().to_string()).is_ok());
        assert!(matches!(
            Modelo3dApple::abrir(d.join("no.ifc").display().to_string()),
            Err(ErrorNucleo::NoExiste { .. })
        ));
        let _ = std::fs::remove_dir_all(&d);
    }
}
