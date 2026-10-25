//! **Los planos DWG y DXF para Metal** (el visor de planos de Windows,
//! `pixpin-cad`, sin su D3D11): el plano se lee una vez en Rust y cruza a
//! Swift como **buferes planos de bytes**, uno por clase de cosa y por grupo,
//! que Swift sube a la tarjeta tal cual (`MTLDevice.makeBuffer(bytes:)`) y
//! dibuja con los sombreadores de Windows traducidos a MSL.
//!
//! # Por capas
//!
//! El visor de Windows dibuja el plano entero de una pieza (a la tarjeta le
//! da igual de que capa es cada raya). Aqui se pidio poder encender y apagar
//! capas, asi que el plano se lee **por capas y grosores**
//! (`Convertidor::convertir_por_capas`, el de exportar a HTML): cada grupo es
//! un [`Modelo`] con su origen. Lo de las capas apagadas tambien se lee (se
//! puede encender), pero empieza apagado, como en AutoCAD.
//!
//! # Los buferes ([`BuferesGrupo`])
//!
//! Todo son palabras de 4 bytes en **little-endian** (el orden de los
//! procesadores de Apple), una estructura detras de otra, con la misma
//! disposicion que los `#[repr(C)]` de `pixpin_cad::modelo` (comprobada al
//! compilar alli y en [`pruebas`]). En Swift basta
//! `datos.withUnsafeBytes { device.makeBuffer(bytes: $0.baseAddress!, ...) }`:
//!
//! | bufer | paso | que |
//! |---|---|---|
//! | `vertices` | 12 | `Vertice`: `x f32, y f32, color u32` (RGBA, rojo en el byte bajo; alfa 0 = el color 7) |
//! | `lineas` | 4 | indices `u32` de tiras de lineas; `0xFFFFFFFF` corta la tira (Metal la reinicia igual) |
//! | `triangulos` | 4 | indices `u32` de triangulos sobre `vertices` |
//! | `vertices_trama` | 16 | `VerticeTrama`: `x, y f32, color u32, trama u32` |
//! | `triangulos_trama` | 4 | indices sobre `vertices_trama` |
//! | `tramas` | 36 | `Trama`: `centro 2 f32, inversa 4 f32, escala f32, desde u32, cuantas u32` |
//! | `familias` | 64 | `Familia`: `base 2 f32, dir 2 f32, paso, corrimiento, largo f32, n u32, trazos 8 f32` |
//! | `malla_letras` | 8 | `[f32; 2]`: los triangulos (o pares de rayas) de cada letra, en em |
//! | `glifos` | 8 | `[u32; 2]`: desde y cuantos vertices de `malla_letras` (bit alto: de rayas) |
//! | `letras` | 32 | `Letra`: `pos 2 f32, m 4 f32, color u32, glifo u32` (una instancia por letra) |
//! | `arcos` | 32 | `Arco`: `centro 2 f32, radio, inicio, barrido f32, color u32, 2 u32 de relleno` |
//! | `tramos_*` | 32 | `Tramo`: `desde u32, cuantos u32, caja 4 f32, tamano f32, clase u32` |
//!
//! Las coordenadas van **respecto al origen del grupo** ([`GrupoPlano`],
//! `f64`): un plano en UTM se veria a saltos en `f32` a secas. Lo demas de
//! la API (cajas, enganches, cotas) va en **coordenadas del dibujo** (las de
//! AutoCAD), en `f64`.
//!
//! # Memoria (§4bis del plan)
//!
//! Los buferes se piden una vez por grupo ([`Plano::buferes`]) y, ya en la
//! tarjeta, [`Plano::soltar_dibujo`] suelta en Rust lo que solo sirve para
//! dibujar (rellenos, sombreados y letras): se quedan las rayas, los vertices
//! y los arcos, que es lo que usa la regla para engancharse. Los indices de
//! la regla ([`Enganches`], [`Rayas`]) se hacen la primera vez que se mide.
//!
//! # Lo anotado
//!
//! Las cotas y los marcos para imprimir van **en la capa del plano**, como
//! en Windows y Android (`plano_cad::anotar` de Windows, `CapaDelPlano.kt`):
//! la `anot-<uid>` del mensaje si el plano es un adjunto de un chat, o la de
//! la cache si se abrio desde el Finder (solo en este Mac), con su `.hoja`.
//! Se leen y se escriben en las unidades del fichero (las de su `.hoja`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pixpin_cad::anotado::{Encaje, GRUPO_COTA, GRUPO_MARCO};
use pixpin_cad::convertir::Grupo;
use pixpin_cad::enganche::Enganches;
use pixpin_cad::modelo::{Arco, Familia, Letra, Modelo, Trama, Tramo, Vertice, VerticeTrama};
use pixpin_cad::regla::{Rayas, Tipo};
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::{ColorRgba, Elemento, Escena, Figura, Punto2};
use pixpin_sincro::anotado::MarcoDeLaHoja;

use crate::{ErrorNucleo, Nucleo};

/// Anotar con el motor del lienzo.
mod anotar;
/// El plano ya leido, en la cache (`.pxcad`).
mod cache;
/// Exportar: la pagina web y el DXF.
mod exportar;
/// Buscar en los textos del plano.
pub(crate) mod textos;

pub use textos::TextoHallado;

// ------------------------------------------------------------- los tipos

/// Una caja en coordenadas del dibujo.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CajaPlano {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// Un punto del dibujo.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct PuntoPlano {
    pub x: f64,
    pub y: f64,
}

/// Un grupo del plano: lo de una capa con un grosor.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GrupoPlano {
    pub indice: u32,
    pub capa: String,
    /// Centesimas de milimetro (`-` o el de por defecto: el normal).
    pub grosor: i32,
    /// Si se ve ahora (su capa encendida).
    pub encendido: bool,
    /// El origen de sus coordenadas `f32`, en el dibujo.
    pub origen_x: f64,
    pub origen_y: f64,
    /// Su caja, en el dibujo.
    pub caja: CajaPlano,
    /// Lo que ocupara en la tarjeta (aproximado).
    pub bytes: u64,
}

/// Una capa del plano.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapaPlano {
    pub nombre: String,
    /// Si se ve ahora.
    pub encendida: bool,
    /// Si estaba encendida en el fichero.
    pub en_el_plano: bool,
    /// Cuantos grupos (grosores) tiene.
    pub grupos: u32,
}

/// Lo que no se pudo dibujar, por tipo (para decirlo, no para fallar).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NoDibujado {
    pub tipo: String,
    pub cuantos: u32,
}

/// Lo que se sabe del plano al abrirlo.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ResumenPlano {
    /// La caja de lo que se ve al abrir (las capas encendidas).
    pub caja: CajaPlano,
    /// La caja para encuadrar: sin los pocos puntos sueltos muy lejos.
    pub caja_util: CajaPlano,
    /// `$INSUNITS`: 4 mm, 5 cm, 6 m… (0 sin decir).
    pub unidades: u32,
    pub grupos: u32,
    pub bytes: u64,
    pub sin_dibujar: Vec<NoDibujado>,
}

/// Los buferes de un grupo, tal como van a la tarjeta (ver la doc del
/// modulo).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BuferesGrupo {
    pub vertices: Vec<u8>,
    pub lineas: Vec<u8>,
    pub triangulos: Vec<u8>,
    pub vertices_trama: Vec<u8>,
    pub triangulos_trama: Vec<u8>,
    pub tramas: Vec<u8>,
    pub familias: Vec<u8>,
    pub malla_letras: Vec<u8>,
    pub glifos: Vec<u8>,
    pub letras: Vec<u8>,
    pub arcos: Vec<u8>,
    pub tramos_lineas: Vec<u8>,
    pub tramos_triangulos: Vec<u8>,
    pub tramos_trama: Vec<u8>,
    pub tramos_letras: Vec<u8>,
    pub tramos_arcos: Vec<u8>,
}

/// De que clase es un enganche (cambia la marca que se ve).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TipoEnganche {
    /// Un vertice, un extremo o un centro.
    Punto,
    /// El pie de la perpendicular desde el punto anterior.
    Perpendicular,
    /// El punto mas cercano de una raya o un arco.
    EnLaRaya,
    /// Nada cerca.
    Libre,
}

/// Donde cae la mira de la regla.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Enganche {
    pub x: f64,
    pub y: f64,
    pub tipo: TipoEnganche,
}

/// Una cota guardada en la capa: sus puntos, en el dibujo.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CotaPlano {
    pub puntos: Vec<PuntoPlano>,
}

/// Las cotas y los marcos para imprimir de la capa del plano.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AnotadoPlano {
    pub cotas: Vec<CotaPlano>,
    pub marcos: Vec<CajaPlano>,
}

/// Como cae la capa de tinta del plano en el dibujo:
/// `x = cero_x + x_capa * u`, `y = cero_y - y_capa * u` (la y de la capa baja).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct EncajePlano {
    pub u: f64,
    pub cero_x: f64,
    pub cero_y: f64,
}

// ------------------------------------------------------------- bytes

/// Una lista de estructuras de palabras de 4 bytes, en little-endian.
fn bytes_de<T, const N: usize>(v: &[T], palabras: impl Fn(&T) -> [u32; N]) -> Vec<u8> {
    let mut b = Vec::with_capacity(v.len() * N * 4);
    for x in v {
        for w in palabras(x) {
            b.extend_from_slice(&w.to_le_bytes());
        }
    }
    b
}

fn f(x: f32) -> u32 {
    x.to_bits()
}

pub(crate) fn vertice(v: &Vertice) -> [u32; 3] {
    [f(v.x), f(v.y), v.color]
}
pub(crate) fn vertice_trama(v: &VerticeTrama) -> [u32; 4] {
    [f(v.x), f(v.y), v.color, v.trama]
}
pub(crate) fn trama(t: &Trama) -> [u32; 9] {
    let i = t.inversa;
    [
        f(t.centro[0]),
        f(t.centro[1]),
        f(i[0]),
        f(i[1]),
        f(i[2]),
        f(i[3]),
        f(t.escala),
        t.desde,
        t.cuantas,
    ]
}
pub(crate) fn familia(x: &Familia) -> [u32; 16] {
    let t = x.trazos;
    [
        f(x.base[0]),
        f(x.base[1]),
        f(x.dir[0]),
        f(x.dir[1]),
        f(x.paso),
        f(x.corrimiento),
        f(x.largo),
        x.n,
        f(t[0]),
        f(t[1]),
        f(t[2]),
        f(t[3]),
        f(t[4]),
        f(t[5]),
        f(t[6]),
        f(t[7]),
    ]
}
pub(crate) fn letra(l: &Letra) -> [u32; 8] {
    [
        f(l.pos[0]),
        f(l.pos[1]),
        f(l.m[0]),
        f(l.m[1]),
        f(l.m[2]),
        f(l.m[3]),
        l.color,
        l.glifo,
    ]
}
pub(crate) fn arco(a: &Arco) -> [u32; 8] {
    [
        f(a.centro[0]),
        f(a.centro[1]),
        f(a.radio),
        f(a.inicio),
        f(a.barrido),
        a.color,
        a.relleno[0],
        a.relleno[1],
    ]
}
pub(crate) fn tramo(t: &Tramo) -> [u32; 8] {
    [
        t.desde,
        t.cuantos,
        f(t.caja[0]),
        f(t.caja[1]),
        f(t.caja[2]),
        f(t.caja[3]),
        f(t.tamano),
        t.clase,
    ]
}

fn indices(v: &[u32]) -> Vec<u8> {
    bytes_de(v, |i| [*i])
}

/// Los buferes de un modelo (ver la doc del modulo).
pub fn buferes_de(m: &Modelo) -> BuferesGrupo {
    BuferesGrupo {
        vertices: bytes_de(&m.vertices, vertice),
        lineas: indices(&m.lineas),
        triangulos: indices(&m.triangulos),
        vertices_trama: bytes_de(&m.vertices_trama, vertice_trama),
        triangulos_trama: indices(&m.triangulos_trama),
        tramas: bytes_de(&m.tramas, trama),
        familias: bytes_de(&m.familias, familia),
        malla_letras: bytes_de(&m.malla_letras, |p| [f(p[0]), f(p[1])]),
        glifos: bytes_de(&m.glifos, |g| [g[0], g[1]]),
        letras: bytes_de(&m.letras, letra),
        arcos: bytes_de(&m.arcos, arco),
        tramos_lineas: bytes_de(&m.tramos_lineas, tramo),
        tramos_triangulos: bytes_de(&m.tramos_triangulos, tramo),
        tramos_trama: bytes_de(&m.tramos_trama, tramo),
        tramos_letras: bytes_de(&m.tramos_letras, tramo),
        tramos_arcos: bytes_de(&m.tramos_arcos, tramo),
    }
}

// ------------------------------------------------------------- el plano

/// Lo de un grupo que no cambia.
struct Ficha {
    capa: String,
    grosor: i16,
    en_el_plano: bool,
    origen: [f64; 2],
    caja: [f64; 4],
    caja_util: [f64; 4],
    bytes: u64,
}

/// Los indices de la regla de un grupo, en sus coordenadas.
struct Regla {
    puntos: Enganches,
    rayas: Rayas,
}

/// Donde vive lo anotado sobre el plano: la capa y su `.hoja`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RutasCapa {
    tinta: PathBuf,
    hoja: PathBuf,
    /// La base del adjunto si el plano es de un chat (su editor escribe la
    /// `.hoja` tras la capa); `None` en la cache.
    base: Option<pixpin_proyecto::anotado::Base>,
}

/// **Un plano DWG o DXF abierto**, listo para Metal. Se lee una vez
/// ([`Nucleo::abrir_plano`]); despues todo son consultas baratas.
#[derive(uniffi::Object)]
pub struct Plano {
    fichas: Vec<Ficha>,
    modelos: Mutex<Vec<Modelo>>,
    encendidos: Mutex<Vec<bool>>,
    reglas: Mutex<Vec<Option<Arc<Regla>>>>,
    /// El origen del plano como el del visor de Windows (el centro de lo que
    /// importa): las cotas de la capa se cuentan respecto a el.
    referencia: [f64; 2],
    /// La caja del plano respecto a `referencia` (la de su `.hoja`).
    caja_ref: [f32; 4],
    /// La unidad de la capa nueva (la letra del plano) y la de antes.
    unidad_letra: f64,
    unidad_antigua: f64,
    unidades: u32,
    sin_dibujar: Vec<(String, u32)>,
    /// Las letras que el plano pide y no estan (`fuentes-shx`).
    faltan: Vec<String>,
    rutas: RutasCapa,
    /// El fichero del plano y la carpeta de datos (la cache, las letras).
    ruta: PathBuf,
    raiz: PathBuf,
}

fn unir(a: Option<[f64; 4]>, b: [f64; 4]) -> Option<[f64; 4]> {
    if !b.iter().all(|v| v.is_finite()) {
        return a;
    }
    Some(match a {
        None => b,
        Some(a) => [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ],
    })
}

impl Plano {
    /// Lo que necesita el asistente para anotar (E1): el origen, la caja
    /// del plano respecto a el y el encaje de la capa.
    pub(crate) fn para_el_asistente(&self) -> ([f64; 2], [f32; 4], Encaje) {
        (self.referencia, self.caja_ref, self.encaje_del_fichero())
    }
}

fn caja_abs(o: [f64; 2], c: [f32; 4]) -> [f64; 4] {
    [
        o[0] + c[0] as f64,
        o[1] + c[1] as f64,
        o[0] + c[2] as f64,
        o[1] + c[3] as f64,
    ]
}

fn caja_plano(c: [f64; 4]) -> CajaPlano {
    CajaPlano {
        x0: c[0],
        y0: c[1],
        x1: c[2],
        y1: c[3],
    }
}

impl Plano {
    /// Junta los grupos leidos. `raiz` es la carpeta de datos (para la capa
    /// de un plano que no es de ningun chat).
    fn de_grupos(grupos: Vec<Grupo>, raiz: &Path, ruta: &Path) -> Plano {
        let mut fichas = Vec::with_capacity(grupos.len());
        let mut modelos = Vec::with_capacity(grupos.len());
        let mut unidades = 0;
        let mut sin_dibujar: std::collections::BTreeMap<String, u32> = Default::default();
        for g in grupos {
            let m = g.modelo;
            if unidades == 0 {
                unidades = m.unidades;
            }
            for (t, n) in &m.sin_dibujar {
                *sin_dibujar.entry(t.clone()).or_default() += n;
            }
            fichas.push(Ficha {
                capa: g.capa,
                grosor: g.grosor,
                en_el_plano: g.encendida,
                origen: m.origen,
                caja: caja_abs(m.origen, m.caja),
                caja_util: caja_abs(m.origen, m.caja_util()),
                bytes: m.bytes_gpu() as u64,
            });
            modelos.push(m);
        }
        // Lo que ve el visor de Windows: solo las capas encendidas (si todas
        // estan apagadas, todo, para no dejar un plano vacio).
        let algun = fichas.iter().any(|f| f.en_el_plano);
        let cuentan: Vec<usize> = (0..fichas.len())
            .filter(|&i| fichas[i].en_el_plano || !algun)
            .collect();
        let caja = cuentan
            .iter()
            .fold(None, |a, &i| unir(a, fichas[i].caja))
            .unwrap_or([0.0; 4]);
        let util = cuentan
            .iter()
            .fold(None, |a, &i| unir(a, fichas[i].caja_util))
            .unwrap_or(caja);
        let referencia = [(util[0] + util[2]) / 2.0, (util[1] + util[3]) / 2.0];
        let caja_ref = [
            (caja[0] - referencia[0]) as f32,
            (caja[1] - referencia[1]) as f32,
            (caja[2] - referencia[0]) as f32,
            (caja[3] - referencia[1]) as f32,
        ];
        // La unidad de la capa sale de las letras de todo lo que se ve.
        let medida = Modelo {
            caja: caja_ref,
            tramos_letras: cuentan
                .iter()
                .flat_map(|&i| modelos[i].tramos_letras.iter().copied())
                .collect(),
            ..Default::default()
        };
        let encendidos = fichas.iter().map(|f| f.en_el_plano || !algun).collect();
        Plano {
            reglas: Mutex::new((0..fichas.len()).map(|_| None).collect()),
            fichas,
            modelos: Mutex::new(modelos),
            encendidos: Mutex::new(encendidos),
            referencia,
            caja_ref,
            unidad_letra: pixpin_cad::anotado::unidad_de_la_capa(&medida),
            unidad_antigua: pixpin_cad::anotado::unidad_antigua(&medida),
            unidades,
            faltan: pixpin_cad::letras::las_que_faltan(
                &sin_dibujar
                    .iter()
                    .map(|(t, n)| (t.clone(), *n))
                    .collect::<Vec<_>>(),
            ),
            // Las letras que faltan no son cosas sin dibujar: van aparte.
            sin_dibujar: sin_dibujar
                .into_iter()
                .filter(|(t, _)| !t.starts_with(pixpin_cad::letras::FUENTE_QUE_FALTA))
                .collect(),
            rutas: rutas_de_la_capa(raiz, ruta),
            ruta: ruta.to_path_buf(),
            raiz: raiz.to_path_buf(),
        }
    }

    fn modelos_tomados(&self) -> std::sync::MutexGuard<'_, Vec<Modelo>> {
        self.modelos.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn encendidos_tomados(&self) -> std::sync::MutexGuard<'_, Vec<bool>> {
        self.encendidos.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Los indices de la regla del grupo `i`, hechos la primera vez.
    fn regla(&self, i: usize) -> Option<Arc<Regla>> {
        let mut r = self.reglas.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(hecha) = r.get(i)?.clone() {
            return Some(hecha);
        }
        let modelos = self.modelos_tomados();
        let m = modelos.get(i)?;
        let hecha = Arc::new(Regla {
            puntos: Enganches::de(m),
            rayas: Rayas::de(m),
        });
        r[i] = Some(hecha.clone());
        Some(hecha)
    }

    /// El encaje de lo que hay ahora en el disco (como `encaje_del_fichero`
    /// de Windows): el de su `.hoja`; sin ella, el de antes si ya hay capa o
    /// el de la letra si es nueva. Respecto a `referencia`.
    fn encaje_del_fichero(&self) -> Encaje {
        leer_hoja(&self.rutas.hoja)
            .and_then(|h| Encaje::de_la_hoja(h, self.caja_ref))
            .unwrap_or_else(|| {
                if self.rutas.tinta.is_file() {
                    Encaje::de_unidad(self.unidad_antigua)
                } else {
                    Encaje::de_unidad(self.unidad_letra)
                }
            })
    }

    fn a_rel(&self, p: PuntoPlano) -> [f64; 2] {
        [p.x - self.referencia[0], p.y - self.referencia[1]]
    }

    fn a_abs(&self, p: [f64; 2]) -> PuntoPlano {
        PuntoPlano {
            x: p[0] + self.referencia[0],
            y: p[1] + self.referencia[1],
        }
    }
}

#[uniffi::export]
impl Plano {
    /// Lo que se sabe del plano: cajas, unidades, cuantos grupos.
    pub fn resumen(&self) -> ResumenPlano {
        let r = self.referencia;
        let c = self.caja_ref;
        let caja = [
            r[0] + c[0] as f64,
            r[1] + c[1] as f64,
            r[0] + c[2] as f64,
            r[1] + c[3] as f64,
        ];
        let enc = self.encendidos_tomados().clone();
        let util = (0..self.fichas.len())
            .filter(|&i| enc[i])
            .fold(None, |a, i| unir(a, self.fichas[i].caja_util))
            .unwrap_or(caja);
        ResumenPlano {
            caja: caja_plano(caja),
            caja_util: caja_plano(util),
            unidades: self.unidades,
            grupos: self.fichas.len() as u32,
            bytes: self.fichas.iter().map(|f| f.bytes).sum(),
            sin_dibujar: self
                .sin_dibujar
                .iter()
                .map(|(t, n)| NoDibujado {
                    tipo: t.clone(),
                    cuantos: *n,
                })
                .collect(),
        }
    }

    /// Los grupos, en orden de dibujo (por capa y grosor).
    pub fn grupos(&self) -> Vec<GrupoPlano> {
        let enc = self.encendidos_tomados();
        self.fichas
            .iter()
            .enumerate()
            .map(|(i, f)| GrupoPlano {
                indice: i as u32,
                capa: f.capa.clone(),
                grosor: f.grosor as i32,
                encendido: enc[i],
                origen_x: f.origen[0],
                origen_y: f.origen[1],
                caja: caja_plano(f.caja),
                bytes: f.bytes,
            })
            .collect()
    }

    /// Los buferes del grupo `indice` (ver la doc del modulo). Se piden una
    /// vez: ya en la tarjeta, [`Plano::soltar_dibujo`].
    pub fn buferes(&self, indice: u32) -> Result<BuferesGrupo, ErrorNucleo> {
        let modelos = self.modelos_tomados();
        let m = modelos
            .get(indice as usize)
            .ok_or_else(|| ErrorNucleo::NoExiste {
                que: format!("grupo {indice}"),
            })?;
        Ok(buferes_de(m))
    }

    /// Suelta en Rust lo que solo sirve para dibujar (rellenos, sombreados
    /// y letras) cuando ya esta en la tarjeta. La regla sigue funcionando.
    pub fn soltar_dibujo(&self) {
        for m in self.modelos_tomados().iter_mut() {
            m.triangulos = Vec::new();
            m.tramos_triangulos = Vec::new();
            m.vertices_trama = Vec::new();
            m.triangulos_trama = Vec::new();
            m.tramos_trama = Vec::new();
            m.tramas = Vec::new();
            m.familias = Vec::new();
            m.malla_letras = Vec::new();
            m.glifos = Vec::new();
            m.letras = Vec::new();
            m.tramos_letras = Vec::new();
        }
    }

    /// Suelta los indices de la regla (el aviso de memoria del sistema); se
    /// rehacen al volver a medir.
    pub fn vaciar_caches(&self) {
        for r in self
            .reglas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter_mut()
        {
            *r = None;
        }
    }

    /// Las capas, por nombre (sin distinguir mayusculas), una vez cada una.
    pub fn capas(&self) -> Vec<CapaPlano> {
        let enc = self.encendidos_tomados();
        let mut out: Vec<CapaPlano> = Vec::new();
        for (i, f) in self.fichas.iter().enumerate() {
            match out.iter_mut().find(|c| c.nombre == f.capa) {
                Some(c) => {
                    c.grupos += 1;
                    c.encendida |= enc[i];
                    c.en_el_plano |= f.en_el_plano;
                }
                None => out.push(CapaPlano {
                    nombre: f.capa.clone(),
                    encendida: enc[i],
                    en_el_plano: f.en_el_plano,
                    grupos: 1,
                }),
            }
        }
        out
    }

    /// Enciende o apaga una capa. Dice si cambio algo.
    pub fn poner_capa(&self, nombre: String, encendida: bool) -> bool {
        let mut enc = self.encendidos_tomados();
        let mut cambio = false;
        for (i, f) in self.fichas.iter().enumerate() {
            if f.capa == nombre && enc[i] != encendida {
                enc[i] = encendida;
                cambio = true;
            }
        }
        cambio
    }

    /// Los grupos que se ven ahora.
    pub fn grupos_visibles(&self) -> Vec<u32> {
        self.encendidos_tomados()
            .iter()
            .enumerate()
            .filter(|(_, e)| **e)
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// **Donde cae la mira de la regla** en `(x, y)` (dibujo), a menos de
    /// `radio` (unidades del dibujo; Swift pasa unos pixeles por lo que mide
    /// un pixel), con el punto anterior `desde` (para la perpendicular). El
    /// orden, como AutoCAD (`regla::Rayas::ajustar`): un punto, la
    /// perpendicular, el punto mas cercano de una raya; si nada, libre. Solo
    /// mira lo que se ve.
    pub fn enganchar(&self, x: f64, y: f64, radio: f64, desde: Option<PuntoPlano>) -> Enganche {
        let rango = |t: Tipo| match t {
            Tipo::Punto => 0,
            Tipo::Perpendicular => 1,
            Tipo::EnLaRaya => 2,
            Tipo::Libre => 3,
        };
        let visibles = self.grupos_visibles();
        let mut mejor: Option<(u8, f64, [f64; 2], Tipo)> = None;
        for i in visibles {
            let i = i as usize;
            let f = &self.fichas[i];
            // Lejos de su caja: ni se mira.
            if x < f.caja[0] - radio
                || x > f.caja[2] + radio
                || y < f.caja[1] - radio
                || y > f.caja[3] + radio
            {
                continue;
            }
            let Some(r) = self.regla(i) else { continue };
            let o = f.origen;
            let p = [x - o[0], y - o[1]];
            let d = desde.map(|d| [d.x - o[0], d.y - o[1]]);
            let (q, tipo) = r.rayas.ajustar(p, radio, d, Some(&r.puntos));
            if tipo == Tipo::Libre {
                continue;
            }
            let dist = (q[0] - p[0]).hypot(q[1] - p[1]);
            let k = rango(tipo);
            if mejor.is_none_or(|(mk, md, ..)| k < mk || (k == mk && dist < md)) {
                mejor = Some((k, dist, [q[0] + o[0], q[1] + o[1]], tipo));
            }
        }
        match mejor {
            Some((_, _, q, t)) => Enganche {
                x: q[0],
                y: q[1],
                tipo: match t {
                    Tipo::Punto => TipoEnganche::Punto,
                    Tipo::Perpendicular => TipoEnganche::Perpendicular,
                    Tipo::EnLaRaya => TipoEnganche::EnLaRaya,
                    Tipo::Libre => TipoEnganche::Libre,
                },
            },
            None => Enganche {
                x,
                y,
                tipo: TipoEnganche::Libre,
            },
        }
    }

    /// Una medida del plano con sus decimales y su unidad (`12.346 m`).
    pub fn texto_de_medida(&self, distancia: f64) -> String {
        pixpin_cad::enganche::medida(distancia, self.unidades)
    }

    /// **Las letras que el plano pide y no estan** (sus textos salen con
    /// otras parecidas y pueden no caber en sus recuadros): para decirlo en
    /// el visor y que el usuario las añada.
    pub fn letras_que_faltan(&self) -> Vec<String> {
        self.faltan.clone()
    }

    /// **Las cotas y los marcos guardados en la capa** (los de Windows y
    /// Android), en el dibujo. Sin capa, nada.
    pub fn anotado(&self) -> AnotadoPlano {
        let escena = leer_escena(&self.rutas.tinta)
            .map(|(_, e)| e)
            .unwrap_or_else(Escena::nueva);
        let (cotas, marcos) = leer_cotas_y_marcos(&escena.elementos, &self.encaje_del_fichero());
        AnotadoPlano {
            cotas: cotas
                .into_iter()
                .map(|c| CotaPlano {
                    puntos: c.into_iter().map(|p| self.a_abs(p)).collect(),
                })
                .collect(),
            marcos: marcos
                .into_iter()
                .map(|m| {
                    let a = self.a_abs([m[0], m[1]]);
                    let b = self.a_abs([m[2], m[3]]);
                    CajaPlano {
                        x0: a.x,
                        y0: a.y,
                        x1: b.x,
                        y1: b.y,
                    }
                })
                .collect(),
        }
    }

    /// **Guarda en la capa estas cotas y estos marcos** (los de ahora en el
    /// visor), como `CapaDelPlano.con`: lo que ya estaba igual se queda, lo
    /// nuevo entra y lo que ya no esta se marca borrado (y viaja como
    /// borrado). Primero la `.hoja` y luego la capa; un plano sin nada hecho
    /// no deja ficheros. Dice si cambio algo.
    pub fn guardar_anotado(&self, anotado: AnotadoPlano) -> Result<bool, ErrorNucleo> {
        let (lienzo, mut escena) = match leer_escena(&self.rutas.tinta) {
            Some(x) => x,
            None if self.rutas.tinta.exists() => {
                return Err(ErrorNucleo::Lienzo {
                    detalle: format!("capa ilegible: {}", self.rutas.tinta.display()),
                });
            }
            None => (excalidraw::Lienzo::vacio(), Escena::nueva()),
        };
        let encaje = self.encaje_del_fichero();
        let cotas: Vec<Vec<[f64; 2]>> = anotado
            .cotas
            .iter()
            .map(|c| c.puntos.iter().map(|p| self.a_rel(*p)).collect())
            .collect();
        let marcos: Vec<[f64; 4]> = anotado
            .marcos
            .iter()
            .map(|m| {
                let a = self.a_rel(PuntoPlano {
                    x: m.x0.min(m.x1),
                    y: m.y0.min(m.y1),
                });
                let b = self.a_rel(PuntoPlano {
                    x: m.x0.max(m.x1),
                    y: m.y0.max(m.y1),
                });
                [a[0], a[1], b[0], b[1]]
            })
            .collect();
        if !poner_en_la_escena(&mut escena, &encaje, &cotas, &marcos) {
            return Ok(false);
        }
        escribir_hoja(&self.rutas.hoja, encaje.hoja(self.caja_ref))?;
        let nuevo = excalidraw::con_escena(&lienzo, &escena);
        pixpin_sincro::disco::escribir_atomico(
            &self.rutas.tinta,
            excalidraw::escribir(&nuevo).as_bytes(),
        )?;
        Ok(true)
    }

    /// La capa de tinta del plano (`.excalidraw`, la del motor del lienzo)
    /// para pintarla encima con `Lienzo::de_fichero`. Puede no existir.
    pub fn ruta_de_la_capa(&self) -> String {
        self.rutas.tinta.display().to_string()
    }

    /// Como cae esa capa en el dibujo (la cuenta de su `.hoja`).
    pub fn encaje(&self) -> EncajePlano {
        let e = self.encaje_del_fichero();
        EncajePlano {
            u: e.u,
            cero_x: e.cero_x + self.referencia[0],
            cero_y: e.cero_y + self.referencia[1],
        }
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Abre un plano DWG o DXF** (por capas). Bloquea (un plano grande
    /// tarda segundos): Swift lo llama desde una `Task` de prioridad
    /// `.userInitiated`.
    pub fn abrir_plano(&self, ruta: String) -> Result<Arc<Plano>, ErrorNucleo> {
        let ruta = PathBuf::from(ruta);
        if !ruta.is_file() {
            return Err(ErrorNucleo::NoExiste {
                que: ruta.display().to_string(),
            });
        }
        let leido = cache::leer_por_capas(self.ruta(), &ruta)
            .map_err(|e| ErrorNucleo::Argumento { detalle: e })?;
        Ok(Arc::new(Plano::de_grupos(leido.grupos, self.ruta(), &ruta)))
    }

    /// Cuantas letras puso el usuario para los planos (`fuentes-shx`, el
    /// boton «Aa» de Windows y Android).
    pub fn letras_de_planos(&self) -> u32 {
        pixpin_cad::letras::letras_en(&pixpin_cad::letras::carpeta_en(self.ruta())).len() as u32
    }

    /// **Añade letras para los planos** (SHX, TTF, OTF, TTC; lo demas no se
    /// copia). Mandan sobre las del sistema y las de PixPin; el plano se
    /// vuelve a leer con ellas al abrirlo otra vez. Dice cuantas copio.
    pub fn anadir_letras_de_planos(&self, rutas: Vec<String>) -> u32 {
        let rutas: Vec<PathBuf> = rutas.into_iter().map(PathBuf::from).collect();
        pixpin_cad::letras::anadir(&pixpin_cad::letras::carpeta_en(self.ruta()), &rutas) as u32
    }

    /// Quita todas las letras del usuario. Dice cuantas quito.
    pub fn quitar_letras_de_planos(&self) -> u32 {
        pixpin_cad::letras::quitar_todas(&pixpin_cad::letras::carpeta_en(self.ruta())) as u32
    }
}

/// Una medida con sus decimales y su unidad (`$INSUNITS`).
#[uniffi::export]
pub fn texto_de_medida(distancia: f64, unidades: u32) -> String {
    pixpin_cad::enganche::medida(distancia, unidades)
}

// ------------------------------------------------------------- la capa
//
// Lo que sigue es `plano_cad::anotar` de Windows (por-traducir, no compila
// fuera de Windows) con la misma cuenta: se copia para que el formato sea
// el mismo, no se inventa nada.

/// La clave de un plano en la cache (`plano_cad::clave` de Windows): FNV-1a
/// de su ruta, su tamano y su fecha.
pub(crate) fn clave(ruta: &Path) -> Option<String> {
    let m = std::fs::metadata(ruta).ok()?;
    let fecha = m
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let texto = format!("{}|{}|{}", ruta.display(), m.len(), fecha);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{h:016x}"))
}

/// Donde vive lo anotado sobre `plano`: si es un adjunto de un chat, en los
/// ficheros de su mensaje (`anot-<uid>`, los que viajan); si no, en la cache.
fn rutas_de_la_capa(raiz: &Path, plano: &Path) -> RutasCapa {
    use pixpin_proyecto::anotado as an;
    if let Some(r) = an::raiz_de(plano)
        && let Some(b) = an::base_del_documento(&r, plano)
    {
        return RutasCapa {
            tinta: b.tinta(),
            hoja: b.marco(),
            base: Some(b),
        };
    }
    let clave = clave(plano).unwrap_or_else(|| "plano".into());
    let carpeta = cache::carpeta(raiz);
    RutasCapa {
        tinta: carpeta.join(format!("{clave}.anot.excalidraw")),
        hoja: carpeta.join(format!("{clave}.anot.hoja")),
        base: None,
    }
}

fn leer_hoja(ruta: &Path) -> Option<[f64; 4]> {
    let m = MarcoDeLaHoja::de_texto(&pixpin_proyecto::anotado::leer(ruta)?)?;
    Some([m.x0 as f64, m.y0 as f64, m.x1 as f64, m.y1 as f64])
}

/// Escribe la `.hoja` solo si no dice ya lo mismo.
fn escribir_hoja(ruta: &Path, h: [f64; 4]) -> std::io::Result<()> {
    let m = MarcoDeLaHoja::nuevo(h[0] as f32, h[1] as f32, h[2] as f32, h[3] as f32);
    if let Some(ya) = pixpin_proyecto::anotado::leer(ruta).and_then(|t| MarcoDeLaHoja::de_texto(&t))
        && ya.casi_igual(&m)
    {
        return Ok(());
    }
    if let Some(d) = ruta.parent() {
        std::fs::create_dir_all(d)?;
    }
    pixpin_proyecto::anotado::escribir(ruta, &m.a_texto())
}

/// La capa del disco, si esta y se entiende.
fn leer_escena(ruta: &Path) -> Option<(excalidraw::Lienzo, Escena)> {
    let texto = std::fs::read_to_string(ruta).ok()?;
    let lienzo = excalidraw::leer(&texto).ok()?;
    let escena = excalidraw::a_escena(&lienzo);
    Some((lienzo, escena))
}

fn es_del_plano(e: &Elemento) -> bool {
    e.grupos.iter().any(|g| g == GRUPO_COTA || g == GRUPO_MARCO)
}

#[allow(clippy::type_complexity)]
fn leer_cotas_y_marcos(
    elementos: &[Elemento],
    encaje: &Encaje,
) -> (Vec<Vec<[f64; 2]>>, Vec<[f64; 4]>) {
    let mut cotas = Vec::new();
    let mut marcos = Vec::new();
    for e in elementos.iter().filter(|e| !e.borrado) {
        if e.grupos.iter().any(|g| g == GRUPO_COTA) {
            if let Figura::Linea { puntos } = &e.figura
                && puntos.len() >= 2
            {
                cotas.push(
                    puntos
                        .iter()
                        .map(|p| encaje.a_plano([p.x as f64, p.y as f64]))
                        .collect(),
                );
            }
        } else if e.grupos.iter().any(|g| g == GRUPO_MARCO)
            && matches!(e.figura, Figura::Marco { .. })
        {
            let a = encaje.a_plano([e.x as f64, e.y as f64]);
            let b = encaje.a_plano([(e.x + e.ancho) as f64, (e.y + e.alto) as f64]);
            marcos.push([
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[0].max(b[0]),
                a[1].max(b[1]),
            ]);
        }
    }
    (cotas, marcos)
}

fn color(hex: u32) -> ColorRgba {
    let c = |d: u32| ((hex >> d) & 0xff) as f32 / 255.0;
    ColorRgba::opaco(c(16), c(8), c(0))
}

/// Una cota como figura de la capa: raya naranja, id de su forma.
fn elemento_de_cota(c: &[[f64; 2]], encaje: &Encaje) -> Elemento {
    let puntos: Vec<Punto2> = c
        .iter()
        .map(|p| {
            let q = encaje.a_capa(*p);
            Punto2::nuevo(q[0] as f32, q[1] as f32)
        })
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &puntos {
        (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
    }
    let numeros: Vec<f64> = c.iter().flat_map(|p| [p[0], p[1]]).collect();
    let mut e = Elemento {
        x: puntos[0].x,
        y: puntos[0].y,
        ancho: x1 - x0,
        alto: y1 - y0,
        trazo: color(0xff8a00),
        grosor: 1.0,
        grupos: vec![GRUPO_COTA.into()],
        figura: Figura::Linea { puntos },
        ..Default::default()
    };
    e.extras.id_de_fichero = Some(pixpin_cad::anotado::huella(GRUPO_COTA, &numeros));
    e
}

/// Un marco para imprimir como figura de la capa: azul, id de su forma.
fn elemento_de_marco(m: &[f64; 4], encaje: &Encaje) -> Elemento {
    let a = encaje.a_capa([m[0], m[3]]);
    let mut e = Elemento {
        x: a[0] as f32,
        y: a[1] as f32,
        ancho: ((m[2] - m[0]) / encaje.u) as f32,
        alto: ((m[3] - m[1]) / encaje.u) as f32,
        trazo: color(0x3d8bff),
        grosor: 1.0,
        grupos: vec![GRUPO_MARCO.into()],
        figura: Figura::Marco {
            nombre: String::new(),
        },
        ..Default::default()
    };
    e.extras.id_de_fichero = Some(pixpin_cad::anotado::huella(GRUPO_MARCO, m));
    e
}

fn forma_en_el_plano(e: &Elemento, encaje: &Encaje) -> Option<(bool, Vec<f64>)> {
    let (cotas, marcos) = leer_cotas_y_marcos(
        std::slice::from_ref(&Elemento {
            borrado: false,
            ..e.clone()
        }),
        encaje,
    );
    if let Some(c) = cotas.first() {
        return Some((true, c.iter().flat_map(|p| [p[0], p[1]]).collect()));
    }
    marcos.first().map(|m| (false, m.to_vec()))
}

/// La capa con estas cotas y estos marcos (`CapaDelPlano.con`): igual por
/// id o por forma (menos de una milesima de unidad de la capa).
fn poner_en_la_escena(
    escena: &mut Escena,
    encaje: &Encaje,
    cotas: &[Vec<[f64; 2]>],
    marcos: &[[f64; 4]],
) -> bool {
    let id_de = |e: &Elemento| e.extras.id_de_fichero.clone().unwrap_or_default();
    let mut queridos: Vec<(Elemento, bool, Vec<f64>)> = Vec::new();
    let de_cotas = cotas.iter().filter(|c| c.len() >= 2).map(|c| {
        (
            elemento_de_cota(c, encaje),
            true,
            c.iter().flat_map(|p| [p[0], p[1]]).collect(),
        )
    });
    let de_marcos = marcos
        .iter()
        .map(|m| (elemento_de_marco(m, encaje), false, m.to_vec()));
    for q in de_cotas.chain(de_marcos) {
        if !queridos.iter().any(|(e, ..)| id_de(e) == id_de(&q.0)) {
            queridos.push(q);
        }
    }
    let tope = encaje.u * 1e-3;
    let igual = |a: &[f64], b: &[f64]| {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tope)
    };
    let mut usados = vec![false; queridos.len()];
    let mut cambio = false;
    for e in escena.elementos.iter_mut().filter(|e| es_del_plano(e)) {
        let forma = forma_en_el_plano(e, encaje);
        let id = id_de(e);
        let cual = queridos
            .iter()
            .enumerate()
            .position(|(i, (q, cota, numeros))| {
                !usados[i]
                    && (id_de(q) == id
                        || forma
                            .as_ref()
                            .is_some_and(|(c, f)| c == cota && igual(f, numeros)))
            });
        match cual {
            Some(i) if e.borrado => {
                let (n, version, suyo) = (e.id, e.version, e.extras.id_de_fichero.clone());
                *e = queridos[i].0.clone();
                e.id = n;
                e.version = version;
                e.extras.id_de_fichero = suyo;
                e.tocar();
                usados[i] = true;
                cambio = true;
            }
            Some(i) => usados[i] = true,
            None if e.borrado => {}
            None => {
                e.borrado = true;
                e.tocar();
                cambio = true;
            }
        }
    }
    for (i, (q, ..)) in queridos.into_iter().enumerate() {
        if !usados[i] {
            escena.anadir(q);
            cambio = true;
        }
    }
    cambio
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;

    /// Un DXF de dos capas: «Muros» con 0,50 mm y «Ejes» apagada (el de las
    /// pruebas de exportar de Windows). Las pruebas de Swift escriben el
    /// mismo texto.
    pub(crate) const DXF: &str = "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n2\n0\nLAYER\n2\nMuros\n70\n0\n62\n1\n6\nCONTINUOUS\n370\n50\n0\nLAYER\n2\nEjes\n70\n0\n62\n-5\n6\nCONTINUOUS\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nMuros\n10\n0\n20\n0\n30\n0\n11\n100\n21\n0\n31\n0\n0\nLINE\n8\nMuros\n10\n100\n20\n0\n30\n0\n11\n100\n21\n60\n31\n0\n0\nCIRCLE\n8\nEjes\n10\n50\n20\n30\n30\n0\n40\n20\n0\nENDSEC\n0\nEOF\n";

    fn carpeta(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-nucleo-planos-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    pub(crate) fn abrir(nombre: &str) -> (PathBuf, Arc<Plano>) {
        let d = carpeta(nombre);
        let ruta = d.join("planta.dxf");
        std::fs::write(&ruta, DXF).unwrap();
        let n = Nucleo::abrir(
            d.join("datos").display().to_string(),
            "Mac de prueba".into(),
        )
        .unwrap();
        let p = n.abrir_plano(ruta.display().to_string()).unwrap();
        (d, p)
    }

    fn palabras(b: &[u8]) -> Vec<u32> {
        b.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    #[test]
    fn cada_estructura_va_en_bytes_con_la_disposicion_de_repr_c() {
        use std::mem::{offset_of, size_of};
        let v = Vertice {
            x: 1.5,
            y: -2.0,
            color: 0xff00_00ff,
        };
        let b = bytes_de(&[v], vertice);
        assert_eq!(b.len(), size_of::<Vertice>());
        assert_eq!(
            &b[offset_of!(Vertice, color)..][..4],
            &0xff00_00ffu32.to_le_bytes()
        );
        assert_eq!(&b[..4], &1.5f32.to_le_bytes());
        let l = Letra {
            pos: [1.0, 2.0],
            m: [3.0, 4.0, 5.0, 6.0],
            color: 7,
            glifo: 8,
        };
        let b = bytes_de(&[l], letra);
        assert_eq!(b.len(), size_of::<Letra>());
        assert_eq!(palabras(&b[offset_of!(Letra, glifo)..]), vec![8]);
        let a = Arco {
            centro: [1.0, 2.0],
            radio: 3.0,
            inicio: 0.5,
            barrido: 1.0,
            color: 9,
            relleno: [0; 2],
        };
        let b = bytes_de(&[a], arco);
        assert_eq!(b.len(), size_of::<Arco>());
        assert_eq!(palabras(&b[offset_of!(Arco, color)..])[0], 9);
        let t = Trama {
            centro: [0.0; 2],
            inversa: [1.0, 0.0, 0.0, 1.0],
            escala: 2.0,
            desde: 3,
            cuantas: 4,
        };
        let b = bytes_de(&[t], trama);
        assert_eq!(b.len(), size_of::<Trama>());
        assert_eq!(palabras(&b[offset_of!(Trama, desde)..]), vec![3, 4]);
        let fa = Familia {
            base: [0.0; 2],
            dir: [1.0, 0.0],
            paso: 1.0,
            corrimiento: 0.0,
            largo: 0.0,
            n: 2,
            trazos: [0.5; 8],
        };
        let b = bytes_de(&[fa], familia);
        assert_eq!(b.len(), size_of::<Familia>());
        assert_eq!(
            &b[offset_of!(Familia, trazos)..][..4],
            &0.5f32.to_le_bytes()
        );
        let vt = VerticeTrama {
            x: 0.0,
            y: 0.0,
            color: 1,
            trama: 2,
        };
        assert_eq!(
            bytes_de(&[vt], vertice_trama).len(),
            size_of::<VerticeTrama>()
        );
        // El tramo no es repr(C): su orden lo fija esta funcion (y Swift).
        let tr = Tramo {
            desde: 1,
            cuantos: 2,
            caja: [3.0, 4.0, 5.0, 6.0],
            tamano: 7.0,
            clase: 8,
        };
        let w = palabras(&bytes_de(&[tr], tramo));
        assert_eq!(w.len(), 8);
        assert_eq!(
            (w[0], w[1], f32::from_bits(w[5]), f32::from_bits(w[6]), w[7]),
            (1, 2, 6.0, 7.0, 8)
        );
    }

    #[test]
    fn un_dxf_se_abre_por_capas_con_sus_buferes() {
        let (d, p) = abrir("abrir");
        let capas = p.capas();
        let nombres: Vec<&str> = capas.iter().map(|c| c.nombre.as_str()).collect();
        assert_eq!(nombres, vec!["Ejes", "Muros"]);
        // «Ejes» esta apagada en el plano: empieza apagada.
        assert!(!capas[0].encendida && !capas[0].en_el_plano);
        assert!(capas[1].encendida);
        let r = p.resumen();
        assert!(
            (r.caja.x0 - 0.0).abs() < 1e-6 && (r.caja.x1 - 100.0).abs() < 1e-6,
            "{r:?}"
        );
        assert!((r.caja.y1 - 60.0).abs() < 1e-6);
        let grupos = p.grupos();
        assert_eq!(grupos.len(), 2);
        let muros = grupos.iter().find(|g| g.capa == "Muros").unwrap();
        assert_eq!(muros.grosor, 50);
        let b = p.buferes(muros.indice).unwrap();
        assert!(!b.vertices.is_empty() && b.vertices.len() % 12 == 0);
        assert!(
            b.lineas.len() % 4 == 0
                && b.tramos_lineas.len() % 32 == 0
                && !b.tramos_lineas.is_empty()
        );
        // Los indices no se salen de los vertices (salvo el corte).
        let nv = (b.vertices.len() / 12) as u32;
        assert!(palabras(&b.lineas).iter().all(|&i| i == u32::MAX || i < nv));
        // El circulo de «Ejes» es un arco exacto.
        let ejes = grupos.iter().find(|g| g.capa == "Ejes").unwrap();
        assert_eq!(p.buferes(ejes.indice).unwrap().arcos.len(), 32);
        // Encender y apagar.
        assert!(p.poner_capa("Ejes".into(), true));
        assert_eq!(p.grupos_visibles().len(), 2);
        assert!(!p.poner_capa("Ejes".into(), true));
        // Caso negativo: un grupo que no existe.
        assert!(p.buferes(99).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_fichero_que_no_es_un_plano_da_error() {
        let d = carpeta("roto");
        let ruta = d.join("roto.dxf");
        std::fs::write(&ruta, b"esto no es un plano").unwrap();
        let n = Nucleo::abrir(d.join("datos").display().to_string(), "Mac".into()).unwrap();
        assert!(n.abrir_plano(ruta.display().to_string()).is_err());
        assert!(matches!(
            n.abrir_plano(d.join("no.dxf").display().to_string()),
            Err(ErrorNucleo::NoExiste { .. })
        ));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_regla_se_engancha_y_solo_a_lo_que_se_ve() {
        let (d, p) = abrir("regla");
        // Cerca de la esquina (100, 0): un punto.
        let e = p.enganchar(100.3, 0.2, 1.0, None);
        assert_eq!(e.tipo, TipoEnganche::Punto);
        assert!((e.x - 100.0).abs() < 1e-4 && e.y.abs() < 1e-4);
        // En mitad de la raya de abajo: en la raya.
        let e = p.enganchar(40.0, 0.4, 1.0, None);
        assert_eq!(e.tipo, TipoEnganche::EnLaRaya);
        assert!(e.y.abs() < 1e-4);
        // Desde (40, 30), cerca de la raya vertical: la perpendicular (100, 30).
        let e = p.enganchar(99.6, 30.5, 1.0, Some(PuntoPlano { x: 40.0, y: 30.0 }));
        assert_eq!(e.tipo, TipoEnganche::Perpendicular);
        assert!((e.x - 100.0).abs() < 1e-4 && (e.y - 30.0).abs() < 1e-4);
        // El circulo de «Ejes» (apagada) no engancha; encendida, si.
        assert_eq!(p.enganchar(50.0, 10.3, 0.6, None).tipo, TipoEnganche::Libre);
        p.poner_capa("Ejes".into(), true);
        assert_ne!(p.enganchar(50.0, 10.3, 0.6, None).tipo, TipoEnganche::Libre);
        // La medida, con su unidad (este DXF no la dice).
        assert_eq!(p.texto_de_medida(12.34567), "12.346");
        assert_eq!(texto_de_medida(2.0, 6), "2.000 m");
        // Tras soltar lo de dibujar, la regla sigue.
        p.soltar_dibujo();
        p.vaciar_caches();
        assert_eq!(p.enganchar(100.3, 0.2, 1.0, None).tipo, TipoEnganche::Punto);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn las_cotas_y_los_marcos_van_a_la_capa_y_vuelven() {
        let (d, p) = abrir("anotado");
        // Sin nada hecho, ni ficheros.
        assert_eq!(
            p.anotado(),
            AnotadoPlano {
                cotas: vec![],
                marcos: vec![]
            }
        );
        assert!(
            !p.guardar_anotado(AnotadoPlano {
                cotas: vec![],
                marcos: vec![]
            })
            .unwrap()
        );
        assert!(!Path::new(&p.ruta_de_la_capa()).exists());
        let cota = CotaPlano {
            puntos: vec![
                PuntoPlano { x: 0.0, y: 0.0 },
                PuntoPlano { x: 100.0, y: 0.0 },
            ],
        };
        let marco = CajaPlano {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 60.0,
        };
        let a = AnotadoPlano {
            cotas: vec![cota.clone()],
            marcos: vec![marco],
        };
        assert!(p.guardar_anotado(a.clone()).unwrap());
        // La capa y su .hoja, en la cache (el plano no es de ningun chat).
        let tinta = PathBuf::from(p.ruta_de_la_capa());
        assert!(tinta.is_file() && tinta.to_string_lossy().ends_with(".anot.excalidraw"));
        let hoja = tinta.with_file_name(
            tinta
                .file_name()
                .unwrap()
                .to_string_lossy()
                .replace(".excalidraw", ".hoja"),
        );
        let marco_hoja = MarcoDeLaHoja::de_texto(&std::fs::read_to_string(&hoja).unwrap()).unwrap();
        assert!(marco_hoja.valido());
        // La cota, en el formato de Windows: grupo, color e id de su forma.
        let texto = std::fs::read_to_string(&tinta).unwrap();
        assert!(
            texto.contains("plano-cota") && texto.contains("plano-marco"),
            "{texto}"
        );
        assert!(
            texto.contains("#ff8a00") && texto.contains("#3d8bff"),
            "{texto}"
        );
        let leido = p.anotado();
        assert_eq!(leido.cotas.len(), 1);
        let q = &leido.cotas[0].puntos;
        assert!(
            (q[1].x - 100.0).abs() < 1e-3 && q[1].y.abs() < 1e-3,
            "{q:?}"
        );
        assert!((leido.marcos[0].y1 - 60.0).abs() < 1e-3);
        // Lo mismo otra vez: nada cambia, nada se escribe.
        assert!(!p.guardar_anotado(leido.clone()).unwrap());
        // Quitar la cota la deja borrada (viaja como borrado).
        assert!(
            p.guardar_anotado(AnotadoPlano {
                cotas: vec![],
                marcos: leido.marcos.clone()
            })
            .unwrap()
        );
        assert!(p.anotado().cotas.is_empty());
        assert!(
            std::fs::read_to_string(&tinta)
                .unwrap()
                .contains("isDeleted")
        );
        // Volver a ponerla la revive.
        assert!(p.guardar_anotado(a).unwrap());
        assert_eq!(p.anotado().cotas.len(), 1);
        // El encaje dice la unidad de la capa.
        assert!(p.encaje().u > 0.0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_capa_rota_no_se_pisa() {
        let (d, p) = abrir("rota");
        let tinta = PathBuf::from(p.ruta_de_la_capa());
        std::fs::create_dir_all(tinta.parent().unwrap()).unwrap();
        std::fs::write(&tinta, "{esto no es json").unwrap();
        // Leer: nada. Escribir: error, y el fichero sigue como estaba.
        assert!(p.anotado().cotas.is_empty());
        let a = AnotadoPlano {
            cotas: vec![CotaPlano {
                puntos: vec![PuntoPlano { x: 0.0, y: 0.0 }, PuntoPlano { x: 1.0, y: 0.0 }],
            }],
            marcos: vec![],
        };
        assert!(p.guardar_anotado(a).is_err());
        assert_eq!(std::fs::read_to_string(&tinta).unwrap(), "{esto no es json");
        let _ = std::fs::remove_dir_all(&d);
    }

    // ---- la cache (las de `plano_cad` de Windows)

    #[test]
    fn se_abren_los_dwg_y_los_dxf() {
        use crate::modelos3d::{ClaseDeModelo, clase_de_modelo};
        assert_eq!(clase_de_modelo("Planta.DWG".into()), ClaseDeModelo::Plano);
        assert_eq!(clase_de_modelo("corte.dxf".into()), ClaseDeModelo::Plano);
        // Caso negativo: lo demas no.
        assert_ne!(clase_de_modelo("plano.pdf".into()), ClaseDeModelo::Plano);
        assert_ne!(clase_de_modelo("dwg".into()), ClaseDeModelo::Plano);
    }

    #[test]
    fn con_letras_nuevas_el_plano_se_vuelve_a_leer() {
        // Sin letras del usuario, el nombre no lleva huella.
        assert_eq!(cache::nombre_en_cache("abc", ""), "abc-l1.pxcad");
        let d = carpeta("huella");
        let vacia = pixpin_cad::letras::huella(&d.join("fuentes-shx"));
        assert_eq!(cache::nombre_en_cache("abc", &vacia), "abc-l1.pxcad");
        std::fs::create_dir_all(d.join("fuentes-shx")).unwrap();
        std::fs::write(d.join("fuentes-shx").join("romans.shx"), b"x").unwrap();
        let con =
            cache::nombre_en_cache("abc", &pixpin_cad::letras::huella(&d.join("fuentes-shx")));
        assert_ne!(con, "abc-l1.pxcad");
        assert!(con.starts_with("abc-l1-") && con.ends_with(".pxcad"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_clave_cambia_si_cambia_el_fichero() {
        let d = carpeta("clave");
        let f = d.join("a.dxf");
        std::fs::write(&f, "0\nEOF\n").unwrap();
        let k1 = clave(&f).unwrap();
        std::fs::write(&f, "0\nSECTION\n0\nEOF\n").unwrap();
        assert_ne!(clave(&f).unwrap(), k1);
        assert_eq!(clave(&d.join("no-esta.dwg")), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_segunda_vez_el_plano_sale_de_la_cache_y_una_cache_rota_se_rehace() {
        let (d, p) = abrir("cache");
        let raiz = d.join("datos");
        let c = cache::carpeta(&raiz);
        let guardados = || -> Vec<PathBuf> {
            std::fs::read_dir(&c)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|r| r.extension().is_some_and(|x| x == "pxcad"))
                .collect()
        };
        let g = guardados();
        assert_eq!(g.len(), 1, "{g:?}");
        // Lo guardado es el plano por capas: el mismo plano otra vez.
        let otra = cache::leer_por_capas(&raiz, &d.join("planta.dxf")).unwrap();
        assert_eq!(otra.grupos.len() as u32, p.resumen().grupos);
        // Roto: se vuelve a leer y se escribe bien.
        std::fs::write(&g[0], b"PXCAPAS1roto").unwrap();
        let n = Nucleo::abrir(raiz.display().to_string(), "Mac de prueba".into()).unwrap();
        let q = n
            .abrir_plano(d.join("planta.dxf").display().to_string())
            .unwrap();
        assert_eq!(q.resumen().grupos, p.resumen().grupos);
        assert!(pixpin_cad::web::de_bytes(&std::fs::read(&g[0]).unwrap()).is_some());
        // Ninguna letra pedida falta en el plano de prueba.
        assert!(q.letras_que_faltan().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_cache_guarda_los_mas_recientes_y_no_toca_lo_anotado() {
        let d = carpeta("podar");
        for i in 0..(cache::EN_CACHE + 3) {
            std::fs::write(d.join(format!("{i:02}-l1.pxcad")), b"x").unwrap();
        }
        std::fs::write(d.join("k.anot.excalidraw"), b"{}").unwrap();
        std::fs::write(d.join("k-textos.json"), b"[]").unwrap();
        cache::podar(&d);
        let quedan = std::fs::read_dir(&d).unwrap().count();
        assert_eq!(quedan, cache::EN_CACHE + 2);
        assert!(d.join("k.anot.excalidraw").is_file() && d.join("k-textos.json").is_file());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn las_letras_de_los_planos_se_anaden_y_se_quitan() {
        let d = carpeta("letras");
        let n = Nucleo::abrir(
            d.join("datos").display().to_string(),
            "Mac de prueba".into(),
        )
        .unwrap();
        assert_eq!(n.letras_de_planos(), 0);
        std::fs::write(d.join("romans.shx"), b"x").unwrap();
        std::fs::write(d.join("nota.txt"), b"x").unwrap();
        // Lo que no es una letra no se copia.
        let rutas = ["romans.shx", "nota.txt"]
            .map(|f| d.join(f).display().to_string())
            .to_vec();
        assert_eq!(n.anadir_letras_de_planos(rutas), 1);
        assert_eq!(n.letras_de_planos(), 1);
        assert_eq!(n.quitar_letras_de_planos(), 1);
        assert_eq!(n.letras_de_planos(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn los_textos_del_plano_se_buscan_y_quedan_en_la_cache() {
        let d = carpeta("textos");
        // Un DXF con un texto.
        let ruta = d.join("calle.dxf");
        std::fs::write(
            &ruta,
            "0\nSECTION\n2\nENTITIES\n0\nTEXT\n8\n0\n10\n5\n20\n7\n30\n0\n40\n2.5\n1\nJR. ANTISUYO\n0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n50\n21\n0\n31\n0\n0\nENDSEC\n0\nEOF\n",
        )
        .unwrap();
        let n = Nucleo::abrir(
            d.join("datos").display().to_string(),
            "Mac de prueba".into(),
        )
        .unwrap();
        let p = n.abrir_plano(ruta.display().to_string()).unwrap();
        let h = p.buscar_textos("jr antisuyo".into(), 5).unwrap();
        assert_eq!(h.len(), 1, "{h:?}");
        assert!((h[0].x - 5.0).abs() < 1e-6 && (h[0].alto - 2.5).abs() < 1e-6);
        let k = clave(&ruta).unwrap();
        assert!(
            cache::carpeta(&d.join("datos"))
                .join(format!("{k}-textos.json"))
                .is_file()
        );
        // Caso negativo.
        assert!(p.buscar_textos("Collasuyo".into(), 5).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
