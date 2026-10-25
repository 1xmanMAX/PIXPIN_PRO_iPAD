//! **La ventana de un modelo 3D (BIM), como un pin**: la misma que la de los
//! planos (sin bordes, nace sin fijar y su pin la deja encima, la barra fuera que aparece y se
//! va), con lo que pide un modelo: girarlo, acercarse y cortarlo. Lo pidio el
//! usuario el 9-oct-2026: «un visor sencillo como un pin, que se pueda ver
//! facilmente cualquier modelo».
//!
//! - Arrastrar (izquierdo): girar alrededor del punto que se toco.
//! - Arrastrar (derecho o central), o Mayus + izquierdo: mover.
//! - Rueda: acercar y alejar hacia el cursor.
//! - Clic: el elemento tocado se resalta y se nombra (Muro · nombre).
//! - Otro clic (o doble clic) en el elegido: aislarlo, solo el, encuadrado;
//!   Esc o doble clic en el vacio vuelven a todo el modelo. Tambien la I.
//! - Doble clic, F o Inicio: el modelo entero.
//! - Barra: asa, tema, caja de seccion (la de Revit: el modelo dentro de una
//!   caja con un tirador en cada cara; arrastrandolos se corta por donde se
//!   quiera, y lo cortado se ve macizo) y el pin.
//! - Niveles y categorias (boton de la barra o L): un panel para mostrar u
//!   ocultar plantas y categorias; lo elegido dice su nivel, su volumen y su
//!   area (sacados de su malla).
//! - Teclas: Esc (suelta lo elegido; si no hay, cierra), B tema, C caja,
//!   A aristas, T encima, P planta, flechas para girar.

mod arbol;

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, ReleaseCapture, VK_CONTROL, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::convertir::rgba;
use crate::gpu::{Gpu, PlanoGpu};
use crate::gpu3d::{ModeloGpu, Vista3d};
use crate::modelo::{Constructor, Modelo};
use crate::modelo3d::Modelo3d;
use arbol::Arbol;
use crate::texto::Textos;
pub use crate::orbita::Orbita;
use crate::orbita::{ALTURA_MAX, por, punto, sub, suma, unit};
use crate::ventana::{
    ALTO_BARRA, CAPA_ENCIMA, De, ESTADO, Ev, ancho_barra, aviso, barra, botones_barra, colocar_barra, crear_barra, crear_ventana, poner_encima,
    raya, rect, seguir_barra, vista_pantalla,
};

const INFINITE: u32 = u32::MAX;
const BOTONES: [&str; 5] = ["asa", "tema", "corte", "capas", "fijar"];
/// Los textos de la ventana, en el idioma de la app.
#[derive(Debug, Clone)]
pub struct TextosUi3d {
    pub abriendo: String,
    pub vacio: String,
    pub error: String,
    pub todo: String,
    pub tema: String,
    pub corte: String,
    pub aislar: String,
    pub panel: String,
    pub niveles: String,
    pub categorias: String,
    pub sin_nivel: String,
    pub mostrar_todo: String,
    pub aristas: String,
    pub planta: String,
    pub encima: String,
    pub cerrar: String,
    pub altura_corte: String,
    pub pista: String,
}

impl Default for TextosUi3d {
    fn default() -> Self {
        Self {
            abriendo: "Abriendo el modelo…".into(),
            vacio: "El modelo no tiene nada que dibujar".into(),
            error: "No se pudo abrir el modelo".into(),
            todo: "Ver todo el modelo\tF".into(),
            tema: "Fondo claro u oscuro\tB".into(),
            corte: "Caja de sección\tC".into(),
            aislar: "Aislar lo elegido\tI".into(),
            panel: "Niveles y categorías\tL".into(),
            niveles: "NIVELES".into(),
            categorias: "CATEGORÍAS".into(),
            sin_nivel: "Sin nivel".into(),
            mostrar_todo: "Mostrar todo".into(),
            aristas: "Aristas\tA".into(),
            planta: "Vista en planta\tP".into(),
            encima: "Siempre encima\tT".into(),
            cerrar: "Cerrar\tEsc".into(),
            altura_corte: "Corte".into(),
            pista: "Arrastrar: girar · Derecho: mover · Rueda: acercar · Clic: ver que es · Otro clic: aislarlo".into(),
        }
    }
}

const M_TODO: u32 = 1;
const M_TEMA: u32 = 2;
const M_CORTE: u32 = 3;
const M_ARISTAS: u32 = 4;
const M_PLANTA: u32 = 5;
const M_ENCIMA: u32 = 6;
const M_CERRAR: u32 = 7;
const M_AISLAR: u32 = 8;
const M_PANEL: u32 = 9;
const M_EXPORTAR: u32 = 10;
/// La primera vista guardada; las demas, seguidas.
const M_VISTA: u32 = 100;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn menu(hwnd: HWND, t: &TextosUi3d, corte: bool, aislado: Option<bool>, panel: bool, aristas: bool, fijada: bool, vistas: &[String], exportar: Option<&str>) {
    // SAFETY: menu propio que se destruye antes de salir; cadenas vivas.
    unsafe {
        let Ok(m) = CreatePopupMenu() else { return };
        let poner = |id: u32, texto: &str, marca: bool| {
            let v: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = AppendMenuW(m, if marca { MF_STRING | MF_CHECKED } else { MF_STRING }, id as usize, PCWSTR(v.as_ptr()));
        };
        poner(M_TODO, &t.todo, false);
        poner(M_PLANTA, &t.planta, false);
        poner(M_TEMA, &t.tema, false);
        poner(M_CORTE, &t.corte, corte);
        // Solo si hay algo elegido o aislado.
        if let Some(a) = aislado {
            poner(M_AISLAR, &t.aislar, a);
        }
        poner(M_PANEL, &t.panel, panel);
        poner(M_ARISTAS, &t.aristas, aristas);
        // Las vistas guardadas del croquis, con su tecla (1 a 9).
        if !vistas.is_empty() {
            let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
            for (i, n) in vistas.iter().enumerate() {
                let texto = if i < 9 { format!("{n}	{}", i + 1) } else { n.clone() };
                poner(M_VISTA + i as u32, &texto, false);
            }
        }
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        if let Some(e) = exportar {
            poner(M_EXPORTAR, e, false);
        }
        poner(M_ENCIMA, &t.encima, fijada);
        poner(M_CERRAR, &t.cerrar, false);
        let mut p = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut p);
        let _ = TrackPopupMenu(m, TPM_RIGHTBUTTON, p.x, p.y, Some(0), hwnd, None);
        let _ = DestroyMenu(m);
    }
}

/// Lo que se escribe tras una cota: metros, pies o nada (un DWG no lo dice).
fn unidad_de(metros: f32) -> &'static str {
    if (metros - 1.0).abs() < 1e-6 {
        " m"
    } else if (metros - 0.3048).abs() < 0.001 {
        " ft"
    } else {
        ""
    }
}

/// Un punto del modelo en la pantalla (pixeles); `None` si queda detras del ojo.
fn en_pantalla(m: &[[f32; 4]; 4], p: [f64; 3], w: u32, h: u32) -> Option<[f64; 2]> {
    let v = [p[0], p[1], p[2], 1.0];
    let c: Vec<f64> = (0..4).map(|j| (0..4).map(|i| v[i] * m[i][j] as f64).sum()).collect();
    (c[3] > 1e-9).then(|| [(c[0] / c[3] + 1.0) / 2.0 * w as f64, (1.0 - c[1] / c[3]) / 2.0 * h as f64])
}

/// El centro de la cara `k` de la caja (`k` como en la caja: 0..3 las
/// caras del minimo x, y, z; 3..6 las del maximo).
fn centro_de_cara(c: [f32; 6], k: usize) -> [f64; 3] {
    let mut p = [(c[0] + c[3]) as f64 / 2.0, (c[1] + c[4]) as f64 / 2.0, (c[2] + c[5]) as f64 / 2.0];
    p[k % 3] = c[k] as f64;
    p
}

/// Hacia fuera de la cara `k`.
fn fuera_de_cara(k: usize) -> [f64; 3] {
    let mut d = [0.0; 3];
    d[k % 3] = if k < 3 { -1.0 } else { 1.0 };
    d
}

/// Un decimo del lado mayor de la caja: la medida para ver como se mueve
/// una cara en la pantalla.
fn decimo(c: [f32; 6]) -> f64 {
    ((c[3] - c[0]).max(c[4] - c[1]).max(c[5] - c[2]) as f64 * 0.1).max(1e-6)
}

/// Las 12 aristas de la caja.
fn aristas_de_caja(c: [f32; 6]) -> Vec<([f64; 3], [f64; 3])> {
    let esq = |i: usize| [c[if i & 1 == 0 { 0 } else { 3 }] as f64, c[if i & 2 == 0 { 1 } else { 4 }] as f64, c[if i & 4 == 0 { 2 } else { 5 }] as f64];
    let mut v = Vec::new();
    for i in 0..8usize {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                v.push((esq(i), esq(i | bit)));
            }
        }
    }
    v
}

/// La caja de seccion, ya en la pantalla: sus aristas y sus tiradores
/// (cara, centro y hacia fuera, unitario en pantalla).
struct CajaVista {
    aristas: Vec<([f64; 2], [f64; 2])>,
    tiradores: Vec<(usize, [f64; 2], [f64; 2])>,
    encima: Option<usize>,
}

fn caja_en_pantalla(c: [f32; 6], m: &[[f32; 4]; 4], w: u32, h: u32, encima: Option<usize>) -> CajaVista {
    let aristas = aristas_de_caja(c).into_iter().filter_map(|(a, b)| Some((en_pantalla(m, a, w, h)?, en_pantalla(m, b, w, h)?))).collect();
    let lado = decimo(c);
    let tiradores = (0..6)
        .filter_map(|k| {
            let p = centro_de_cara(c, k);
            let a = en_pantalla(m, p, w, h)?;
            let b = en_pantalla(m, suma(p, por(fuera_de_cara(k), lado)), w, h)?;
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy);
            Some((k, a, if l > 1e-6 { [dx / l, dy / l] } else { [0.0, 0.0] }))
        })
        .collect();
    CajaVista { aristas, tiradores, encima }
}

/// El tirador bajo (x, y), si hay uno a mano (el disco o su flecha).
fn tirador_en(cv: &CajaVista, x: i32, y: i32, e: f64) -> Option<usize> {
    cv.tiradores
        .iter()
        .map(|(k, p, d)| {
            let q = [p[0] + d[0] * 14.0 * e, p[1] + d[1] * 14.0 * e];
            let dist = |c: [f64; 2]| (c[0] - x as f64).hypot(c[1] - y as f64);
            (*k, dist(*p).min(dist(q)))
        })
        .filter(|(_, d)| *d <= 13.0 * e)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k)
}

/// La caja de un elemento (relativa al origen), para encuadrarlo al aislarlo.
fn caja_de_elemento(m: &Modelo3d, e: u32) -> Option<[f32; 6]> {
    let mut c: Option<[f32; 6]> = None;
    for v in m.vertices.iter().filter(|v| v.elemento == e) {
        let p = v.pos;
        let k = c.get_or_insert([p[0], p[1], p[2], p[0], p[1], p[2]]);
        for i in 0..3 {
            k[i] = k[i].min(p[i]);
            k[i + 3] = k[i + 3].max(p[i]);
        }
    }
    c
}

/// La caja recien puesta: la del modelo, algo holgada (que no corte nada).
fn caja_inicial(c: [f32; 6]) -> [f32; 6] {
    let m = (c[3] - c[0]).max(c[4] - c[1]).max(c[5] - c[2]) * 0.01;
    [c[0] - m, c[1] - m, c[2] - m, c[3] + m, c[4] + m, c[5] + m]
}

/// Lleva la cara `k` de la caja a `valor`, sin cruzar la de enfrente ni
/// irse lejos del modelo (`limite`).
fn mover_cara(c: [f32; 6], k: usize, valor: f64, limite: [f32; 6]) -> [f32; 6] {
    let mut c = c;
    let a = k % 3;
    let hueco = ((limite[a + 3] - limite[a]) * 0.002).max(1e-6);
    let v = valor as f32;
    c[k] = if k < 3 { v.clamp(limite[a], (c[a + 3] - hueco).max(limite[a])) } else { v.clamp((c[a] + hueco).min(limite[a + 3]), limite[a + 3]) };
    c
}

fn disco(c: &mut Constructor, centro: [f64; 2], r: f64, color: u32) {
    let n = 20;
    let mut t = Vec::with_capacity(n * 3);
    for i in 0..n {
        let a0 = i as f64 / n as f64 * std::f64::consts::TAU;
        let a1 = (i + 1) as f64 / n as f64 * std::f64::consts::TAU;
        t.push(centro);
        t.push([centro[0] + r * a0.cos(), centro[1] + r * a0.sin()]);
        t.push([centro[0] + r * a1.cos(), centro[1] + r * a1.sin()]);
    }
    c.triangulos(&t, color, Some(CAPA_ENCIMA));
}

struct Encima<'a> {
    /// Lo elegido, ya dicho: «Muro · nombre · 1ER PISO · 3.82 m³ · 21.80 m²».
    elegido: Option<String>,
    /// El panel de niveles y categorias, si esta abierto.
    arbol: Option<&'a arbol::Arbol>,
    caja: Option<CajaVista>,
    /// Mientras se arrastra un tirador: que cara y donde queda (en el mundo).
    cara: Option<(usize, f64)>,
    /// El punto alrededor del que se gira, mientras se gira.
    pivote: Option<[f64; 2]>,
    unidad: &'a str,
    pista: Option<&'a str>,
}

/// Lo que va plano encima del modelo: el nombre de lo elegido, la caja de
/// seccion con sus tiradores, el pivote del giro y la pista.
fn encima_del_modelo(textos: &mut Textos, a: &Encima, ui: &TextosUi3d, w: u32, h: u32, e: f64, claro: bool) -> Modelo {
    let mut c = Constructor::nuevo();
    let (pildora, letra) = if claro { (0xEE1C1C1Eu32, rgba(255, 255, 255)) } else { (0xEEF5F5F7u32, rgba(0x1C, 0x1C, 0x1E)) };
    let tam = 13.0 * e;
    let pastilla = |textos: &mut Textos, c: &mut Constructor, t: &str, x: f64, y: f64| {
        let ancho = textos.medir_pantalla(t, tam);
        rect(c, x, y, x + ancho + 20.0 * e, y + 26.0 * e, pildora);
        textos.en_pantalla(c, t, x + 10.0 * e, y + 17.5 * e, tam, letra, x + ancho + 40.0 * e);
    };
    if let Some(t) = &a.elegido {
        pastilla(textos, &mut c, t, 12.0 * e, h as f64 - 38.0 * e);
    } else if let Some(p) = a.pista {
        pastilla(textos, &mut c, p, 12.0 * e, h as f64 - 38.0 * e);
    }
    let azul = rgba(0x00, 0x60, 0xDF);
    let blanco = rgba(255, 255, 255);
    if let Some(cv) = &a.caja {
        let raya_caja = (azul & 0x00ff_ffff) | 0xC000_0000;
        for (p, q) in &cv.aristas {
            raya(&mut c, *p, *q, 1.5 * e, raya_caja);
        }
        for (k, p, d) in &cv.tiradores {
            let grande = cv.encima == Some(*k) || a.cara.is_some_and(|(cara, _)| cara == *k);
            let r = if grande { 7.5 } else { 6.0 } * e;
            // La flecha hacia fuera de la cara y el disco en su centro.
            if d[0] != 0.0 || d[1] != 0.0 {
                let punta = [p[0] + d[0] * (r + 11.0 * e), p[1] + d[1] * (r + 11.0 * e)];
                let base = [p[0] + d[0] * (r + 2.0 * e), p[1] + d[1] * (r + 2.0 * e)];
                let n = [-d[1] * 5.5 * e, d[0] * 5.5 * e];
                c.triangulos(&[punta, [base[0] + n[0], base[1] + n[1]], [base[0] - n[0], base[1] - n[1]]], azul, Some(CAPA_ENCIMA));
            }
            disco(&mut c, *p, r + 1.5 * e, blanco);
            disco(&mut c, *p, r, azul);
        }
    }
    if let Some(p) = a.pivote {
        disco(&mut c, p, 5.5 * e, blanco);
        disco(&mut c, p, 4.0 * e, rgba(0xFF, 0x8A, 0x00));
    }
    if let (Some((k, v)), Some(cv)) = (a.cara, &a.caja)
        && let Some((_, p, d)) = cv.tiradores.iter().find(|t| t.0 == k)
    {
        let eje = ["X", "Y", "Z"][k % 3];
        let t = format!("{} {eje} {v:.2}{}", ui.altura_corte, a.unidad);
        let ancho = textos.medir_pantalla(&t, tam);
        let (x, y) = (p[0] + d[0] * 30.0 * e, p[1] + d[1] * 30.0 * e);
        let x = if d[0] < 0.0 { x - ancho - 20.0 * e } else { x };
        let x = x.clamp(4.0 * e, (w as f64 - ancho - 24.0 * e).max(4.0 * e));
        let y = (y - 13.0 * e).clamp(4.0 * e, (h as f64 - 30.0 * e).max(4.0 * e));
        pastilla(textos, &mut c, &t, x, y);
    }
    // El panel, lo ultimo: encima de la caja y de todo.
    if let Some(ar) = a.arbol {
        ar.dibujar(&mut c, textos, ui, a.unidad, h, e, claro);
    }
    c.terminar()
}

/// **Lo que un croquis 3D le pide de mas a la ventana** (tanda 5): su fondo,
/// con que tema abre, que se encuadra al abrir, desde donde se mira y sus
/// vistas guardadas. Para un modelo BIM, todo vacio ([`ver`]).
#[derive(Debug, Clone, Default)]
pub struct Extras {
    /// El fondo propio (`colorDelFondo`), mientras no se cambie el tema.
    pub fondo: Option<[f32; 4]>,
    /// Con que tema abre (por defecto, oscuro).
    pub claro: Option<bool>,
    /// Lo que se encuadra (en el mundo, absoluto), si no es todo el modelo:
    /// el suelo y las hojas sin fin no cuentan.
    pub encuadre: Option<[f64; 6]>,
    /// Desde donde se mira al abrir y al pulsar F: rumbo y altura.
    pub orientacion: Option<(f64, f64)>,
    /// Las vistas congeladas del croquis: en el menu y con las teclas 1 a 9.
    pub vistas: Vec<crate::croquis::Vista3D>,
    /// «Exportar…» en el menu (y Ctrl+E): quien abre la ventana sabe como.
    pub exportar: Option<AlExportar>,
}

/// La foto de lo que se ve: `(ancho, alto, RGBA)`.
pub type Foto = (u32, u32, Vec<u8>);

/// El «Exportar…» del menu: su rotulo y que hacer con la vista que se ve
/// (la foto de la ventana, si se pudo leer).
#[derive(Clone)]
pub struct AlExportar {
    pub texto: String,
    pub hacer: std::sync::Arc<dyn Fn(Option<Foto>) + Send + Sync>,
}

/// **Lo que se ve en la ventana** (su zona de cliente), leido de la
/// pantalla: la ventana pinta con la tarjeta y no tiene superficie GDI que
/// imprimir, asi que la vista se exporta como se ve.
pub fn foto_de_la_ventana(hwnd: HWND) -> Option<Foto> {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
        GetDC, ReleaseDC, SRCCOPY, SelectObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    let mut r = RECT::default();
    let mut p = POINT::default();
    // SAFETY: ventana viva de este hilo; estructuras locales.
    unsafe {
        GetClientRect(hwnd, &mut r).ok()?;
        let _ = ClientToScreen(hwnd, &mut p);
    }
    let (ancho, alto) = (r.right - r.left, r.bottom - r.top);
    if ancho <= 0 || alto <= 0 {
        return None;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ancho,
            biHeight: -alto,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: objetos GDI creados y soltados aqui; `bits` vive hasta el
    // DeleteObject y mide ancho x alto x 4.
    unsafe {
        let pantalla = GetDC(None);
        let dc = CreateCompatibleDC(Some(pantalla));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(bmp) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            ReleaseDC(None, pantalla);
            return None;
        };
        let viejo = SelectObject(dc, bmp.into());
        let _ = BitBlt(dc, 0, 0, ancho, alto, Some(pantalla), p.x, p.y, SRCCOPY);
        let n = (ancho * alto * 4) as usize;
        let mut px = std::slice::from_raw_parts(bits as *const u8, n).to_vec();
        SelectObject(dc, viejo);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, pantalla);
        for c in px.chunks_exact_mut(4) {
            c.swap(0, 2);
            c[3] = 255;
        }
        Some((ancho as u32, alto as u32, px))
    }
}

impl std::fmt::Debug for AlExportar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlExportar").field("texto", &self.texto).finish()
    }
}

/// **Abre la ventana del modelo** y no vuelve hasta que se cierra.
pub fn ver(titulo: &str, cargando: Receiver<Result<Modelo3d, String>>, ui: TextosUi3d) -> Result<(), String> {
    ver_con(titulo, cargando, ui, Extras::default())
}

/// [`ver`], con lo que pide un croquis ([`Extras`]).
pub fn ver_con(titulo: &str, cargando: Receiver<Result<Modelo3d, String>>, ui: TextosUi3d, extras: Extras) -> Result<(), String> {
    let nombres_de_vistas: Vec<String> = extras.vistas.iter().map(|v| v.nombre.clone()).collect();
    // Encuadrar con la orientacion del croquis, si la trae.
    let encuadre = |caja: [f32; 6], w: u32, h: u32| {
        let mut o = Orbita::encuadrar(caja, w, h);
        if let Some((rumbo, altura)) = extras.orientacion {
            o.rumbo = rumbo;
            o.altura = altura.clamp(-ALTURA_MAX, ALTURA_MAX);
            o.dist = o.distancia_para(caja, w, h).max(1e-3);
        }
        o
    };
    // El tema se ha tocado: el fondo propio deja de mandar.
    let mut tema_tocado = false;
    let (hwnd, mut w, mut h) = crear_ventana(w!("PixPinModelo3d"), &format!("{titulo} — PixPin")).map_err(|e| e.to_string())?;
    let mut gpu = match Gpu::nueva(hwnd, w, h) {
        Ok(g) => g,
        Err(e) => {
            // SAFETY: ventana propia.
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(format!("sin Direct3D: {e}"));
        }
    };
    // SAFETY: ventana propia.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let e = dpi as f64 / 96.0;
    let (bw, bh) = (ancho_barra(e, BOTONES.len()).ceil() as i32, (ALTO_BARRA * e).ceil() as i32);
    let hbarra = crear_barra(hwnd, bw, bh).map_err(|e| e.to_string())?;
    let mut gpu_barra = Gpu::nueva(hbarra, bw as u32, bh as u32).map_err(|e| e.to_string())?;
    ESTADO.with(|s| s.borrow_mut().barra = Some((hbarra, (8.0 * e) as i32)));
    let mut textos = Textos::nuevo();
    let mut modelo: Option<(Modelo3d, ModeloGpu)> = None;
    let mut mensaje = ui.abriendo.clone();
    let mut cam = Orbita { objetivo: [0.0; 3], rumbo: -0.8, altura: 0.5, dist: 10.0 };
    let mut destino = cam;
    let mut radio = 10.0;
    // Lo que se encuadra y se corta: la caja sin los puntos sueltos lejanos.
    let mut caja_vista = [0f32; 6];
    let mut claro = extras.claro.unwrap_or(false);
    let mut fijada = crate::ventana::FIJADA_AL_NACER;
    let mut aristas = true;
    // La caja de seccion (relativa al origen del modelo), si esta puesta.
    let mut seccion: Option<[f32; 6]> = None;
    // Arrastrando un tirador: la cara, desde donde, su valor al empezar y
    // cuantos pixeles se mueve en pantalla por unidad del mundo.
    let mut tirador: Option<(usize, i32, i32, f64, [f64; 2])> = None;
    let mut tirador_encima: Option<usize> = None;
    let mut elegido: Option<u32> = None;
    // El panel de niveles y categorias: lo que tiene el modelo y si se ve.
    let mut arbol_m: Option<Arbol> = None;
    // El encuadre del principio, mientras nadie toque la vista: si la
    // ventana cambia de tamano (el pin se coloca despues de abrirse), se
    // vuelve a encuadrar a la medida nueva.
    let mut sin_tocar = true;
    let mut panel = false;
    // El clic empezo en el panel: al soltar no se elige nada.
    let mut bajo_en_panel = false;
    // Aislado: el unico elemento que se ve y su caja.
    let mut aislado: Option<(u32, [f32; 6])> = None;
    let mut pista_hasta = Some(Instant::now() + Duration::from_secs(6));
    let (mut dentro_plano, mut dentro_barra) = (false, false);
    let mut fuera_desde: Option<Instant> = None;
    let mut barra_visible = false;
    // Arrastre: el ultimo punto, la camara alli, que boton, el pivote y si
    // ya se movio.
    let mut arrastre: Option<(i32, i32, Orbita, u8, [f64; 3], bool)> = None;
    let mut boton_encima: Option<usize> = None;
    let mut sucio = true;
    let mut barra_sucia = true;
    // El ultimo punto tocado por la rueda: se reusa mientras se sigue girando.
    let mut rueda: Option<(Instant, i32, i32, [f64; 3])> = None;
    let mut aviso_ui: Option<(Modelo, PlanoGpu)> = None;
    let vista = |cam: &Orbita, w: u32, h: u32, radio: f64, elegido: Option<u32>, seccion: Option<[f32; 6]>, aislado: Option<u32>, claro: bool| {
        let (r, u, f) = cam.ejes();
        let luz = unit(suma(suma(por(f, -0.5), por(u, 0.75)), por(r, 0.35)));
        let ojo = cam.ojo();
        Vista3d {
            mvp: cam.matriz(w, h, radio),
            luz: [luz[0] as f32, luz[1] as f32, luz[2] as f32, 0.0],
            ojo: [ojo[0] as f32, ojo[1] as f32, ojo[2] as f32, 1.0],
            corte: [if seccion.is_some() { 1.0 } else { 0.0 }, 0.0, (4.5 * e) as f32, 0.0],
            sec_min: seccion.map_or([0.0; 4], |c| [c[0], c[1], c[2], 0.0]),
            sec_max: seccion.map_or([0.0; 4], |c| [c[3], c[4], c[5], 0.0]),
            elegido: elegido.map_or(0, |e| e + 1),
            claro: if claro { 1.0 } else { 0.0 },
            pantalla: [w as f32, h as f32],
            aislado: aislado.map_or(0, |e| e + 1),
            relleno: [0.0; 3],
        }
    };
    'bucle: loop {
        let animando = cam != destino;
        // SAFETY: bucle de mensajes de este hilo.
        unsafe {
            if !animando && !sucio && !barra_sucia {
                let espera = if modelo.is_none() || barra_visible || fuera_desde.is_some() || pista_hasta.is_some() { 50 } else { INFINITE };
                let _ = MsgWaitForMultipleObjectsEx(None, espera, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        if modelo.is_none() && mensaje == ui.abriendo {
            let llegado = match cargando.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(String::new())),
            };
            if let Some(r) = llegado {
                match r {
                    Ok(m) if m.vacio() => mensaje = ui.vacio.clone(),
                    Ok(m) => match gpu.subir_3d(&m) {
                        Ok(g) => {
                            let c = m.caja;
                            radio = ((c[3] - c[0]).hypot(c[4] - c[1]).hypot(c[5] - c[2]) as f64 / 2.0).max(1e-3);
                            caja_vista = m.caja_util();
                            if let Some(a) = extras.encuadre {
                                let o = m.origen;
                                let r = |k: usize| (a[k] - o[k % 3]) as f32;
                                caja_vista = [r(0), r(1), r(2), r(3), r(4), r(5)];
                            }
                            arbol_m = Some(Arbol::de(&m, &ui));
                            cam = encuadre(caja_vista, w, h);
                            destino = cam;
                            modelo = Some((m, g));
                            mensaje.clear();
                        }
                        Err(err) => mensaje = format!("{}: {err}", ui.error),
                    },
                    Err(err) if err.is_empty() => mensaje = ui.error.clone(),
                    Err(err) => mensaje = format!("{}: {err}", ui.error),
                }
                aviso_ui = None;
                sucio = true;
            }
        }
        let eventos: Vec<(De, Ev)> = ESTADO.with(|s| s.borrow_mut().eventos.drain(..).collect());
        let b_barra = botones_barra(e, BOTONES.len());
        let boton_en = |x: i32| -> Option<usize> { b_barra.iter().position(|(a, b)| (x as f64) >= *a && (x as f64) < *b) };
        let caja = caja_vista;
        let (mut cambiar_tema, mut cambiar_corte, mut cambiar_encima, mut encuadrar, mut planta) = (false, false, false, false, false);
        // Cambio lo que se ve en el panel: hay que rehacer los indices.
        let mut refiltrar = false;
        let mut cambiar_panel = false;
        // Aislar lo elegido (Some(e)) o volver a todo (None).
        let mut aislar: Option<Option<u32>> = None;
        // Una vista guardada del croquis, pedida por el menu o su tecla.
        let mut vista_pedida: Option<usize> = None;
        // Lo que se ve: el aislado o el modelo.
        let caja = aislado.map_or(caja, |a| a.1);
        // Hasta donde puede ir la caja: el modelo entero y algo mas.
        let limite = modelo.as_ref().map_or([0f32; 6], |(m, _)| caja_inicial(caja_inicial(m.caja)));
        for (de, ev) in eventos {
            if matches!((de, &ev), (De::Plano, Ev::Tecla(_))) {
                sin_tocar = false;
            }
            match (de, ev) {
                (_, Ev::Cerrar) => break 'bucle,
                (De::Barra, Ev::Mover(x, _)) => {
                    dentro_barra = true;
                    let b = boton_en(x);
                    if b != boton_encima {
                        boton_encima = b;
                        barra_sucia = true;
                    }
                }
                (De::Barra, Ev::Salir) => {
                    dentro_barra = false;
                    boton_encima = None;
                    barra_sucia = true;
                }
                (De::Barra, Ev::Bajar(x, _, 0)) => match boton_en(x) {
                    Some(0) => {
                        // SAFETY: ventanas propias.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
                        }
                    }
                    Some(1) => cambiar_tema = true,
                    Some(2) => cambiar_corte = true,
                    Some(3) => cambiar_panel = true,
                    Some(4) => cambiar_encima = true,
                    _ => {}
                },
                (De::Barra, _) => {}
                (De::Plano, Ev::Tamano(nw, nh)) => {
                    if (nw, nh) != (w, h) {
                        w = nw;
                        h = nh;
                        let _ = gpu.redimensionar(w, h);
                        aviso_ui = None;
                        sucio = true;
                        if sin_tocar && modelo.is_some() {
                            cam = encuadre(caja_vista, w, h);
                            destino = cam;
                        }
                    }
                }
                (De::Plano, Ev::Mover(x, y)) => {
                    dentro_plano = true;
                    if let Some((k, x0, y0, v0, por_unidad)) = tirador {
                        // La cara sigue al raton a lo largo de su eje.
                        let l2 = por_unidad[0] * por_unidad[0] + por_unidad[1] * por_unidad[1];
                        if l2 > 1e-9
                            && let Some(c) = seccion
                        {
                            let d = ((x - x0) as f64 * por_unidad[0] + (y - y0) as f64 * por_unidad[1]) / l2;
                            let signo = if k < 3 { -1.0 } else { 1.0 };
                            seccion = Some(mover_cara(c, k, v0 + signo * d, limite));
                            sucio = true;
                        }
                    } else if let Some((x0, y0, c0, b, pivote, movido)) = arrastre {
                        let (dx, dy) = ((x - x0) as f64, (y - y0) as f64);
                        if movido || dx.abs() + dy.abs() > 3.0 {
                            // SAFETY: lee el estado de Mayus.
                            let mayus = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                            // **Girar**: poco a poco desde el ultimo punto (al
                            // llegar a la planta y volver responde en seguida)
                            // y media vuelta por el lado corto de la ventana,
                            // sea del tamano que sea.
                            let k = std::f64::consts::PI / w.min(h).max(1) as f64;
                            cam = if b == 0 && !mayus { c0.girar(-dx * k, dy * k, pivote) } else { c0.mover(dx, dy, h) };
                            destino = cam;
                            arrastre = Some((x, y, cam, b, pivote, true));
                            sucio = true;
                        }
                    } else if let Some(c) = seccion {
                        let cv = caja_en_pantalla(c, &cam.matriz(w, h, radio), w, h, None);
                        let encima = tirador_en(&cv, x, y, e);
                        if encima != tirador_encima {
                            tirador_encima = encima;
                            sucio = true;
                        }
                    }
                }
                (De::Plano, Ev::Salir) => dentro_plano = false,
                (De::Plano, Ev::Bajar(x, y, b)) => {
                    pista_hasta = None;
                    sin_tocar = false;
                    // SAFETY: lee el estado de Ctrl.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    if panel
                        && let Some(ar) = &mut arbol_m
                        && ar.dentro(x, y, h, e)
                    {
                        // **El panel**: mostrar u ocultar (Ctrl: solo ese).
                        bajo_en_panel = true;
                        if b == 0 && ar.tocar(x, y, ctrl, &ui, h, e) {
                            refiltrar = true;
                        }
                        sucio = true;
                    } else if b == 0 && ctrl {
                        // SAFETY: ventana propia.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
                        }
                    } else if let Some(c) = seccion
                        && b == 0
                        && let Some(k) = tirador_en(&caja_en_pantalla(c, &cam.matriz(w, h, radio), w, h, None), x, y, e)
                    {
                        // Un tirador de la caja: cuantos pixeles por unidad
                        // se mueve su cara, para que siga al raton.
                        let m = cam.matriz(w, h, radio);
                        let p = centro_de_cara(c, k);
                        let lado = decimo(c);
                        let por_unidad = match (en_pantalla(&m, p, w, h), en_pantalla(&m, suma(p, por(fuera_de_cara(k), lado)), w, h)) {
                            (Some(a), Some(b)) => [(b[0] - a[0]) / lado, (b[1] - a[1]) / lado],
                            _ => [0.0, 0.0],
                        };
                        tirador = Some((k, x, y, c[k] as f64, por_unidad));
                        sucio = true;
                    } else if let Some((_, g)) = &modelo {
                        // Girar alrededor de lo que se toca; si no se toca
                        // nada, del centro de lo que se ve (la caja, si esta
                        // puesta), para que el modelo no salga disparado.
                        cam = destino;
                        let v = vista(&cam, w, h, radio, elegido, seccion, aislado.map(|a| a.0), claro);
                        let pivote = if b == 0 { gpu.elegir(g, &v, x, y).ok().flatten().map(|(_, p)| p.map(|c| c as f64)) } else { None };
                        let c = seccion.unwrap_or(caja);
                        let centro = [(c[0] + c[3]) as f64 / 2.0, (c[1] + c[4]) as f64 / 2.0, (c[2] + c[5]) as f64 / 2.0];
                        // El centro, si queda delante del ojo; si no, el objetivo.
                        let delante = punto(sub(centro, cam.ojo()), cam.adelante()) > 0.0;
                        let pivote = pivote.unwrap_or(if delante { centro } else { cam.objetivo });
                        arrastre = Some((x, y, cam, b, pivote, false));
                    }
                }
                (De::Plano, Ev::Subir(x, y, b)) => {
                    let era_tirador = tirador.take().is_some();
                    let movido = arrastre.is_some_and(|a| a.5);
                    arrastre = None;
                    if std::mem::take(&mut bajo_en_panel) {
                        // Del panel: ya se hizo al bajar.
                    } else if era_tirador || movido {
                        // Fuera el pivote y la medida de la cara.
                        sucio = true;
                    } else if b == 2 {
                        let a = if aislado.is_some() { Some(true) } else { elegido.map(|_| false) };
                        menu(hwnd, &ui, seccion.is_some(), a, panel, aristas, fijada, &nombres_de_vistas, extras.exportar.as_ref().map(|x| x.texto.as_str()));
                    } else if b == 0
                        && let Some((_, g)) = &modelo
                    {
                        let v = vista(&cam, w, h, radio, elegido, seccion, aislado.map(|a| a.0), claro);
                        let nuevo = gpu.elegir(g, &v, x, y).ok().flatten().map(|(e, _)| e);
                        if nuevo.is_some() && nuevo == elegido && aislado.is_none_or(|a| Some(a.0) != nuevo) {
                            // **Otro clic en el elegido: aislarlo.**
                            aislar = Some(nuevo);
                        } else if nuevo != elegido {
                            elegido = nuevo;
                            sucio = true;
                        }
                    }
                }
                (De::Plano, Ev::Doble(x, y))
                    if panel && arbol_m.as_ref().is_some_and(|ar| ar.dentro(x, y, h, e)) =>
                {
                    // Un doble clic en el panel son dos clics.
                    // SAFETY: lee el estado de Ctrl.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    if let Some(ar) = &mut arbol_m
                        && ar.tocar(x, y, ctrl, &ui, h, e)
                    {
                        refiltrar = true;
                    }
                    bajo_en_panel = true;
                    sucio = true;
                }
                (De::Plano, Ev::Doble(x, y)) => {
                    // En un elemento lo aisla el clic que sigue (el de soltar);
                    // en el vacio, todo el modelo.
                    let hay = modelo.as_ref().is_some_and(|(_, g)| {
                        let v = vista(&cam, w, h, radio, elegido, seccion, aislado.map(|a| a.0), claro);
                        gpu.elegir(g, &v, x, y).ok().flatten().is_some()
                    });
                    if !hay {
                        if aislado.is_some() {
                            aislar = Some(None);
                        } else {
                            encuadrar = true;
                        }
                    }
                }
                (De::Plano, Ev::Rueda(x, y, d)) => {
                    pista_hasta = None;
                    sin_tocar = false;
                    if panel
                        && let Some(ar) = &mut arbol_m
                        && ar.dentro(x, y, h, e)
                    {
                        ar.rueda(d, h, e);
                        sucio = true;
                        continue;
                    }
                    let factor = 1.2f64.powf(-(d as f64) / 120.0);
                    // El punto bajo el cursor: el tocado (si se sigue girando
                    // la rueda en el mismo sitio, el de antes).
                    let fijo = match rueda {
                        Some((t, rx, ry, p)) if t.elapsed() < Duration::from_millis(400) && (rx - x).abs() + (ry - y).abs() < 4 => Some(p),
                        _ => None,
                    };
                    let fijo = fijo.or_else(|| {
                        let (_, g) = modelo.as_ref()?;
                        let v = vista(&destino, w, h, radio, elegido, seccion, aislado.map(|a| a.0), claro);
                        gpu.elegir(g, &v, x, y).ok().flatten().map(|(_, p)| p.map(|c| c as f64))
                    });
                    // Sin nada debajo: el punto del rayo a la distancia del objetivo.
                    let fijo = fijo.unwrap_or_else(|| suma(destino.ojo(), por(destino.rayo(x as f64, y as f64, w, h), destino.dist)));
                    rueda = Some((Instant::now(), x, y, fijo));
                    destino = destino.zoom(factor, fijo);
                }
                (De::Plano, Ev::Menu(id)) => match id {
                    M_TODO => encuadrar = true,
                    M_TEMA => cambiar_tema = true,
                    M_CORTE => cambiar_corte = true,
                    M_AISLAR => aislar = Some(if aislado.is_some() { None } else { elegido }),
                    M_PANEL => cambiar_panel = true,
                    M_ARISTAS => {
                        aristas = !aristas;
                        sucio = true;
                    }
                    M_PLANTA => planta = true,
                    M_EXPORTAR => {
                        if let Some(x) = &extras.exportar {
                            (x.hacer)(foto_de_la_ventana(hwnd));
                        }
                    }
                    M_ENCIMA => cambiar_encima = true,
                    M_CERRAR => break 'bucle,
                    n if (M_VISTA..M_VISTA + extras.vistas.len() as u32).contains(&n) => vista_pedida = Some((n - M_VISTA) as usize),
                    _ => {}
                },
                (De::Plano, Ev::TeclaSoltada(_) | Ev::Caracter(_)) => {}
                (De::Plano, Ev::Tecla(vk)) => match vk {
                    0x1B => {
                        if aislado.is_some() {
                            aislar = Some(None);
                        } else if elegido.is_some() {
                            elegido = None;
                            sucio = true;
                        } else {
                            break 'bucle;
                        }
                    }
                    0x46 | 0x24 => encuadrar = true,
                    0x42 => cambiar_tema = true,
                    0x43 => cambiar_corte = true,
                    0x49 => aislar = Some(if aislado.is_some() { None } else { elegido }),
                    0x4C => cambiar_panel = true,
                    0x41 => {
                        aristas = !aristas;
                        sucio = true;
                    }
                    0x50 => planta = true,
                    0x54 => cambiar_encima = true,
                    // 1 a 9: las vistas guardadas del croquis.
                    0x31..=0x39 => vista_pedida = Some((vk - 0x31) as usize),
                    0xBB | 0x6B => destino = destino.zoom(1.0 / 1.4, destino.objetivo),
                    0xBD | 0x6D => destino = destino.zoom(1.4, destino.objetivo),
                    0x25 => destino = destino.girar(0.26, 0.0, destino.objetivo),
                    0x27 => destino = destino.girar(-0.26, 0.0, destino.objetivo),
                    0x26 => destino = destino.girar(0.0, 0.2, destino.objetivo),
                    0x28 => destino = destino.girar(0.0, -0.2, destino.objetivo),
                    _ => {}
                },
            }
        }
        if let Some(que) = aislar
            && let Some((m, _)) = &modelo
        {
            // Aislar y encuadrarlo; o volver a todo, encuadrado.
            aislado = que.and_then(|e| Some((e, caja_de_elemento(m, e)?)));
            let c = aislado.map_or(caja_vista, |a| a.1);
            destino = encuadre(c, w, h);
            if let Some(e) = que {
                elegido = Some(e);
            }
            sucio = true;
        }
        if let Some(i) = vista_pedida
            && let (Some(v), Some((m, _))) = (extras.vistas.get(i), &modelo)
        {
            destino = crate::croquis::orbita_de(v, m.origen, w, h, e);
            sin_tocar = false;
        }
        let caja = aislado.map_or(caja_vista, |a| a.1);
        if encuadrar && modelo.is_some() {
            destino = encuadre(caja, w, h);
        }
        if planta && modelo.is_some() {
            let mut o = Orbita::encuadrar(caja, w, h);
            o.rumbo = -std::f64::consts::FRAC_PI_2;
            o.altura = ALTURA_MAX;
            o.dist = o.distancia_para(caja, w, h).max(1e-3);
            destino = o;
        }
        if cambiar_panel {
            panel = !panel;
            barra_sucia = true;
            sucio = true;
        }
        if refiltrar
            && let (Some((m, g)), Some(ar)) = (&mut modelo, &arbol_m)
        {
            let ver = |k: u32| m.elementos.get(k as usize).is_none_or(|el| ar.visible(el));
            if let Err(err) = gpu.filtrar_3d(g, m, &ver) {
                tracing::warn!(error = %err, "no se pudo ocultar");
            }
            // Lo elegido, si se oculto, se suelta.
            if elegido.is_some_and(|k| !ver(k)) {
                elegido = None;
            }
        }
        if cambiar_tema {
            claro = !claro;
            tema_tocado = true;
            barra_sucia = true;
            aviso_ui = None;
            sucio = true;
        }
        if cambiar_corte && modelo.is_some() {
            seccion = if seccion.is_some() { None } else { Some(caja_inicial(caja)) };
            tirador = None;
            tirador_encima = None;
            barra_sucia = true;
            sucio = true;
        }
        if cambiar_encima {
            fijada = !fijada;
            poner_encima(hwnd, fijada);
            barra_sucia = true;
        }
        if pista_hasta.is_some_and(|t| Instant::now() > t) {
            pista_hasta = None;
            sucio = true;
        }
        // La barra, como en los planos.
        let quiere = dentro_plano || dentro_barra || arrastre.is_some();
        if quiere {
            fuera_desde = None;
        } else if barra_visible && fuera_desde.is_none() {
            fuera_desde = Some(Instant::now());
        }
        let mostrar = quiere || fuera_desde.is_some_and(|t| t.elapsed() < Duration::from_millis(600));
        if !mostrar {
            fuera_desde = None;
        }
        if mostrar && modelo.is_some() {
            if barra_visible {
                seguir_barra(hwnd);
            } else {
                let mut r = RECT::default();
                // SAFETY: ventanas propias; estructuras locales.
                unsafe {
                    let _ = GetWindowRect(hwnd, &mut r);
                    let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                    let _ = GetMonitorInfoW(mon, &mut info);
                    let (x, y) = colocar_barra(r, info.rcWork, bw, bh, (8.0 * e) as i32);
                    let _ = SetWindowPos(hbarra, Some(if fijada { HWND_TOPMOST } else { HWND_NOTOPMOST }), x, y, bw, bh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
                }
                barra_visible = true;
                barra_sucia = true;
            }
        } else if barra_visible {
            barra_visible = false;
            // SAFETY: ventana propia.
            let _ = unsafe { ShowWindow(hbarra, SW_HIDE) };
        }
        if cam != destino {
            let c = cam.hacia(&destino, 0.35);
            cam = if c.cerca_de(&destino) { destino } else { c };
            sucio = true;
        }
        if barra_sucia && barra_visible {
            let m = barra(e, &BOTONES, boton_encima, claro, &[false, false, seccion.is_some(), panel, fijada]);
            if let Ok(p) = gpu_barra.subir(&m) {
                let (gw, gh) = gpu_barra.tamano();
                let fondo = if claro { [0.976, 0.976, 0.984, 1.0] } else { [0.118, 0.118, 0.125, 1.0] };
                let _ = gpu_barra.dibujar(fondo, &[(&p, vista_pantalla(&m, gw, gh))]);
            }
            barra_sucia = false;
        }
        if aviso_ui.is_none() && !mensaje.is_empty() {
            let m = aviso(&mut textos, &mensaje, w, h, e, claro);
            aviso_ui = gpu.subir(&m).ok().map(|p| (m, p));
            sucio = true;
        }
        if sucio {
            let fondo = match extras.fondo {
                Some(f) if !tema_tocado => f,
                _ if claro => [0.955, 0.958, 0.965, 1.0],
                _ => [0.13, 0.15, 0.19, 1.0],
            };
            let mut capas: Vec<(&PlanoGpu, crate::gpu::Vista)> = Vec::new();
            if mensaje.is_empty() {
                aviso_ui = None;
            } else if let Some((m, p)) = &aviso_ui {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            let datos = Encima {
                elegido: modelo.as_ref().and_then(|(m, _)| Some(arbol::describir(m.elementos.get(elegido? as usize)?, m, unidad_de(m.metros)))),
                arbol: arbol_m.as_ref().filter(|_| panel),
                caja: seccion.map(|c| caja_en_pantalla(c, &cam.matriz(w, h, radio), w, h, tirador_encima)),
                cara: tirador.and_then(|t| Some((t.0, modelo.as_ref()?.0.origen[t.0 % 3] + seccion?[t.0] as f64))),
                pivote: arrastre.filter(|a| a.5 && a.3 == 0).and_then(|a| en_pantalla(&cam.matriz(w, h, radio), a.4, w, h)),
                unidad: modelo.as_ref().map_or("", |(m, _)| unidad_de(m.metros)),
                pista: pista_hasta.is_some().then_some(ui.pista.as_str()),
            };
            let encima = if modelo.is_some() {
                let m = encima_del_modelo(&mut textos, &datos, &ui, w, h, e, claro);
                gpu.subir(&m).ok().map(|p| (m, p))
            } else {
                None
            };
            if let Some((m, p)) = &encima {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            let v = vista(&cam, w, h, radio, elegido, seccion, aislado.map(|a| a.0), claro);
            let r = gpu.dibujar_3d(fondo, modelo.as_ref().map(|(_, g)| (g, v)), aristas, &capas);
            if let Err(err) = r {
                tracing::warn!(error = %err, "no se pudo dibujar el modelo");
            }
            sucio = false;
        }
    }
    // SAFETY: ventanas propias.
    unsafe {
        let _ = DestroyWindow(hbarra);
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_caras_de_la_caja_no_se_cruzan_ni_se_van_lejos() {
        let limite = [0.0, 0.0, 0.0, 10.0, 10.0, 10.0];
        let c = [1.0, 1.0, 1.0, 9.0, 9.0, 9.0];
        // La de arriba baja hasta 4.
        assert_eq!(mover_cara(c, 5, 4.0, limite)[5], 4.0);
        // No pasa de la de abajo, ni del limite.
        assert!(mover_cara(c, 5, -3.0, limite)[5] > 1.0);
        assert_eq!(mover_cara(c, 0, -50.0, limite)[0], 0.0);
        assert!(mover_cara(c, 0, 50.0, limite)[0] < 9.0);
    }

    #[test]
    fn los_tiradores_miran_hacia_fuera() {
        let o = Orbita { objetivo: [5.0, 5.0, 5.0], rumbo: -0.8, altura: 0.5, dist: 40.0 };
        let c = [0.0, 0.0, 0.0, 10.0, 10.0, 10.0];
        let cv = caja_en_pantalla(c, &o.matriz(800, 600, 20.0), 800, 600, None);
        assert_eq!(cv.aristas.len(), 12);
        assert_eq!(cv.tiradores.len(), 6);
        // El de arriba apunta hacia arriba en la pantalla; el de abajo, hacia abajo.
        let arriba = cv.tiradores.iter().find(|t| t.0 == 5).unwrap();
        let abajo = cv.tiradores.iter().find(|t| t.0 == 2).unwrap();
        assert!(arriba.2[1] < -0.5 && abajo.2[1] > 0.5, "{:?} {:?}", arriba.2, abajo.2);
        // Y se encuentra al tocarlo.
        assert_eq!(tirador_en(&cv, arriba.1[0] as i32, arriba.1[1] as i32, 1.0), Some(5));
    }
}
