//! **La ventana del plano, como un pin**: sin bordes, nace sin fijar (se
//! fija con su pin, [`FIJADA_AL_NACER`]), se estira por los bordes. Lo que el usuario pidio el
//! 8-oct-2026: «como un pin normal: una barra fuera del pin que aparece y
//! desaparece», con solo el tema (claro u oscuro) y «acotar plano» para
//! medir.
//!
//! - Rueda: acercar y alejar hacia el cursor, suave.
//! - Arrastrar (izquierdo o central): mover el plano. Ctrl + arrastrar, o el
//!   asa de la barra: mover la ventana.
//! - Doble clic: el plano entero (o, acotando, terminar la medida).
//! - Clic derecho: un menu (todo, tema, acotar, encima, cerrar).
//! - Teclas: Esc (acotando: deja la medida; si no, cierra), F o Inicio
//!   todo, B tema, M acotar, T encima, flechas, + y −.
//!
//! No se dibuja nada si nada cambia: en reposo no gasta.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
    ScreenToClient,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VK_CONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const WM_MOUSELEAVE: u32 = 0x02A3;
const INFINITE: u32 = u32::MAX;

use crate::convertir::rgba;
use crate::gpu::{Gpu, PlanoGpu, Vista};
use crate::modelo::{Constructor, Modelo};
use crate::texto::Textos;

/// De que ventana es un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum De {
    Plano,
    Barra,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Ev {
    Mover(i32, i32),
    Bajar(i32, i32, u8),
    Subir(i32, i32, u8),
    Doble(i32, i32),
    Rueda(i32, i32, i32),
    Tecla(u32),
    TeclaSoltada(u32),
    Caracter(u32),
    Tamano(u32, u32),
    Salir,
    Cerrar,
    Menu(u32),
}

#[derive(Default)]
pub(crate) struct Estado {
    pub(crate) eventos: VecDeque<(De, Ev)>,
    /// La barra y su separacion del plano: el plano la arrastra consigo.
    pub(crate) barra: Option<(HWND, i32)>,
}

thread_local! {
    pub(crate) static ESTADO: RefCell<Estado> = RefCell::new(Estado::default());
}

fn apuntar(de: De, e: Ev) {
    ESTADO.with(|s| s.borrow_mut().eventos.push_back((de, e)));
}

/// Pone la barra junto al plano. Se llama tambien mientras Windows arrastra
/// la ventana (su bucle no deja correr el nuestro), asi la barra la sigue.
/// No cambia el orden Z: subirla cada vez subia tambien el plano por
/// encima del recorte de una captura.
pub(crate) fn seguir_barra(plano: HWND) {
    let Some((barra, sep)) = ESTADO.with(|s| s.borrow().barra) else {
        return;
    };
    // SAFETY: ventanas propias; estructuras locales.
    unsafe {
        if !IsWindowVisible(barra).as_bool() {
            return;
        }
        let (mut r, mut rb) = (RECT::default(), RECT::default());
        let _ = GetWindowRect(plano, &mut r);
        let _ = GetWindowRect(barra, &mut rb);
        let mon = MonitorFromWindow(plano, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(mon, &mut info);
        let (x, y) = colocar_barra(r, info.rcWork, rb.right - rb.left, rb.bottom - rb.top, sep);
        if (x, y) != (rb.left, rb.top) {
            let _ = SetWindowPos(
                barra,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

fn xy(l: LPARAM) -> (i32, i32) {
    (
        (l.0 & 0xffff) as i16 as i32,
        ((l.0 >> 16) & 0xffff) as i16 as i32,
    )
}

unsafe extern "system" fn procedimiento_plano(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    // SAFETY: lo mismo que `procedimiento`.
    unsafe { procedimiento(De::Plano, hwnd, msg, wp, lp) }
}

/// Cada ventana con el suyo: la barra recibe mensajes (su `WM_SIZE`) antes
/// de que se sepa su `HWND`, y con uno solo se tomaban por del plano.
unsafe extern "system" fn procedimiento_barra(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    // SAFETY: lo mismo que `procedimiento`.
    unsafe { procedimiento(De::Barra, hwnd, msg, wp, lp) }
}

unsafe fn procedimiento(de: De, hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCCALCSIZE if wp.0 != 0 && de == De::Plano => return LRESULT(0),
        WM_NCHITTEST if de == De::Plano => {
            let (sx, sy) = xy(lp);
            let mut p = POINT { x: sx, y: sy };
            let mut r = RECT::default();
            // SAFETY: ventana propia; salidas locales.
            unsafe {
                let _ = ScreenToClient(hwnd, &mut p);
                let _ = GetClientRect(hwnd, &mut r);
            }
            let borde = 7;
            let (izq, der, arr, aba) = (
                p.x < borde,
                p.x >= r.right - borde,
                p.y < borde,
                p.y >= r.bottom - borde,
            );
            let ht = match (izq, der, arr, aba) {
                (true, _, true, _) => HTTOPLEFT,
                (_, true, true, _) => HTTOPRIGHT,
                (true, _, _, true) => HTBOTTOMLEFT,
                (_, true, _, true) => HTBOTTOMRIGHT,
                (true, ..) => HTLEFT,
                (_, true, ..) => HTRIGHT,
                (_, _, true, _) => HTTOP,
                (.., true) => HTBOTTOM,
                _ => HTCLIENT,
            };
            return LRESULT(ht as isize);
        }
        WM_MOUSEACTIVATE if de == De::Barra => return LRESULT(MA_NOACTIVATE as isize),
        // Sigue a DefWindowProc: de ahi salen WM_SIZE y WM_MOVE.
        WM_WINDOWPOSCHANGED if de == De::Plano => seguir_barra(hwnd),
        WM_SIZE => {
            let (w, h) = xy(lp);
            apuntar(de, Ev::Tamano(w.max(1) as u32, h.max(1) as u32));
            return LRESULT(0);
        }
        WM_MOUSEMOVE => {
            let (x, y) = xy(lp);
            apuntar(de, Ev::Mover(x, y));
            let mut t = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            // SAFETY: estructura local con su tamano.
            let _ = unsafe { TrackMouseEvent(&mut t) };
            return LRESULT(0);
        }
        WM_MOUSELEAVE => apuntar(de, Ev::Salir),
        WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN => {
            let (x, y) = xy(lp);
            if de == De::Plano {
                // SAFETY: ventana propia.
                unsafe { SetCapture(hwnd) };
            }
            apuntar(
                de,
                Ev::Bajar(
                    x,
                    y,
                    if msg == WM_LBUTTONDOWN {
                        0
                    } else if msg == WM_MBUTTONDOWN {
                        1
                    } else {
                        2
                    },
                ),
            );
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP => {
            let (x, y) = xy(lp);
            // SAFETY: suelta la captura propia.
            let _ = unsafe { ReleaseCapture() };
            apuntar(
                de,
                Ev::Subir(
                    x,
                    y,
                    if msg == WM_LBUTTONUP {
                        0
                    } else if msg == WM_MBUTTONUP {
                        1
                    } else {
                        2
                    },
                ),
            );
            return LRESULT(0);
        }
        WM_LBUTTONDBLCLK => {
            let (x, y) = xy(lp);
            apuntar(de, Ev::Doble(x, y));
            return LRESULT(0);
        }
        WM_MOUSEWHEEL => {
            let (sx, sy) = xy(lp);
            let mut p = POINT { x: sx, y: sy };
            // SAFETY: ventana propia; punto local.
            let _ = unsafe { ScreenToClient(hwnd, &mut p) };
            apuntar(
                de,
                Ev::Rueda(p.x, p.y, ((wp.0 >> 16) & 0xffff) as i16 as i32),
            );
            return LRESULT(0);
        }
        WM_KEYDOWN => {
            apuntar(de, Ev::Tecla(wp.0 as u32));
            return LRESULT(0);
        }
        // Para escribir con el motor del lienzo al anotar.
        WM_KEYUP => {
            apuntar(de, Ev::TeclaSoltada(wp.0 as u32));
            return LRESULT(0);
        }
        WM_CHAR => {
            apuntar(de, Ev::Caracter(wp.0 as u32));
            return LRESULT(0);
        }
        WM_COMMAND => {
            apuntar(de, Ev::Menu((wp.0 & 0xffff) as u32));
            return LRESULT(0);
        }
        WM_CLOSE => {
            apuntar(de, Ev::Cerrar);
            return LRESULT(0);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_GETMINMAXINFO => {
            // SAFETY: en este mensaje `lp` es un MINMAXINFO valido.
            let mm = unsafe { &mut *(lp.0 as *mut MINMAXINFO) };
            mm.ptMinTrackSize = POINT { x: 160, y: 120 };
            return LRESULT(0);
        }
        _ => {}
    }
    // SAFETY: lo demas, a Windows.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn registrar() -> windows::core::Result<windows::Win32::Foundation::HINSTANCE> {
    // SAFETY: clases propias; registrar dos veces falla sin efecto.
    unsafe {
        let inst: windows::Win32::Foundation::HINSTANCE = GetModuleHandleW(None)?.into();
        let icono = LoadIconW(Some(inst), PCWSTR(1 as _)).unwrap_or_default();
        let procs: [(PCWSTR, bool, WNDPROC); 3] = [
            (w!("PixPinPlanoCad"), true, Some(procedimiento_plano)),
            (w!("PixPinModelo3d"), true, Some(procedimiento_plano)),
            (w!("PixPinPlanoBarra"), false, Some(procedimiento_barra)),
        ];
        for (clase, dobles, proc_) in procs {
            let c = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: if dobles {
                    CS_DBLCLKS
                } else {
                    WNDCLASS_STYLES(0)
                },
                lpfnWndProc: proc_,
                hInstance: inst,
                hIcon: icono,
                hIconSm: icono,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: clase,
                ..Default::default()
            };
            RegisterClassExW(&c);
        }
        Ok(inst)
    }
}

pub(crate) fn crear_ventana(
    clase: PCWSTR,
    titulo: &str,
) -> windows::core::Result<(HWND, u32, u32)> {
    let inst = registrar()?;
    // SAFETY: ventana propia; cadenas vivas durante cada llamada.
    unsafe {
        // En el monitor del raton, el 60 % de su area de trabajo.
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let mon = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(mon, &mut info);
        let t = info.rcWork;
        let (aw, ah) = (t.right - t.left, t.bottom - t.top);
        let (w, h) = ((aw as f32 * 0.6) as i32, (ah as f32 * 0.65) as i32);
        let titulo: Vec<u16> = titulo.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_TOPMOST,
            clase,
            PCWSTR(titulo.as_ptr()),
            WS_POPUP | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU,
            t.left + (aw - w) / 2,
            t.top + (ah - h) / 2 + (ah as f32 * 0.03) as i32,
            w,
            h,
            None,
            None,
            Some(inst),
            None,
        )?;
        let si: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &si as *const i32 as *const _,
            4,
        );
        // Un pixel de marco de DWM: la sombra de las ventanas de Windows.
        let _ = DwmExtendFrameIntoClientArea(
            hwnd,
            &MARGINS {
                cxLeftWidth: 1,
                cxRightWidth: 1,
                cyTopHeight: 1,
                cyBottomHeight: 1,
            },
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        // Nace TOPMOST (asi sale delante aunque Windows no deje darle el
        // foco) y se baja en seguida a una ventana mas: al frente, sin fijar.
        poner_encima(hwnd, FIJADA_AL_NACER);
        Ok((hwnd, w as u32, h as u32))
    }
}

/// La barra: una ventanita sin activar, duena del plano (va siempre encima
/// de el), con las esquinas redondas de Windows 11.
pub(crate) fn crear_barra(dueno: HWND, w: i32, h: i32) -> windows::core::Result<HWND> {
    let inst = registrar()?;
    // SAFETY: ventana propia, nace oculta.
    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            w!("PixPinPlanoBarra"),
            w!(""),
            WS_POPUP,
            0,
            0,
            w,
            h,
            Some(dueno),
            None,
            Some(inst),
            None,
        )?;
        let redondas: i32 = 2;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &redondas as *const i32 as *const _,
            4,
        );
        Ok(hwnd)
    }
}

/// La camara: que punto del plano (respecto a su origen) esta en el centro
/// y cuantas unidades del plano mide un pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camara {
    pub centro: [f64; 2],
    pub px: f64,
}

impl Camara {
    /// El plano entero en una ventana de `w` x `h`, con un margen.
    pub fn encuadrar(caja: [f32; 4], w: u32, h: u32) -> Camara {
        let (cw, ch) = (
            (caja[2] - caja[0]).max(1e-9) as f64,
            (caja[3] - caja[1]).max(1e-9) as f64,
        );
        let px = (cw / (w.max(1) as f64 * 0.92)).max(ch / (h.max(1) as f64 * 0.92));
        Camara {
            centro: [
                (caja[0] + caja[2]) as f64 / 2.0,
                (caja[1] + caja[3]) as f64 / 2.0,
            ],
            px: if px.is_finite() && px > 0.0 { px } else { 1.0 },
        }
    }

    /// El punto del plano bajo el pixel (x, y) de una ventana `w` x `h`.
    pub fn plano(&self, x: f64, y: f64, w: u32, h: u32) -> [f64; 2] {
        [
            self.centro[0] + (x - w as f64 / 2.0) * self.px,
            self.centro[1] - (y - h as f64 / 2.0) * self.px,
        ]
    }

    /// El pixel de un punto del plano.
    pub fn pantalla(&self, p: [f64; 2], w: u32, h: u32) -> [f64; 2] {
        [
            (p[0] - self.centro[0]) / self.px + w as f64 / 2.0,
            h as f64 / 2.0 - (p[1] - self.centro[1]) / self.px,
        ]
    }

    /// Acercar `factor` (>1 aleja) dejando quieto el punto bajo (x, y).
    pub fn zoom(&self, factor: f64, x: f64, y: f64, w: u32, h: u32) -> Camara {
        let antes = self.plano(x, y, w, h);
        let px = (self.px * factor).clamp(1e-9, 1e9);
        let mut c = Camara {
            centro: self.centro,
            px,
        };
        let despues = c.plano(x, y, w, h);
        c.centro = [
            c.centro[0] + antes[0] - despues[0],
            c.centro[1] + antes[1] - despues[1],
        ];
        c
    }

    fn vista(&self, w: u32, h: u32, color7: [f32; 4], modo: f32) -> Vista {
        Vista {
            escala: [
                (2.0 / (w as f64 * self.px)) as f32,
                (2.0 / (h as f64 * self.px)) as f32,
            ],
            centro: [self.centro[0] as f32, self.centro[1] as f32],
            color7,
            px: self.px as f32,
            modo,
            relleno: [0.0; 2],
        }
    }
}

pub use crate::enganche::{Enganches, medida, sufijo};

// ------------------------------------------------------------------ pintar

const BOTONES: [&str; 8] = [
    "asa", "tema", "acotar", "marcos", "anotar", "tres", "letras", "fijar",
];
pub(crate) const ALTO_BARRA: f64 = 48.0;
const BOTON: f64 = 40.0;

/// Lo que mide la barra con `n` botones (el asa cuenta como uno).
pub(crate) fn ancho_barra(e: f64, n: usize) -> f64 {
    (4.0 + 18.0 + (2.0 + BOTON) * (n.max(1) - 1) as f64 + 4.0) * e
}

/// Donde cae cada boton de la barra (x desde, x hasta); el primero es el asa.
pub(crate) fn botones_barra(e: f64, n: usize) -> Vec<(f64, f64)> {
    let x0 = 4.0 * e;
    let mut v = vec![(x0, x0 + 18.0 * e)];
    for _ in 1..n {
        let a = v[v.len() - 1].1 + 2.0 * e;
        v.push((a, a + BOTON * e));
    }
    v
}

/// Lo que va encima se dibuja despues: el modelo ordena por tamano (mayor
/// primero) y por sitio, asi que en la barra y en las cotas el «tamano» es
/// la capa: fondo, rayas, pastillas y marcas, en ese orden.
pub(crate) const CAPA_FONDO: f64 = 1e9;
pub(crate) const CAPA_RAYAS: f64 = 1e7;
pub(crate) const CAPA_ENCIMA: f64 = 1e5;
const CAPA_ARRIBA: f64 = 1e3;

pub(crate) fn rect_en(
    c: &mut Constructor,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    color: u32,
    capa: f64,
) {
    c.triangulos(
        &[[x0, y0], [x1, y0], [x0, y1], [x1, y0], [x1, y1], [x0, y1]],
        color,
        Some(capa),
    );
}

pub(crate) fn rect(c: &mut Constructor, x0: f64, y0: f64, x1: f64, y1: f64, color: u32) {
    rect_en(c, x0, y0, x1, y1, color, CAPA_ENCIMA);
}

/// Una raya gruesa (de `g` pixeles) en pantalla.
pub(crate) fn raya(c: &mut Constructor, a: [f64; 2], b: [f64; 2], g: f64, color: u32) {
    raya_en(c, a, b, g, color, CAPA_RAYAS);
}

pub(crate) fn raya_en(
    c: &mut Constructor,
    a: [f64; 2],
    b: [f64; 2],
    g: f64,
    color: u32,
    capa: f64,
) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy);
    if l < 1e-9 {
        return;
    }
    let (nx, ny) = (-dy / l * g / 2.0, dx / l * g / 2.0);
    c.triangulos(
        &[
            [a[0] + nx, a[1] + ny],
            [a[0] - nx, a[1] - ny],
            [b[0] + nx, b[1] + ny],
            [a[0] - nx, a[1] - ny],
            [b[0] - nx, b[1] - ny],
            [b[0] + nx, b[1] + ny],
        ],
        color,
        Some(capa),
    );
}

/// La barra: `botones` por nombre (el primero, el asa) y cuales estan
/// activos (en azul): acotar mientras se mide, el pin si el plano esta
/// siempre encima de las demas ventanas.
pub(crate) fn barra(
    e: f64,
    botones: &[&str],
    encima: Option<usize>,
    claro: bool,
    activos: &[bool],
) -> Modelo {
    barra_con_avisos(e, botones, encima, claro, activos, &[])
}

/// Como [`barra`], y los botones de `avisos` en ambar (el «Aa» cuando el
/// plano pide letras que no estan, como en Android).
pub(crate) fn barra_con_avisos(
    e: f64,
    botones: &[&str],
    encima: Option<usize>,
    claro: bool,
    activos: &[bool],
    avisos: &[bool],
) -> Modelo {
    let mut c = Constructor::nuevo();
    let (fondo, tinta, hover, tenue) = if claro {
        (
            rgba(0xF9, 0xF9, 0xFB),
            rgba(0x1C, 0x1C, 0x1E),
            0x14000000u32,
            rgba(0x6E, 0x6E, 0x73),
        )
    } else {
        (
            rgba(0x1E, 0x1E, 0x20),
            rgba(0xF5, 0xF5, 0xF7),
            0x24ffffffu32,
            rgba(0xC7, 0xC7, 0xCC),
        )
    };
    let (w, h) = (ancho_barra(e, botones.len()), ALTO_BARRA * e);
    rect_en(&mut c, 0.0, 0.0, w, h, fondo, CAPA_FONDO);
    let b = botones_barra(e, botones.len());
    let y0 = (h - BOTON * e) / 2.0;
    let azul = rgba(0x00, 0x60, 0xDF);
    for (i, (x0, x1)) in b.iter().enumerate() {
        let activo = activos.get(i).copied().unwrap_or(false);
        if activo {
            rect_en(
                &mut c,
                *x0,
                y0,
                *x1,
                y0 + BOTON * e,
                azul,
                CAPA_RAYAS * 10.0,
            );
        } else if encima == Some(i) && i > 0 {
            rect_en(
                &mut c,
                *x0,
                y0,
                *x1,
                y0 + BOTON * e,
                hover,
                CAPA_RAYAS * 10.0,
            );
        }
        let (cx, cy) = ((x0 + x1) / 2.0, h / 2.0);
        let r = 8.0 * e;
        let col = if activo {
            rgba(255, 255, 255)
        } else if avisos.get(i).copied().unwrap_or(false) {
            rgba(0xFF, 0xB0, 0x20)
        } else {
            tinta
        };
        match botones[i] {
            "letras" => {
                // «Aa»: una A grande y una a pequeña, de rayas.
                let p = |x: f64, y: f64| [cx + x * e, cy + y * e];
                raya(&mut c, p(-10.0, 7.0), p(-5.0, -7.0), 1.6 * e, col);
                raya(&mut c, p(-5.0, -7.0), p(0.0, 7.0), 1.6 * e, col);
                raya(&mut c, p(-8.0, 2.5), p(-2.0, 2.5), 1.4 * e, col);
                let anillo: Vec<[f64; 2]> = (0..=24)
                    .map(|k| {
                        let a = k as f64 / 24.0 * std::f64::consts::TAU;
                        p(5.5 + 3.4 * a.cos(), 3.6 + 3.4 * a.sin())
                    })
                    .collect();
                for v in anillo.windows(2) {
                    raya(&mut c, v[0], v[1], 1.4 * e, col);
                }
                raya(&mut c, p(8.9, -0.5), p(8.9, 7.0), 1.4 * e, col);
            }
            "asa" => {
                // Seis puntos: de aqui se arrastra la ventana.
                for (dx, dy) in [
                    (-3.0, -6.0),
                    (3.0, -6.0),
                    (-3.0, 0.0),
                    (3.0, 0.0),
                    (-3.0, 6.0),
                    (3.0, 6.0),
                ] {
                    let (px, py) = (cx + dx * e, cy + dy * e);
                    rect(
                        &mut c,
                        px - 1.2 * e,
                        py - 1.2 * e,
                        px + 1.2 * e,
                        py + 1.2 * e,
                        tenue,
                    );
                }
            }
            "tema" => {
                let circ: Vec<[f64; 2]> = (0..=40)
                    .map(|k| {
                        let a = k as f64 / 40.0 * std::f64::consts::TAU;
                        [cx + r * a.cos(), cy + r * a.sin()]
                    })
                    .collect();
                for v in circ.windows(2) {
                    raya(&mut c, v[0], v[1], 1.6 * e, col);
                }
                let mut tri = Vec::new();
                for k in 0..20 {
                    let a0 = -std::f64::consts::FRAC_PI_2 + k as f64 / 20.0 * std::f64::consts::PI;
                    let a1 =
                        -std::f64::consts::FRAC_PI_2 + (k + 1) as f64 / 20.0 * std::f64::consts::PI;
                    tri.extend_from_slice(&[
                        [cx, cy],
                        [cx + r * a0.cos(), cy + r * a0.sin()],
                        [cx + r * a1.cos(), cy + r * a1.sin()],
                    ]);
                }
                c.triangulos(&tri, col, Some(CAPA_RAYAS));
            }
            "fijar" => {
                // Una chincheta inclinada: cabeza, cuerpo, ala y aguja.
                let (ux, uy) = (
                    std::f64::consts::FRAC_1_SQRT_2,
                    -std::f64::consts::FRAC_1_SQRT_2,
                );
                let (nx, ny) = (-uy, ux);
                // El eje va de arriba-derecha (cabeza) a abajo-izquierda (punta).
                let q =
                    |t: f64, s: f64| [cx - ux * t * e + nx * s * e, cy - uy * t * e + ny * s * e];
                // Cabeza redonda, cuerpo, ala ancha y la aguja.
                let cab: Vec<[f64; 2]> = (0..=24)
                    .map(|k| {
                        let a = k as f64 / 24.0 * std::f64::consts::TAU;
                        let m = q(-8.0, 0.0);
                        [m[0] + 4.2 * e * a.cos(), m[1] + 4.2 * e * a.sin()]
                    })
                    .collect();
                let centro_cab = q(-8.0, 0.0);
                let mut tri = Vec::new();
                for v in cab.windows(2) {
                    tri.extend_from_slice(&[centro_cab, v[0], v[1]]);
                }
                let cuerpo = [q(-8.0, -3.0), q(-8.0, 3.0), q(-1.5, 2.2), q(-1.5, -2.2)];
                tri.extend_from_slice(&[
                    cuerpo[0], cuerpo[1], cuerpo[2], cuerpo[0], cuerpo[2], cuerpo[3],
                ]);
                let ala = [q(-1.5, -5.5), q(-1.5, 5.5), q(1.0, 5.5), q(1.0, -5.5)];
                tri.extend_from_slice(&[ala[0], ala[1], ala[2], ala[0], ala[2], ala[3]]);
                c.triangulos(&tri, col, Some(CAPA_RAYAS));
                raya(&mut c, q(1.0, 0.0), q(9.5, 0.0), 1.6 * e, col);
            }
            "corte" => {
                // Un cubo en perspectiva con un plano que lo corta.
                let p = |x: f64, y: f64| [cx + x * e, cy + y * e];
                for (a, b2) in [
                    ((-7.0, -3.0), (2.0, -3.0)),
                    ((2.0, -3.0), (2.0, 8.0)),
                    ((2.0, 8.0), (-7.0, 8.0)),
                    ((-7.0, 8.0), (-7.0, -3.0)),
                    ((-7.0, -3.0), (-2.0, -8.0)),
                    ((2.0, -3.0), (7.0, -8.0)),
                    ((-2.0, -8.0), (7.0, -8.0)),
                    ((7.0, -8.0), (7.0, 3.0)),
                    ((2.0, 8.0), (7.0, 3.0)),
                ] {
                    raya(&mut c, p(a.0, a.1), p(b2.0, b2.1), 1.4 * e, col);
                }
                let corte = [p(-10.0, 3.0), p(5.0, 3.0), p(10.0, -2.0), p(-5.0, -2.0)];
                c.triangulos(
                    &[corte[0], corte[1], corte[2], corte[0], corte[2], corte[3]],
                    (col & 0x00ff_ffff) | 0x7000_0000,
                    Some(CAPA_RAYAS),
                );
            }
            "anotar" => {
                // Un lapiz en diagonal, con su punta.
                let (ux, uy) = (
                    std::f64::consts::FRAC_1_SQRT_2,
                    -std::f64::consts::FRAC_1_SQRT_2,
                );
                let (nx, ny) = (-uy, ux);
                let q = |t: f64, s2: f64| {
                    [cx + ux * t * e + nx * s2 * e, cy + uy * t * e + ny * s2 * e]
                };
                let cuerpo = [q(-4.0, -2.6), q(-4.0, 2.6), q(9.0, 2.6), q(9.0, -2.6)];
                for k in 0..4 {
                    raya(&mut c, cuerpo[k], cuerpo[(k + 1) % 4], 1.5 * e, col);
                }
                raya(&mut c, q(-4.0, -2.6), q(-9.0, 0.0), 1.5 * e, col);
                raya(&mut c, q(-4.0, 2.6), q(-9.0, 0.0), 1.5 * e, col);
                raya(&mut c, q(6.0, -2.6), q(6.0, 2.6), 1.3 * e, col);
            }
            "marcos" => {
                // Un recuadro a trazos con las marcas de corte en las esquinas.
                let (x0, y0, x1, y1) = (cx - 8.0 * e, cy - 6.0 * e, cx + 8.0 * e, cy + 6.0 * e);
                for (a, b2) in [
                    ([x0, y0], [x1, y0]),
                    ([x1, y0], [x1, y1]),
                    ([x1, y1], [x0, y1]),
                    ([x0, y1], [x0, y0]),
                ] {
                    let l = (b2[0] - a[0]).hypot(b2[1] - a[1]);
                    let n = (l / (3.0 * e)).floor().max(1.0) as usize;
                    for k in (0..n).step_by(2) {
                        let t0 = k as f64 / n as f64;
                        let t1 = ((k + 1) as f64 / n as f64).min(1.0);
                        raya(
                            &mut c,
                            [a[0] + (b2[0] - a[0]) * t0, a[1] + (b2[1] - a[1]) * t0],
                            [a[0] + (b2[0] - a[0]) * t1, a[1] + (b2[1] - a[1]) * t1],
                            1.5 * e,
                            col,
                        );
                    }
                }
                for (px, py, dx, dy) in [
                    (x0, y0, -1.0, -1.0),
                    (x1, y0, 1.0, -1.0),
                    (x1, y1, 1.0, 1.0),
                    (x0, y1, -1.0, 1.0),
                ] {
                    raya(
                        &mut c,
                        [px + dx * 1.5 * e, py],
                        [px + dx * 4.0 * e, py],
                        1.4 * e,
                        col,
                    );
                    raya(
                        &mut c,
                        [px, py + dy * 1.5 * e],
                        [px, py + dy * 4.0 * e],
                        1.4 * e,
                        col,
                    );
                }
            }
            "capas" => {
                // Una lista: tres rayas con su casilla delante.
                for k in -1..=1 {
                    let y = cy + k as f64 * 5.5 * e;
                    c.triangulos(
                        &[
                            [cx - 9.0 * e, y - 1.8 * e],
                            [cx - 5.4 * e, y - 1.8 * e],
                            [cx - 5.4 * e, y + 1.8 * e],
                            [cx - 9.0 * e, y - 1.8 * e],
                            [cx - 5.4 * e, y + 1.8 * e],
                            [cx - 9.0 * e, y + 1.8 * e],
                        ],
                        col,
                        Some(CAPA_RAYAS),
                    );
                    raya(&mut c, [cx - 2.5 * e, y], [cx + 9.0 * e, y], 1.6 * e, col);
                }
            }
            "aristas" | "tres" => {
                // Un cubo de rayas.
                let p = |x: f64, y: f64| [cx + x * e, cy + y * e];
                for (a, b2) in [
                    ((-7.0, -3.0), (3.0, -3.0)),
                    ((3.0, -3.0), (3.0, 8.0)),
                    ((3.0, 8.0), (-7.0, 8.0)),
                    ((-7.0, 8.0), (-7.0, -3.0)),
                    ((-7.0, -3.0), (-3.0, -8.0)),
                    ((3.0, -3.0), (7.0, -8.0)),
                    ((-3.0, -8.0), (7.0, -8.0)),
                    ((7.0, -8.0), (7.0, 3.0)),
                    ((3.0, 8.0), (7.0, 3.0)),
                ] {
                    raya(&mut c, p(a.0, a.1), p(b2.0, b2.1), 1.6 * e, col);
                }
            }
            _ => {
                // Una regla en diagonal con sus marcas.
                let (a, bb) = ([cx - 9.0 * e, cy + 5.0 * e], [cx + 9.0 * e, cy - 5.0 * e]);
                let (dx, dy) = ((bb[0] - a[0]) / 1.0, (bb[1] - a[1]) / 1.0);
                let l = dx.hypot(dy);
                let (ux, uy) = (dx / l, dy / l);
                let (nx, ny) = (-uy, ux);
                let ancho = 4.5 * e;
                let p = |t: f64, s: f64| [a[0] + ux * t + nx * s, a[1] + uy * t + ny * s];
                for (t0, t1, s0, s1) in [
                    (0.0, l, -ancho, -ancho),
                    (0.0, l, ancho, ancho),
                    (0.0, 0.0, -ancho, ancho),
                    (l, l, -ancho, ancho),
                ] {
                    raya(&mut c, p(t0, s0), p(t1, s1), 1.5 * e, col);
                }
                for k in 1..5 {
                    let t = l * k as f64 / 5.0;
                    let largo = if k % 2 == 0 { ancho * 0.9 } else { ancho * 0.4 };
                    raya(&mut c, p(t, -ancho), p(t, -ancho + largo), 1.3 * e, col);
                }
            }
        } else if let Some(icono) = crate::iconos::de_boton(botones[i], claro, pantalla_completa) {
            // El icono de Material, de 20 px en el centro del boton, como en
            // la barra del lector.
            let lado = ICONO * e;
            let (ix, iy) = ((x0 + x1 - lado) / 2.0, (h - lado) / 2.0);
            crate::iconos::pintar(&mut c, icono, ix, iy, lado, col, CAPA_RAYAS);
        }
    }
    c.terminar()
}

// ------------------------------------------------------------------ el riel

/// **El riel de la derecha, como el de los lectores de PDF y Word**
/// (`pixpin_ui::riel_marcas`): una isla redondeada arriba a la derecha con
/// el lapiz (anotar) y el marco para imprimir, uno debajo de otro, botones
/// de 36 px y lo pulsado en el azul de la barra. Medidas, en pixeles
/// logicos: las del riel de los lectores.
const RIEL_LADO: f64 = 36.0;
const RIEL_BORDE: f64 = 16.0;
const RIEL_RELLENO: f64 = 4.0;

/// La isla del riel y la caja de cada boton (`[x0, y0, x1, y1]`, pixeles de
/// la ventana) en una ventana de `w` de ancho. Sin botones, no hay isla.
pub(crate) fn riel_cajas(w: u32, e: f64, n: usize) -> Option<([f64; 4], Vec<[f64; 4]>)> {
    if n == 0 {
        return None;
    }
    let (lado, relleno) = (RIEL_LADO * e, RIEL_RELLENO * e);
    let ancho = lado + 2.0 * relleno;
    let (x0, y0) = (w as f64 - RIEL_BORDE * e - ancho, RIEL_BORDE * e);
    let isla = [x0, y0, x0 + ancho, y0 + n as f64 * lado + 2.0 * relleno];
    let botones = (0..n)
        .map(|i| {
            let y = y0 + relleno + i as f64 * lado;
            [x0 + relleno, y, x0 + relleno + lado, y + lado]
        })
        .collect();
    Some((isla, botones))
}

/// El boton del riel bajo (x, y), o `Some(None)` si cae en la isla fuera de
/// todo boton (no es del plano), o `None` si cae fuera.
pub(crate) fn riel_en(w: u32, e: f64, n: usize, x: i32, y: i32) -> Option<Option<usize>> {
    let (isla, botones) = riel_cajas(w, e, n)?;
    let (x, y) = (x as f64, y as f64);
    let dentro = |c: &[f64; 4]| x >= c[0] && x < c[2] && y >= c[1] && y < c[3];
    dentro(&isla).then(|| botones.iter().position(dentro))
}

/// Pinta el riel: `botones` por nombre ("anotar", "marcos"), el que tiene
/// el raton encima y cuales estan pulsados. Siempre en los colores de los
/// lectores (el cristal oscuro), sea cual sea el fondo del plano.
pub(crate) fn riel(
    e: f64,
    w: u32,
    botones: &[&str],
    encima: Option<usize>,
    activos: &[bool],
) -> Modelo {
    let mut c = Constructor::nuevo();
    let Some((isla, cajas)) = riel_cajas(w, e, botones.len()) else {
        return c.terminar();
    };
    let cristal = (rgba(0x26, 0x26, 0x2c) & 0x00ff_ffff) | 0xEB00_0000;
    redondeado_en(
        &mut c,
        isla[0],
        isla[1],
        isla[2],
        isla[3],
        8.0 * e,
        cristal,
        CAPA_FONDO,
    );
    for (i, (b, k)) in botones.iter().zip(&cajas).enumerate() {
        let activo = activos.get(i).copied().unwrap_or(false);
        if activo {
            redondeado_en(
                &mut c,
                k[0],
                k[1],
                k[2],
                k[3],
                6.0 * e,
                rgba(0x00, 0x60, 0xDF),
                CAPA_RAYAS * 10.0,
            );
        } else if encima == Some(i) {
            redondeado_en(
                &mut c,
                k[0],
                k[1],
                k[2],
                k[3],
                6.0 * e,
                0x1FFF_FFFF,
                CAPA_RAYAS * 10.0,
            );
        }
        if let Some(icono) = crate::iconos::de_boton(b, false, false) {
            let lado = ICONO * e;
            let color = if activo {
                rgba(255, 255, 255)
            } else {
                rgba(0xe4, 0xe2, 0xe6)
            };
            crate::iconos::pintar(
                &mut c,
                icono,
                (k[0] + k[2] - lado) / 2.0,
                (k[1] + k[3] - lado) / 2.0,
                lado,
                color,
                CAPA_RAYAS,
            );
        }
    }
    c.terminar()
}

// ------------------------------------------------- los globos de los botones

/// **La tecla de cada boton** de la barra y del riel (19-oct-2026, el
/// usuario: «cuando pase por un icono, que me aparezca su atajo»). Las
/// mismas en el plano y en el modelo, y las de todos los visores: la I los
/// marcos, la A el lapiz, Ctrl+M el marcador.
pub(crate) fn tecla_de_boton(boton: &str) -> Option<&'static str> {
    Some(match boton {
        "marcador" => "Ctrl+M",
        "anotar" => "A",
        "marcos" => "I",
        "acotar" => "D",
        "tres" => "3",
        "letras" | "capas" => "L",
        "corte" => "C",
        "tema" => "B",
        "pantalla" => "F11",
        "fijar" => "T",
        "cerrar" => "Esc",
        _ => return None,
    })
}

/// Los nombres de los botones, para su globo (en el idioma de la app).
#[derive(Debug, Clone)]
pub struct NombresDeBotones {
    pub mover: String,
    pub marcador: String,
    pub anotar: String,
    pub marcos: String,
    pub acotar: String,
    pub tres: String,
    pub letras: String,
    pub corte: String,
    pub capas: String,
    pub tema: String,
    pub pantalla: String,
    pub fijar: String,
    pub cerrar: String,
}

impl Default for NombresDeBotones {
    fn default() -> Self {
        NombresDeBotones {
            mover: "Mover (arrastrar)".into(),
            marcador: "Marcador".into(),
            anotar: "Anotar".into(),
            marcos: "Marcos para imprimir".into(),
            acotar: "Acotar plano".into(),
            tres: "Ver en 3D".into(),
            letras: "Letras del plano".into(),
            corte: "Caja de sección".into(),
            capas: "Niveles y categorías".into(),
            tema: "Fondo claro u oscuro".into(),
            pantalla: "Pantalla completa".into(),
            fijar: "Siempre encima".into(),
            cerrar: "Cerrar".into(),
        }
    }
}

impl NombresDeBotones {
    pub fn ingles() -> Self {
        NombresDeBotones {
            mover: "Move (drag)".into(),
            marcador: "Bookmark".into(),
            anotar: "Annotate".into(),
            marcos: "Frames to print".into(),
            acotar: "Measure".into(),
            tres: "View in 3D".into(),
            letras: "Drawing fonts".into(),
            corte: "Section box".into(),
            capas: "Levels and categories".into(),
            tema: "Light or dark background".into(),
            pantalla: "Full screen".into(),
            fijar: "Always on top".into(),
            cerrar: "Close".into(),
        }
    }
    c.terminar()
}

/// Los marcos para imprimir, para pintarlos.
struct Marcos<'a> {
    hechos: &'a [[f64; 4]],
    nuevo: Option<([f64; 2], [f64; 2])>,
    marcando: bool,
}

/// Las cotas y lo demas que va encima del plano, en pixeles.
struct Acotar {
    activo: bool,
    puntos: Vec<[f64; 2]>,
    hechas: Vec<Vec<[f64; 2]>>,
    cursor: Option<[f64; 2]>,
    enganche: Option<[f64; 2]>,
    /// De que clase es el enganche (cambia la marca).
    tipo: crate::regla::Tipo,
}

/// Donde cae la mira: un punto, la perpendicular desde el ultimo punto, una
/// raya o un arco (como Android v0.113.0), o libre (`None`).
fn enganchar(
    g: Option<&(Enganches, crate::regla::Rayas)>,
    p: [f64; 2],
    radio: f64,
    desde: Option<[f64; 2]>,
) -> Option<([f64; 2], crate::regla::Tipo)> {
    let (puntos, rayas) = g?;
    let (q, t) = rayas.ajustar(p, radio, desde, Some(puntos));
    (t != crate::regla::Tipo::Libre).then_some((q, t))
}

#[allow(clippy::too_many_arguments)]
fn encima_del_plano(
    textos: &mut Textos,
    a: &Acotar,
    marcos: &Marcos,
    cam: &Camara,
    w: u32,
    h: u32,
    e: f64,
    unidades: u32,
    claro: bool,
    pista: &str,
) -> Modelo {
    let mut c = Constructor::nuevo();
    // Los marcos para imprimir: en azul, con su numero.
    {
        let azul = rgba(0x00, 0x78, 0xD4);
        let rect_px = |c: &mut Constructor, a: [f64; 2], b: [f64; 2], grosor: f64| {
            let (p, q) = (cam.pantalla(a, w, h), cam.pantalla(b, w, h));
            for (u, v) in [
                ([p[0], p[1]], [q[0], p[1]]),
                ([q[0], p[1]], [q[0], q[1]]),
                ([q[0], q[1]], [p[0], q[1]]),
                ([p[0], q[1]], [p[0], p[1]]),
            ] {
                raya_en(c, u, v, grosor, azul, CAPA_ARRIBA);
            }
        };
        for (i, m) in marcos.hechos.iter().enumerate() {
            rect_px(&mut c, [m[0], m[3]], [m[2], m[1]], 2.0 * e);
            let esquina = cam.pantalla([m[0], m[3]], w, h);
            let t = format!("{}", i + 1);
            let tam = 12.0 * e;
            let ancho = textos.medir_pantalla(&t, tam);
            rect_en(
                &mut c,
                esquina[0],
                esquina[1],
                esquina[0] + ancho + 12.0 * e,
                esquina[1] + 20.0 * e,
                azul,
                CAPA_ARRIBA * 0.5,
            );
            textos.en_pantalla(
                &mut c,
                &t,
                esquina[0] + 6.0 * e,
                esquina[1] + 15.0 * e,
                tam,
                rgba(255, 255, 255),
                esquina[0] + 80.0 * e,
            );
        }
        if let Some((a, b)) = marcos.nuevo {
            rect_px(&mut c, a, b, 1.5 * e);
        }
    }
    let naranja = rgba(0xFF, 0x8A, 0x00);
    let verde = rgba(0x30, 0xD1, 0x58);
    let (pildora, letra) = if claro {
        (0xEE1C1C1Eu32, rgba(255, 255, 255))
    } else {
        (0xEEF5F5F7u32, rgba(0x1C, 0x1C, 0x1E))
    };
    let a_px = |p: [f64; 2]| cam.pantalla(p, w, h);
    let tam = 12.5 * e;
    let mut etiqueta = |c: &mut Constructor, t: &str, x: f64, y: f64| {
        let ancho = textos.medir_pantalla(t, tam);
        let (x0, y0) = (x - ancho / 2.0 - 6.0 * e, y - tam * 0.95);
        rect(c, x0, y0, x0 + ancho + 12.0 * e, y0 + tam * 1.55, pildora);
        textos.en_pantalla(
            c,
            t,
            x - ancho / 2.0,
            y + tam * 0.25,
            tam,
            letra,
            x + ancho + 50.0,
        );
    };
    let mut cadenas: Vec<(&Vec<[f64; 2]>, bool)> = a.hechas.iter().map(|v| (v, false)).collect();
    let mut viva = a.puntos.clone();
    if a.activo
        && !a.puntos.is_empty()
        && let Some(cur) = a.enganche.or(a.cursor)
    {
        viva.push(cur);
    }
    cadenas.push((&viva, true));
    for (cad, es_viva) in cadenas {
        if cad.is_empty() {
            continue;
        }
        let mut total = 0.0;
        for par in cad.windows(2) {
            let (p, q) = (a_px(par[0]), a_px(par[1]));
            raya(&mut c, p, q, 2.0 * e, naranja);
            let d = (par[1][0] - par[0][0]).hypot(par[1][1] - par[0][1]);
            total += d;
            let (mx, my) = ((p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0);
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            let l = dx.hypot(dy).max(1e-9);
            let (nx, ny) = (-dy / l, dx / l);
            let lado = if ny > 0.0 { -1.0 } else { 1.0 };
            etiqueta(
                &mut c,
                &medida(d, unidades),
                mx + nx * lado * 14.0 * e,
                my + ny * lado * 14.0 * e,
            );
        }
        for p in cad.iter() {
            let q = a_px(*p);
            rect(
                &mut c,
                q[0] - 3.0 * e,
                q[1] - 3.0 * e,
                q[0] + 3.0 * e,
                q[1] + 3.0 * e,
                naranja,
            );
        }
        if cad.len() > 2 {
            let u = a_px(*cad.last().unwrap_or(&[0.0, 0.0]));
            etiqueta(
                &mut c,
                &format!("Σ {}", medida(total, unidades)),
                u[0] + 40.0 * e,
                u[1] + 22.0 * e,
            );
        }
        let _ = es_viva;
    }
    if a.activo || marcos.marcando {
        if let Some(g) = a.enganche.filter(|_| a.activo) {
            let q = a_px(g);
            let s = 6.0 * e;
            let lados: Vec<([f64; 2], [f64; 2])> = match a.tipo {
                // La perpendicular: ⊥ y su «90°».
                crate::regla::Tipo::Perpendicular => vec![([-s, s], [s, s]), ([0.0, s], [0.0, -s])],
                // En la raya: el reloj de arena de AutoCAD.
                crate::regla::Tipo::EnLaRaya => vec![
                    ([-s, -s], [s, -s]),
                    ([-s, s], [s, s]),
                    ([-s, -s], [s, s]),
                    ([s, -s], [-s, s]),
                ],
                _ => vec![
                    ([-s, -s], [s, -s]),
                    ([s, -s], [s, s]),
                    ([s, s], [-s, s]),
                    ([-s, s], [-s, -s]),
                ],
            };
            for (p0, p1) in lados {
                raya_en(
                    &mut c,
                    [q[0] + p0[0], q[1] + p0[1]],
                    [q[0] + p1[0], q[1] + p1[1]],
                    1.8 * e,
                    verde,
                    CAPA_ARRIBA,
                );
            }
            if a.tipo == crate::regla::Tipo::Perpendicular {
                textos.en_pantalla(
                    &mut c,
                    "90°",
                    q[0] + s + 4.0 * e,
                    q[1] - s - 2.0 * e,
                    tam,
                    verde,
                    q[0] + 200.0 * e,
                );
            }
        }
        // La pista, abajo a la izquierda.
        let ancho = textos.medir_pantalla(pista, tam);
        let (x0, y0) = (12.0 * e, h as f64 - 34.0 * e);
        rect(
            &mut c,
            x0,
            y0,
            x0 + ancho + 20.0 * e,
            y0 + 24.0 * e,
            pildora,
        );
        textos.en_pantalla(
            &mut c,
            pista,
            x0 + 10.0 * e,
            y0 + 16.5 * e,
            tam,
            letra,
            x0 + ancho + 40.0 * e,
        );
    }
    c.terminar()
}

/// Un aviso en una pastilla, abajo a la izquierda.
fn pastilla_abajo(textos: &mut Textos, t: &str, h: u32, e: f64, claro: bool) -> Modelo {
    let mut c = Constructor::nuevo();
    let (pildora, letra) = if claro {
        (0xEE1C1C1Eu32, rgba(255, 255, 255))
    } else {
        (0xEEF5F5F7u32, rgba(0x1C, 0x1C, 0x1E))
    };
    let tam = 12.5 * e;
    let ancho = textos.medir_pantalla(t, tam);
    let (x0, y0) = (12.0 * e, h as f64 - 34.0 * e);
    rect(
        &mut c,
        x0,
        y0,
        x0 + ancho + 20.0 * e,
        y0 + 24.0 * e,
        pildora,
    );
    textos.en_pantalla(
        &mut c,
        t,
        x0 + 10.0 * e,
        y0 + 16.5 * e,
        tam,
        letra,
        x0 + ancho + 40.0 * e,
    );
    c.terminar()
}

/// Un aviso en el centro (abriendo, error).
pub(crate) fn aviso(
    textos: &mut Textos,
    texto: &str,
    w: u32,
    h: u32,
    escala: f64,
    claro: bool,
) -> Modelo {
    let mut c = Constructor::nuevo();
    let tam = 15.0 * escala;
    let tinta = if claro {
        rgba(0x6b, 0x6a, 0x66)
    } else {
        rgba(0x9a, 0x98, 0x93)
    };
    let ancho = textos.medir_pantalla(texto, tam);
    textos.en_pantalla(
        &mut c,
        texto,
        (w as f64 - ancho) / 2.0,
        h as f64 / 2.0,
        tam,
        tinta,
        w as f64,
    );
    c.terminar()
}

/// La vista de un modelo dibujado en pixeles de la ventana (y hacia abajo).
pub(crate) fn vista_pantalla(m: &Modelo, w: u32, h: u32) -> Vista {
    Vista {
        escala: [2.0 / w as f32, -2.0 / h as f32],
        centro: [
            (w as f64 / 2.0 - m.origen[0]) as f32,
            (h as f64 / 2.0 - m.origen[1]) as f32,
        ],
        color7: [1.0; 4],
        px: 1.0,
        modo: 0.0,
        relleno: [0.0; 2],
    }
}

/// Donde va la barra: encima del plano si cabe, si no debajo, si no dentro.
pub fn colocar_barra(plano: RECT, trabajo: RECT, ancho: i32, alto: i32, sep: i32) -> (i32, i32) {
    let x = plano
        .left
        .clamp(trabajo.left, (trabajo.right - ancho).max(trabajo.left));
    let arriba = plano.top - sep - alto;
    if arriba >= trabajo.top {
        return (x, arriba);
    }
    let abajo = plano.bottom + sep;
    if abajo + alto <= trabajo.bottom {
        return (x, abajo);
    }
    (x + sep, plano.top.max(trabajo.top) + sep)
}

const M_TODO: u32 = 1;
const M_TEMA: u32 = 2;
const M_ACOTAR: u32 = 3;
const M_BORRAR: u32 = 4;
const M_ENCIMA: u32 = 5;
const M_CERRAR: u32 = 6;
const M_TRES: u32 = 7;
const M_MARCOS: u32 = 8;
const M_IMPRIMIR: u32 = 9;
const M_BORRAR_MARCOS: u32 = 10;
const M_ANOTAR: u32 = 11;
const M_LETRAS: u32 = 12;
const M_EXPORTAR: u32 = 13;
// Las del menu del boton «Aa».
const M_ANADIR_LETRAS: u32 = 40;
const M_QUITAR_LETRAS: u32 = 41;
const M_NADA: u32 = 49;

/// **El menu del boton «Aa»** (el dialogo «Letras del plano» de Android):
/// que letras pide el plano y no estan, donde suelen estar, añadir las
/// propias y quitarlas. Devuelve lo elegido.
fn menu_letras(hwnd: HWND, t: &TextosUi, faltan: &[String], mias: usize) -> Option<u32> {
    // SAFETY: menu propio que se destruye antes de salir; cadenas vivas
    // durante cada AppendMenuW (las copia el sistema).
    unsafe {
        let m = CreatePopupMenu().ok()?;
        let poner = |id: u32, texto: &str, gris: bool| {
            let v: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
            let f = if gris {
                MF_STRING | MF_GRAYED
            } else {
                MF_STRING
            };
            let _ = AppendMenuW(m, f, id as usize, PCWSTR(v.as_ptr()));
        };
        if faltan.is_empty() {
            poner(M_NADA, &t.letras_no_falta, true);
        } else {
            poner(M_NADA, &t.letras_faltan, true);
            for f in faltan {
                poner(M_NADA, &format!("      {f}"), true);
            }
        }
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        for l in t.letras_donde.lines() {
            poner(M_NADA, l, true);
        }
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        poner(M_ANADIR_LETRAS, &t.letras_anadir, false);
        poner(
            M_QUITAR_LETRAS,
            &t.letras_quitar.replace("{n}", &mias.to_string()),
            mias == 0,
        );
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let _ = SetForegroundWindow(hwnd);
        let r = TrackPopupMenu(
            m,
            TPM_LEFTALIGN | TPM_RETURNCMD,
            p.x,
            p.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(m);
        (r.0 != 0).then_some(r.0 as u32)
    }
}

/// Las letras del usuario, para el boton «Aa»: cuantas hay, añadir (con el
/// dialogo de Windows), quitarlas y volver a leer el plano con ellas.
pub struct Letras {
    pub mias: Box<dyn Fn() -> usize>,
    /// Pide las letras con el dialogo «Abrir» (sobre la ventana dada) y las
    /// copia; devuelve cuantas.
    pub anadir: Box<dyn Fn(isize) -> usize>,
    pub quitar: Box<dyn Fn() -> usize>,
    /// Lee el plano otra vez (con la huella nueva de las letras).
    #[allow(clippy::type_complexity)]
    pub releer: Box<dyn Fn() -> Receiver<Result<Modelo, String>>>,
    /// La huella de las letras ahora ([`crate::letras::huella`]): si cambia
    /// con el plano abierto (llegaron de otro aparato), se relee.
    pub huella: Box<dyn Fn() -> String>,
}

/// Cada cuanto se mira si cambiaron las letras con el plano abierto.
const MIRAR_LETRAS_CADA_MS: u32 = 2000;

#[allow(clippy::too_many_arguments)]
fn menu(
    hwnd: HWND,
    t: &TextosUi,
    acotando: bool,
    hay_cotas: bool,
    fijada: bool,
    marcando: bool,
    hay_marcos: bool,
) {
    // SAFETY: menu propio que se destruye antes de salir; cadenas vivas.
    unsafe {
        let Ok(m) = CreatePopupMenu() else { return };
        let poner = |id: u32, texto: &str, marca: bool, gris: bool| {
            let v: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
            let mut f = MF_STRING;
            if marca {
                f |= MF_CHECKED;
            }
            if gris {
                f |= MF_GRAYED;
            }
            let _ = AppendMenuW(m, f, id as usize, PCWSTR(v.as_ptr()));
        };
        poner(M_TODO, &t.todo, false, false);
        poner(M_TEMA, &t.tema, false, false);
        poner(M_ACOTAR, &t.acotar, acotando, false);
        poner(M_BORRAR, &t.borrar_cotas, false, !hay_cotas);
        poner(M_TRES, &t.tres, false, false);
        poner(M_LETRAS, &t.letras, false, false);
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        poner(M_ANOTAR, &t.anotar, false, false);
        poner(M_MARCOS, &t.marcos, marcando, false);
        poner(M_IMPRIMIR, &t.imprimir, false, false);
        poner(M_EXPORTAR, &t.exportar, false, false);
        poner(M_BORRAR_MARCOS, &t.borrar_marcos, false, !hay_marcos);
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        poner(M_ENCIMA, &t.encima, fijada, false);
        poner(M_CERRAR, &t.cerrar, false, false);
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let _ = TrackPopupMenu(m, TPM_RIGHTBUTTON, p.x, p.y, Some(0), hwnd, None);
        let _ = DestroyMenu(m);
    }
}

/// **Abre la ventana del plano** y no vuelve hasta que se cierra. `cargando`
/// trae el plano cuando esta listo (o el porque no).
/// Lo que el visor no sabe hacer solo y le da quien lo abre.
#[derive(Default)]
pub struct Acciones {
    /// El boton «3D»: abrir el mismo plano en 3D.
    pub abrir_3d: Option<Box<dyn Fn()>>,
    /// Imprimir: la ventana del visor, el plano, los marcos (`[x0, y0, x1,
    /// y1]` del plano; vacio: el plano entero) y las cotas puestas.
    #[allow(clippy::type_complexity)]
    pub imprimir: Option<Box<dyn Fn(isize, &Modelo, &[[f64; 4]], &[Vec<[f64; 2]>])>>,
    /// Anotar encima: el motor del lienzo (ver [`crate::anotado`]). Se le
    /// da el plano cuando llega, para leer su capa.
    #[allow(clippy::type_complexity)]
    pub anotador: Option<Box<dyn FnOnce(&Modelo) -> Box<dyn crate::anotado::Anotador>>>,
    /// El boton «Aa»: las letras del usuario (ver [`crate::letras`]).
    pub letras: Option<Letras>,
    /// Exportar (HTML, PDF, PNG, DXF...): lo mismo que imprimir recibe.
    #[allow(clippy::type_complexity)]
    pub exportar: Option<Box<dyn Fn(isize, &Modelo, &[[f64; 4]], &[Vec<[f64; 2]>])>>,
    /// Donde mirar al abrir, en vez del plano entero: `[x, y, ancho]` en
    /// coordenadas del dibujo (el centro y lo que cabe a lo ancho). Lo pide
    /// el asistente al llevar a un texto encontrado (`abrir` con `zona`).
    pub vista_inicial: Option<[f64; 3]>,
}

/// La firma de las cotas y los marcos (para saber si cambiaron y guardarlos
/// en la capa del motor del lienzo, ver [`crate::anotado`]).
fn firma_de(cotas: &[Vec<[f64; 2]>], marcos: &[[f64; 4]]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for c in cotas {
        c.len().hash(&mut h);
        c.iter().flatten().for_each(|v| v.to_bits().hash(&mut h));
    }
    marcos.len().hash(&mut h);
    marcos
        .iter()
        .flatten()
        .for_each(|v| v.to_bits().hash(&mut h));
    h.finish()
}

/// Lo que ve la ventana, para el motor del lienzo.
fn vista_para_el_motor(
    hwnd: HWND,
    camara: &Camara,
    w: u32,
    h: u32,
    e: f64,
    m: &Modelo,
    claro: bool,
) -> crate::anotado::VistaPlano {
    let mut p = POINT::default();
    // SAFETY: ventana propia; punto local.
    let _ = unsafe { windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut p) };
    crate::anotado::VistaPlano {
        centro: camara.centro,
        px: camara.px,
        ancho: w,
        alto: h,
        origen_pantalla: (p.x, p.y),
        escala_por_cien: (e * 100.0).round() as u32,
        unidad: crate::anotado::unidad_de_la_capa(m),
        claro,
    }
}

pub fn ver(
    titulo: &str,
    cargando: Receiver<Result<Modelo, String>>,
    textos_ui: TextosUi,
    mut acciones: Acciones,
) -> Result<(), String> {
    let mut cargando = cargando;
    // Releyendo el plano con letras nuevas: el de antes sigue a la vista.
    let mut releyendo = false;
    // Las letras que el plano pide y no estan (el «Aa» en ambar) y hasta
    // cuando se dice abajo, como el aviso de Android al abrir.
    let mut faltan_letras: Vec<String> = Vec::new();
    let mut aviso_letras_hasta: Option<Instant> = None;
    let abrir_3d = &acciones.abrir_3d;
    // El motor del lienzo para anotar: nace cuando llega el plano.
    let mut hacer_anotador = acciones.anotador.take();
    let vista_inicial = acciones.vista_inicial.take();
    let mut anotador: Option<Box<dyn crate::anotado::Anotador>> = None;
    let mut anotando = false;
    let (hwnd, mut w, mut h) = crear_ventana(w!("PixPinPlanoCad"), &format!("{titulo} — PixPin"))
        .map_err(|e| e.to_string())?;
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
    let (bw, bh) = (
        ancho_barra(e, BOTONES.len()).ceil() as i32,
        (ALTO_BARRA * e).ceil() as i32,
    );
    let hbarra = crear_barra(hwnd, bw, bh).map_err(|e| e.to_string())?;
    let mut gpu_barra = Gpu::nueva(hbarra, bw as u32, bh as u32).map_err(|e| e.to_string())?;
    ESTADO.with(|s| s.borrow_mut().barra = Some((hbarra, (8.0 * e) as i32)));
    let mut textos = Textos::nuevo();
    let mut plano: Option<(Modelo, PlanoGpu)> = None;
    let mut enganches: Option<(Enganches, crate::regla::Rayas)> = None;
    let mut mensaje = textos_ui.abriendo.clone();
    let mut camara = Camara {
        centro: [0.0, 0.0],
        px: 1.0,
    };
    let mut destino = camara;
    let mut claro = false;
    let mut fijada = crate::ventana::FIJADA_AL_NACER;
    let mut dentro_plano = false;
    let mut dentro_barra = false;
    let mut fuera_desde: Option<Instant> = None;
    let mut barra_visible = false;
    let mut arrastre: Option<(i32, i32, Camara, bool)> = None;
    let mut boton_encima: Option<usize> = None;
    let mut acotar = Acotar {
        activo: false,
        puntos: Vec::new(),
        hechas: Vec::new(),
        cursor: None,
        enganche: None,
        tipo: crate::regla::Tipo::Libre,
    };
    let mut raton = (0i32, 0i32);
    let mut sucio = true;
    let mut barra_sucia = true;
    let mut aviso_ui: Option<(Modelo, PlanoGpu)> = None;
    // Objetos de Civil 3D guardados sin su dibujo: cuantos, y hasta cuando se avisa.
    let mut sin_dibujo_civil = 0u32;
    // Los marcos para imprimir (como Android v0.111): si se estan marcando y
    // el que se arrastra ahora (dos esquinas del plano).
    let mut marcos: Vec<[f64; 4]> = Vec::new();
    let mut marcando = false;
    let mut nuevo_marco: Option<([f64; 2], [f64; 2])> = None;
    // La firma de las cotas y los marcos ya guardados en la capa: si cambia,
    // se guardan (viajan con ella, como en Android).
    let mut firma_guardada = firma_de(&acotar.hechas, &marcos);
    let mut aviso_civil_hasta: Option<Instant> = None;
    // Las letras con que se leyo el plano (su huella) y cuando se miraron:
    // si cambian sin el boton «Aa» (llegaron de otro aparato propio), el
    // plano se vuelve a leer sin mover la vista.
    let mut huella_leida: Option<String> = None;
    let mut letras_miradas = Instant::now();
    'bucle: loop {
        let animando = camara != destino;
        // SAFETY: bucle de mensajes de este hilo.
        unsafe {
            if !animando && !sucio && !barra_sucia {
                let espera =
                    if plano.is_none() || releyendo || barra_visible || fuera_desde.is_some() {
                        50
                    } else if huella_leida.is_some() {
                        MIRAR_LETRAS_CADA_MS
                    } else {
                        INFINITE
                    };
                let _ = MsgWaitForMultipleObjectsEx(None, espera, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        // El plano, cuando llega.
        if (plano.is_none() && mensaje == textos_ui.abriendo) || releyendo {
            let otra_vez = releyendo;
            let llegado = cargando.try_recv();
            if !matches!(llegado, Err(std::sync::mpsc::TryRecvError::Empty)) {
                releyendo = false;
            }
            match llegado {
                Ok(Ok(m)) => {
                    if m.vacio() {
                        mensaje = textos_ui.vacio.clone();
                    } else {
                        match gpu.subir(&m) {
                            Ok(p) => {
                                // Lo que importa, no la caja entera: un objeto suelto a
                                // kilometros dejaba el plano diminuto, como si no abriera.
                                // Releido con otras letras, se queda donde se miraba.
                                if !otra_vez {
                                    camara = Camara::encuadrar(m.caja_util(), w, h);
                                }
                                enganches = None;
                                faltan_letras = crate::letras::las_que_faltan(&m.sin_dibujar);
                                aviso_letras_hasta = (!faltan_letras.is_empty())
                                    .then(|| Instant::now() + Duration::from_secs(8));
                                if huella_leida.is_none() {
                                    huella_leida = acciones.letras.as_ref().map(|l| (l.huella)());
                                    letras_miradas = Instant::now();
                                }
                                barra_sucia = true;
                                // Para las pruebas: PIXPIN_CAD_VISTA="x,y,ancho" (coordenadas del plano).
                                if let Ok(v) = std::env::var("PIXPIN_CAD_VISTA")
                                    && !otra_vez
                                {
                                    let n: Vec<f64> = v
                                        .split(',')
                                        .filter_map(|t| t.trim().parse().ok())
                                        .collect();
                                    if n.len() == 3 && n[2] > 0.0 {
                                        camara = Camara {
                                            centro: [n[0] - m.origen[0], n[1] - m.origen[1]],
                                            px: n[2] / w.max(1) as f64,
                                        };
                                    }
                                }
                                // Lo pedido al abrir (el asistente llevando a un texto).
                                if let Some([x, y, ancho]) = vista_inicial
                                    && !otra_vez
                                    && ancho > 0.0
                                    && x.is_finite()
                                    && y.is_finite()
                                {
                                    camara = Camara {
                                        centro: [x - m.origen[0], y - m.origen[1]],
                                        px: ancho / w.max(1) as f64,
                                    };
                                }
                                destino = camara;
                                sin_dibujo_civil = m
                                    .sin_dibujar
                                    .iter()
                                    .filter(|(t, _)| {
                                        t.starts_with(crate::convertir::PROXY_SIN_DIBUJO)
                                    })
                                    .map(|(_, n)| n)
                                    .sum();
                                if sin_dibujo_civil > 0 {
                                    aviso_civil_hasta =
                                        Some(Instant::now() + Duration::from_secs(12));
                                }
                                if let Some(f) = hacer_anotador.take() {
                                    let a = f(&m);
                                    // Las cotas y los marcos guardados en la capa (de aqui o del movil).
                                    (acotar.hechas, marcos) = a.cotas_y_marcos();
                                    firma_guardada = firma_de(&acotar.hechas, &marcos);
                                    anotador = Some(a);
                                }
                                plano = Some((m, p));
                                mensaje.clear();
                            }
                            Err(err) => mensaje = format!("{}: {err}", textos_ui.error),
                        }
                    }
                    aviso_ui = None;
                    sucio = true;
                }
                Ok(Err(err)) => {
                    mensaje = format!("{}: {err}", textos_ui.error);
                    aviso_ui = None;
                    sucio = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    mensaje = textos_ui.error.clone();
                    aviso_ui = None;
                    sucio = true;
                }
            }
        }
        let eventos: Vec<(De, Ev)> = ESTADO.with(|s| s.borrow_mut().eventos.drain(..).collect());
        let b_barra = botones_barra(e, BOTONES.len());
        let boton_en = |x: i32| -> Option<usize> {
            b_barra
                .iter()
                .position(|(a, b)| (x as f64) >= *a && (x as f64) < *b)
        };
        let mut cambiar_tema = false;
        let mut cambiar_acotar = false;
        let mut cambiar_encima = false;
        let mut borrar_cotas = false;
        let mut encuadrar = false;
        let mut ver_en_3d = false;
        let mut cambiar_marcos = false;
        let mut cambiar_anotar = false;
        let mut imprimir = false;
        let mut exportar = false;
        let mut ver_letras = false;
        for (de, ev) in eventos {
            // **Anotando, el raton y el teclado son del motor del lienzo**
            // (la rueda y el boton central siguen moviendo el plano).
            if anotando
                && de == De::Plano
                && let (Some(a), Some((m, _))) = (anotador.as_mut(), &plano)
            {
                let v = vista_para_el_motor(hwnd, &camara, w, h, e, m, claro);
                let (ox, oy) = v.origen_pantalla;
                // SAFETY: lee el estado de las teclas.
                let tecla = |vk: i32| unsafe { GetKeyState(vk) } < 0;
                let ev_motor = match ev {
                    Ev::Mover(x, y) => Some(crate::anotado::EventoPlano::Mover(x + ox, y + oy)),
                    Ev::Bajar(x, y, 0) | Ev::Doble(x, y) => {
                        Some(crate::anotado::EventoPlano::Bajar(x + ox, y + oy))
                    }
                    Ev::Subir(x, y, 0) => Some(crate::anotado::EventoPlano::Subir(x + ox, y + oy)),
                    Ev::Tecla(0x1B) if !a.quiere_escape() => None,
                    Ev::Tecla(vk) => Some(crate::anotado::EventoPlano::Tecla {
                        vk,
                        shift: tecla(0x10),
                        ctrl: tecla(0x11),
                        alt: tecla(0x12),
                    }),
                    Ev::TeclaSoltada(vk) => Some(crate::anotado::EventoPlano::TeclaSoltada(vk)),
                    Ev::Caracter(c) => char::from_u32(c).map(crate::anotado::EventoPlano::Caracter),
                    _ => None,
                };
                if let Some(evm) = ev_motor {
                    let r = a.evento(evm, &v);
                    if r.arrastra {
                        // SAFETY: ventana propia.
                        unsafe { SetCapture(hwnd) };
                    }
                    if r.salir {
                        cambiar_anotar = true;
                    }
                    sucio = true;
                    if matches!(ev, Ev::Mover(..)) {
                        dentro_plano = true;
                    }
                    if r.consumido
                        || matches!(ev, Ev::Tecla(_) | Ev::TeclaSoltada(_) | Ev::Caracter(_))
                    {
                        continue;
                    }
                }
                if matches!(ev, Ev::Tecla(0x1B)) {
                    cambiar_anotar = true;
                    continue;
                }
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
                        // El asa: Windows arrastra la ventana del plano.
                        // SAFETY: ventanas propias.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(
                                hwnd,
                                WM_NCLBUTTONDOWN,
                                Some(WPARAM(HTCAPTION as usize)),
                                Some(LPARAM(0)),
                            );
                        }
                    }
                    Some(1) => cambiar_tema = true,
                    Some(2) => cambiar_acotar = true,
                    Some(3) => cambiar_marcos = true,
                    Some(4) => cambiar_anotar = true,
                    Some(5) => ver_en_3d = true,
                    Some(6) => ver_letras = true,
                    Some(7) => cambiar_encima = true,
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
                    }
                }
                (De::Plano, Ev::Mover(x, y)) => {
                    raton = (x, y);
                    dentro_plano = true;
                    if let Some((a, _)) = nuevo_marco {
                        nuevo_marco = Some((a, camara.plano(x as f64, y as f64, w, h)));
                        sucio = true;
                        continue;
                    }
                    if let Some((x0, y0, c0, ventana)) = arrastre {
                        if ventana {
                            continue;
                        }
                        if (x - x0).abs() + (y - y0).abs() > 3 || !acotar.activo {
                            let c = Camara {
                                centro: [
                                    c0.centro[0] - (x - x0) as f64 * c0.px,
                                    c0.centro[1] + (y - y0) as f64 * c0.px,
                                ],
                                px: c0.px,
                            };
                            camara = c;
                            destino = c;
                            sucio = true;
                        }
                    }
                    if acotar.activo {
                        let p = camara.plano(x as f64, y as f64, w, h);
                        acotar.cursor = Some(p);
                        let g = enganchar(
                            enganches.as_ref(),
                            p,
                            12.0 * e * camara.px,
                            acotar.puntos.last().copied(),
                        );
                        acotar.enganche = g.map(|(q, _)| q);
                        acotar.tipo = g.map_or(crate::regla::Tipo::Libre, |(_, t)| t);
                        sucio = true;
                    }
                }
                (De::Plano, Ev::Salir) => {
                    dentro_plano = false;
                    if acotar.activo {
                        acotar.cursor = None;
                        acotar.enganche = None;
                        sucio = true;
                    }
                }
                (De::Plano, Ev::Bajar(x, y, b)) => {
                    // SAFETY: lee el estado de la tecla Ctrl.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    if b == 0 && ctrl {
                        // Ctrl + arrastrar: mover la ventana.
                        // SAFETY: ventana propia.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(
                                hwnd,
                                WM_NCLBUTTONDOWN,
                                Some(WPARAM(HTCAPTION as usize)),
                                Some(LPARAM(0)),
                            );
                        }
                    } else if b == 0 && marcando && plano.is_some() {
                        let p = camara.plano(x as f64, y as f64, w, h);
                        nuevo_marco = Some((p, p));
                    } else if b <= 1 {
                        arrastre = Some((x, y, camara, false));
                    } else {
                        let hay_cotas = !acotar.hechas.is_empty() || !acotar.puntos.is_empty();
                        menu(
                            hwnd,
                            &textos_ui,
                            acotar.activo,
                            hay_cotas,
                            fijada,
                            marcando,
                            !marcos.is_empty(),
                        );
                    }
                }
                (De::Plano, Ev::Subir(x, y, b)) => {
                    if let Some((a, _)) = nuevo_marco.take() {
                        let q = camara.plano(x as f64, y as f64, w, h);
                        // Un marco de menos de 8 pixeles es un clic, no un marco.
                        if (q[0] - a[0]).abs() > 8.0 * camara.px
                            && (q[1] - a[1]).abs() > 8.0 * camara.px
                        {
                            marcos.push([
                                a[0].min(q[0]),
                                a[1].min(q[1]),
                                a[0].max(q[0]),
                                a[1].max(q[1]),
                            ]);
                        }
                        sucio = true;
                        continue;
                    }
                    let clic =
                        arrastre.is_some_and(|(x0, y0, _, _)| (x - x0).abs() + (y - y0).abs() <= 3);
                    arrastre = None;
                    if b == 0 && clic && acotar.activo && plano.is_some() {
                        let p = camara.plano(x as f64, y as f64, w, h);
                        let p = enganchar(
                            enganches.as_ref(),
                            p,
                            12.0 * e * camara.px,
                            acotar.puntos.last().copied(),
                        )
                        .map_or(p, |(q, _)| q);
                        if acotar.puntos.last() != Some(&p) {
                            acotar.puntos.push(p);
                        }
                        sucio = true;
                    }
                }
                (De::Plano, Ev::Doble(..)) => {
                    if acotar.activo {
                        // El doble clic deja la medida (y su segundo clic ya
                        // puso el ultimo punto).
                        if acotar.puntos.len() >= 2 {
                            acotar.hechas.push(std::mem::take(&mut acotar.puntos));
                        }
                        acotar.puntos.clear();
                        sucio = true;
                    } else {
                        encuadrar = true;
                    }
                }
                (De::Plano, Ev::Rueda(x, y, d)) => {
                    let factor = 1.25f64.powf(-(d as f64) / 120.0);
                    destino = destino.zoom(factor, x as f64, y as f64, w, h);
                }
                (De::Plano, Ev::Menu(id)) => match id {
                    M_TODO => encuadrar = true,
                    M_TEMA => cambiar_tema = true,
                    M_ACOTAR => cambiar_acotar = true,
                    M_BORRAR => borrar_cotas = true,
                    M_ENCIMA => cambiar_encima = true,
                    M_TRES => ver_en_3d = true,
                    M_MARCOS => cambiar_marcos = true,
                    M_ANOTAR => cambiar_anotar = true,
                    M_LETRAS => ver_letras = true,
                    M_IMPRIMIR => imprimir = true,
                    M_EXPORTAR => exportar = true,
                    M_BORRAR_MARCOS => {
                        marcos.clear();
                        sucio = true;
                    }
                    M_CERRAR => break 'bucle,
                    _ => {}
                },
                (De::Plano, Ev::TeclaSoltada(_) | Ev::Caracter(_)) => {}
                (De::Plano, Ev::Tecla(vk)) => match vk {
                    0x1B => {
                        if !acotar.puntos.is_empty() {
                            acotar.puntos.clear();
                            sucio = true;
                        } else if acotar.activo {
                            cambiar_acotar = true;
                        } else {
                            break 'bucle;
                        }
                    }
                    0x0D => {
                        if acotar.puntos.len() >= 2 {
                            acotar.hechas.push(std::mem::take(&mut acotar.puntos));
                            sucio = true;
                        }
                    }
                    0x08 => {
                        acotar.puntos.pop();
                        sucio = true;
                    }
                    // Supr: marcando, el ultimo marco; si no, las cotas.
                    0x2E if marcando => {
                        marcos.pop();
                        sucio = true;
                    }
                    0x2E => borrar_cotas = true,
                    // Ctrl+P imprimir, como en cualquier programa; I, los marcos.
                    // SAFETY: lee el estado de la tecla Ctrl.
                    0x50 if unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0 => imprimir = true,
                    // Ctrl+E: exportar.
                    // SAFETY: lee el estado de la tecla Ctrl.
                    0x45 if unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0 => exportar = true,
                    0x49 => cambiar_marcos = true,
                    0x41 => cambiar_anotar = true,
                    0x4C => ver_letras = true,
                    0x46 | 0x24 => encuadrar = true,
                    0x42 => cambiar_tema = true,
                    0x4D => cambiar_acotar = true,
                    0x54 => cambiar_encima = true,
                    0x33 | 0x63 => ver_en_3d = true,
                    0xBB | 0x6B => {
                        destino = destino.zoom(1.0 / 1.4, w as f64 / 2.0, h as f64 / 2.0, w, h)
                    }
                    0xBD | 0x6D => {
                        destino = destino.zoom(1.4, w as f64 / 2.0, h as f64 / 2.0, w, h)
                    }
                    0x25 => destino.centro[0] -= w as f64 * 0.15 * destino.px,
                    0x27 => destino.centro[0] += w as f64 * 0.15 * destino.px,
                    0x26 => destino.centro[1] += h as f64 * 0.15 * destino.px,
                    0x28 => destino.centro[1] -= h as f64 * 0.15 * destino.px,
                    _ => {}
                },
            }
        }
        if encuadrar && let Some((m, _)) = &plano {
            destino = Camara::encuadrar(m.caja_util(), w, h);
        }
        if cambiar_tema {
            claro = !claro;
            barra_sucia = true;
            aviso_ui = None;
            sucio = true;
        }
        // Medir, marcar y anotar no van a la vez: el arrastre seria de todos.
        if ((cambiar_marcos && !marcando) || (cambiar_acotar && !acotar.activo) || imprimir)
            && anotando
        {
            cambiar_anotar = true;
        }
        if cambiar_anotar && anotador.is_some() {
            anotando = !anotando;
            if anotando {
                marcando = false;
                if acotar.activo {
                    cambiar_acotar = true;
                }
            } else if let Some(a) = anotador.as_mut() {
                // Lo que se escribia se queda escrito y la capa se guarda.
                a.terminar();
                // Anotando se pudo borrar o mover una cota o un marco con el motor.
                (acotar.hechas, marcos) = a.cotas_y_marcos();
                firma_guardada = firma_de(&acotar.hechas, &marcos);
            }
            barra_sucia = true;
            sucio = true;
        }
        if cambiar_marcos {
            marcando = !marcando;
            nuevo_marco = None;
            if marcando && acotar.activo {
                cambiar_acotar = true;
            }
            barra_sucia = true;
            sucio = true;
        }
        if cambiar_acotar && !acotar.activo && marcando {
            marcando = false;
            barra_sucia = true;
        }
        if exportar && let (Some(f), Some((m, _))) = (&acciones.exportar, &plano) {
            let mut cotas = acotar.hechas.clone();
            if acotar.puntos.len() >= 2 {
                cotas.push(acotar.puntos.clone());
            }
            f(hwnd.0 as isize, m, &marcos, &cotas);
        }
        if imprimir && let (Some(f), Some((m, _))) = (&acciones.imprimir, &plano) {
            let mut cotas = acotar.hechas.clone();
            if acotar.puntos.len() >= 2 {
                cotas.push(acotar.puntos.clone());
            }
            f(hwnd.0 as isize, m, &marcos, &cotas);
        }
        if cambiar_acotar {
            acotar.activo = !acotar.activo;
            acotar.puntos.clear();
            if acotar.activo
                && enganches.is_none()
                && let Some((m, _)) = &plano
            {
                enganches = Some((Enganches::de(m), crate::regla::Rayas::de(m)));
            }
            if acotar.activo {
                acotar.cursor = Some(camara.plano(raton.0 as f64, raton.1 as f64, w, h));
            }
            // SAFETY: cursor del sistema.
            unsafe {
                let _ = SetCursor(
                    LoadCursorW(None, if acotar.activo { IDC_CROSS } else { IDC_ARROW }).ok(),
                );
            }
            barra_sucia = true;
            sucio = true;
        }
        if borrar_cotas {
            acotar.hechas.clear();
            acotar.puntos.clear();
            sucio = true;
        }
        // Una cota o un marco puesto o quitado: a la capa, que viaja.
        let firma = firma_de(&acotar.hechas, &marcos);
        if firma != firma_guardada
            && let Some(a) = anotador.as_mut()
        {
            a.poner_cotas_y_marcos(&acotar.hechas, &marcos);
            firma_guardada = firma;
        }
        if cambiar_encima {
            fijada = !fijada;
            poner_encima(hwnd, fijada);
            barra_sucia = true;
        }
        if ver_en_3d
            && plano.is_some()
            && let Some(f) = abrir_3d
        {
            f();
        }
        // El boton «Aa»: las letras que faltan, añadir las propias o quitarlas.
        // Con letras nuevas, el plano se vuelve a leer (su cache lleva la huella).
        if ver_letras
            && plano.is_some()
            && !releyendo
            && let Some(l) = &acciones.letras
        {
            let cambiadas = match menu_letras(hwnd, &textos_ui, &faltan_letras, (l.mias)()) {
                Some(M_ANADIR_LETRAS) => (l.anadir)(hwnd.0 as isize),
                Some(M_QUITAR_LETRAS) => (l.quitar)(),
                _ => 0,
            };
            if cambiadas > 0 {
                cargando = (l.releer)();
                releyendo = true;
                huella_leida = Some((l.huella)());
                mensaje = textos_ui.releyendo.clone();
                aviso_ui = None;
                aviso_letras_hasta = None;
                sucio = true;
            }
        }
        // **Letras que llegaron de otro aparato propio** (la sincronizacion
        // las deja en la misma carpeta): mirar de vez en cuando su huella
        // (leer una carpeta pequeña) y, si cambio, releer como con «Aa».
        if plano.is_some()
            && !releyendo
            && letras_miradas.elapsed() >= Duration::from_millis(MIRAR_LETRAS_CADA_MS as u64)
            && let (Some(l), Some(antes)) = (&acciones.letras, &huella_leida)
        {
            letras_miradas = Instant::now();
            let ahora = (l.huella)();
            if ahora != *antes {
                cargando = (l.releer)();
                releyendo = true;
                huella_leida = Some(ahora);
                mensaje = textos_ui.releyendo.clone();
                aviso_ui = None;
                aviso_letras_hasta = None;
                sucio = true;
            }
        }
        if aviso_letras_hasta.is_some_and(|t| Instant::now() > t) {
            aviso_letras_hasta = None;
            sucio = true;
        }
        if aviso_civil_hasta.is_some_and(|t| Instant::now() > t) {
            aviso_civil_hasta = None;
            sucio = true;
        }
        if acotar.activo {
            // SAFETY: cursor del sistema, sobre la ventana propia.
            unsafe {
                if dentro_plano {
                    let _ = SetCursor(LoadCursorW(None, IDC_CROSS).ok());
                }
            }
        }
        // La barra: sale con el raton encima del plano o de ella, y se va
        // un poco despues de salir de los dos.
        let quiere = dentro_plano || dentro_barra || arrastre.is_some();
        if quiere {
            fuera_desde = None;
        } else if barra_visible && fuera_desde.is_none() {
            fuera_desde = Some(Instant::now());
        }
        let mostrar =
            quiere || fuera_desde.is_some_and(|t| t.elapsed() < Duration::from_millis(600));
        if !mostrar {
            fuera_desde = None;
        }
        if mostrar && plano.is_some() {
            if barra_visible {
                seguir_barra(hwnd);
            } else {
                // Al aparecer, una vez, delante; luego solo se mueve: subirla
                // en cada vuelta peleaba con el recorte de las capturas.
                let mut r = RECT::default();
                // SAFETY: ventanas propias; estructuras locales.
                unsafe {
                    let _ = GetWindowRect(hwnd, &mut r);
                    let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                    let mut info = MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    let _ = GetMonitorInfoW(mon, &mut info);
                    let (x, y) = colocar_barra(r, info.rcWork, bw, bh, (8.0 * e) as i32);
                    let _ = SetWindowPos(
                        hbarra,
                        Some(if fijada { HWND_TOPMOST } else { HWND_NOTOPMOST }),
                        x,
                        y,
                        bw,
                        bh,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
                barra_visible = true;
                barra_sucia = true;
            }
        } else if barra_visible {
            barra_visible = false;
            // SAFETY: ventana propia.
            let _ = unsafe { ShowWindow(hbarra, SW_HIDE) };
        }
        // La camara va hacia su destino: suave, sin pasarse.
        if camara != destino {
            let k = 0.38;
            let px = (camara.px.ln() + (destino.px.ln() - camara.px.ln()) * k).exp();
            let centro = [
                camara.centro[0] + (destino.centro[0] - camara.centro[0]) * k,
                camara.centro[1] + (destino.centro[1] - camara.centro[1]) * k,
            ];
            camara = if (px - destino.px).abs() < destino.px * 1e-4
                && (centro[0] - destino.centro[0]).abs() < destino.px * 0.05
                && (centro[1] - destino.centro[1]).abs() < destino.px * 0.05
            {
                destino
            } else {
                Camara { centro, px }
            };
            sucio = true;
        }
        if barra_sucia && barra_visible {
            let avisos = [
                false,
                false,
                false,
                false,
                false,
                false,
                !faltan_letras.is_empty(),
                false,
            ];
            let m = barra_con_avisos(
                e,
                &BOTONES,
                boton_encima,
                claro,
                &[
                    false,
                    false,
                    acotar.activo,
                    marcando,
                    anotando,
                    false,
                    false,
                    fijada,
                ],
                &avisos,
            );
            if let Ok(p) = gpu_barra.subir(&m) {
                let (gw, gh) = gpu_barra.tamano();
                let fondo = if claro {
                    [0.976, 0.976, 0.984, 1.0]
                } else {
                    [0.118, 0.118, 0.125, 1.0]
                };
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
            let fondo = if claro {
                [0.99, 0.988, 0.976, 1.0]
            } else {
                [0.13, 0.15, 0.19, 1.0]
            };
            let color7 = if claro {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            let mut capas: Vec<(&PlanoGpu, Vista)> = Vec::new();
            if let Some((_, p)) = &plano {
                capas.push((p, camara.vista(w, h, color7, if claro { 2.0 } else { 1.0 })));
            }
            if mensaje.is_empty() {
                aviso_ui = None;
            } else if let Some((m, p)) = &aviso_ui {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            // Las cotas, encima de todo.
            let encima =
                if acotar.activo || !acotar.hechas.is_empty() || marcando || !marcos.is_empty() {
                    let unidades = plano.as_ref().map_or(0, |(m, _)| m.unidades);
                    let pista = if marcando {
                        &textos_ui.pista_marcos
                    } else {
                        &textos_ui.pista_acotar
                    };
                    let vista_marcos = Marcos {
                        hechos: &marcos,
                        nuevo: nuevo_marco,
                        marcando,
                    };
                    let m = encima_del_plano(
                        &mut textos,
                        &acotar,
                        &vista_marcos,
                        &camara,
                        w,
                        h,
                        e,
                        unidades,
                        claro,
                        pista,
                    );
                    gpu.subir(&m).ok().map(|p| (m, p))
                } else {
                    None
                };
            if let Some((m, p)) = &encima {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            let civil = if aviso_civil_hasta.is_some() && !acotar.activo {
                let t = textos_ui
                    .sin_dibujo_civil
                    .replace("{n}", &sin_dibujo_civil.to_string());
                let m = pastilla_abajo(&mut textos, &t, h, e, claro);
                gpu.subir(&m).ok().map(|p| (m, p))
            } else if aviso_letras_hasta.is_some() && !acotar.activo {
                // Como el aviso de Android al abrir: «Faltan N letras… toca Aa».
                let t = textos_ui
                    .faltan_letras
                    .replace("{n}", &faltan_letras.len().to_string());
                let m = pastilla_abajo(&mut textos, &t, h, e, claro);
                gpu.subir(&m).ok().map(|p| (m, p))
            } else {
                None
            };
            if let Some((m, p)) = &civil {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            let dibujado = match (anotador.as_mut(), &plano) {
                (Some(a), Some((m, _))) => {
                    let v = vista_para_el_motor(hwnd, &camara, w, h, e, m, claro);
                    gpu.dibujar_con(fondo, &capas, &mut |d, t| a.pintar(d, t, &v, anotando))
                }
                _ => gpu.dibujar(fondo, &capas),
            };
            if let Err(err) = dibujado {
                tracing::warn!(error = %err, "no se pudo dibujar el plano");
            }
            // Lo anotado iba en su foto estirada (o a medio rehacer): otro
            // fotograma, ya nitido, aunque la camara no se mueva.
            sucio = anotador.as_ref().is_some_and(|a| a.falta_pintar());
        }
    }
    if anotando && let Some(a) = anotador.as_mut() {
        a.terminar();
    }
    // SAFETY: ventanas propias.
    unsafe {
        let _ = DestroyWindow(hbarra);
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

/// **Si el plano (o el modelo 3D) nace «siempre encima»: no** (13-oct-2026,
/// el usuario: «los archivos que no aparezcan pineados ... por defecto»). Es
/// un archivo abierto para verlo junto a lo demas; el pin de la barra (o la
/// T) lo fija.
pub const FIJADA_AL_NACER: bool = false;

pub(crate) fn poner_encima(hwnd: HWND, si: bool) {
    // SAFETY: ventana propia.
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            Some(if si { HWND_TOPMOST } else { HWND_NOTOPMOST }),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
}

/// Los textos de la ventana, en el idioma de la app.
#[derive(Debug, Clone)]
pub struct TextosUi {
    pub abriendo: String,
    pub vacio: String,
    pub error: String,
    pub pista_acotar: String,
    pub todo: String,
    pub tema: String,
    pub acotar: String,
    pub borrar_cotas: String,
    pub encima: String,
    pub cerrar: String,
    pub tres: String,
    pub marcos: String,
    pub anotar: String,
    pub imprimir: String,
    pub exportar: String,
    pub borrar_marcos: String,
    pub pista_marcos: String,
    /// Con `{n}`: cuantos objetos de Civil 3D vinieron sin su dibujo.
    pub sin_dibujo_civil: String,
    /// El boton «Aa» (como «Letras del plano» de Android).
    pub letras: String,
    pub letras_faltan: String,
    pub letras_no_falta: String,
    /// Donde suelen estar, un renglon del menu por linea.
    pub letras_donde: String,
    pub letras_anadir: String,
    /// Con `{n}`: cuantas puso el usuario.
    pub letras_quitar: String,
    /// Con `{n}`: cuantas faltan.
    pub faltan_letras: String,
    pub releyendo: String,
}

impl Default for TextosUi {
    fn default() -> Self {
        Self {
            abriendo: "Abriendo el plano…".into(),
            vacio: "El plano no tiene nada que dibujar".into(),
            error: "No se pudo abrir el plano".into(),
            pista_acotar: "Clic en los puntos para medir · Doble clic o Intro: terminar · Esc: salir".into(),
            todo: "Ver todo el plano\tF".into(),
            tema: "Fondo claro u oscuro\tB".into(),
            acotar: "Acotar plano\tM".into(),
            borrar_cotas: "Borrar las cotas\tSupr".into(),
            encima: "Siempre encima\tT".into(),
            cerrar: "Cerrar\tEsc".into(),
            tres: "Ver en 3D\t3".into(),
            marcos: "Marcos para imprimir\tI".into(),
            anotar: "Anotar\tA".into(),
            imprimir: "Imprimir…\tCtrl+P".into(),
            exportar: "Exportar (HTML, PDF, PNG, DXF…)…\tCtrl+E".into(),
            borrar_marcos: "Borrar los marcos".into(),
            pista_marcos: "Arrastra para marcar lo que se imprime · Ctrl+P: imprimir · Supr: quitar el último".into(),
            sin_dibujo_civil: "{n} objetos de Civil 3D se guardaron sin su dibujo: guárdalo con PROXYGRAPHICS = 1 o exporta a LandXML".into(),
            letras: "Letras del plano…	L".into(),
            letras_faltan: "Este plano pide letras que no están; sus textos salen con otras parecidas y pueden no caber en sus recuadros:".into(),
            letras_no_falta: "No falta ninguna letra que el plano diga por su nombre.".into(),
            letras_donde: concat!(
                r"Las SHX suelen estar en C:\Program Files\Autodesk\AutoCAD 20xx\Fonts o junto al plano que te pasaron;",
                "\n",
                r"las TrueType, en C:\Windows\Fonts. Las que añadas aquí mandan sobre las de AutoCAD, Windows y PixPin."
            )
            .into(),
            letras_anadir: "Añadir letras (SHX, TTF, OTF, TTC)…".into(),
            letras_quitar: "Quitar las mías ({n})".into(),
            faltan_letras: "Faltan {n} letras del plano: pulsa Aa arriba para añadirlas".into(),
            releyendo: "Leyendo el plano con tus letras…".into(),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_rueda_deja_quieto_el_punto_bajo_el_cursor() {
        let c = Camara {
            centro: [100.0, 50.0],
            px: 2.0,
        };
        let antes = c.plano(30.0, 400.0, 800, 600);
        let z = c.zoom(0.5, 30.0, 400.0, 800, 600);
        let despues = z.plano(30.0, 400.0, 800, 600);
        assert!((antes[0] - despues[0]).abs() < 1e-9 && (antes[1] - despues[1]).abs() < 1e-9);
        assert_eq!(z.px, 1.0);
        // Ida y vuelta entre pantalla y plano.
        let p = c.pantalla(antes, 800, 600);
        assert!((p[0] - 30.0).abs() < 1e-9 && (p[1] - 400.0).abs() < 1e-9);
    }

    #[test]
    fn encuadrar_mete_el_plano_entero_con_margen() {
        let c = Camara::encuadrar([-100.0, -10.0, 100.0, 10.0], 1000, 500);
        assert_eq!(c.centro, [0.0, 0.0]);
        assert!((c.px - 200.0 / 920.0).abs() < 1e-9);
        // Caso negativo: una caja vacia no da una escala absurda.
        assert!(Camara::encuadrar([0.0; 4], 100, 100).px > 0.0);
    }

    #[test]
    fn el_plano_y_el_modelo_nacen_sin_fijar() {
        // Un archivo abierto es una ventana mas; el pin (o la T) lo fija.
        const { assert!(!FIJADA_AL_NACER) };
    }

    #[test]
    fn la_barra_va_encima_si_cabe_y_si_no_debajo() {
        let t = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let p = RECT {
            left: 100,
            top: 300,
            right: 900,
            bottom: 800,
        };
        assert_eq!(colocar_barra(p, t, 120, 48, 8), (100, 244));
        let p = RECT {
            left: 100,
            top: 20,
            right: 900,
            bottom: 800,
        };
        assert_eq!(colocar_barra(p, t, 120, 48, 8), (100, 808));
        // Caso negativo: sin sitio fuera, dentro y arriba.
        let p = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        assert_eq!(colocar_barra(p, t, 120, 48, 8), (8, 8));
    }
}

#[cfg(test)]
mod pruebas_letras {
    use super::*;

    /// El boton «Aa» esta en la barra, antes de la chincheta, y el menu
    /// tiene su entrada; sin letras que falten, el «Aa» no va en ambar.
    #[test]
    fn el_boton_aa_esta_en_la_barra_y_en_el_menu() {
        let i = BOTONES.iter().position(|b| *b == "letras").unwrap();
        assert_eq!(BOTONES[i + 1], "fijar");
        let t = TextosUi::default();
        assert!(
            t.letras.contains("\tL")
                && t.faltan_letras.contains("{n}")
                && t.letras_quitar.contains("{n}")
        );
        // Con aviso, el «Aa» cambia de color; sin aviso, no.
        let con = barra_con_avisos(
            1.0,
            &BOTONES,
            None,
            false,
            &[],
            &[false, false, false, false, false, false, true, false],
        );
        let sin = barra(1.0, &BOTONES, None, false, &[]);
        let colores = |m: &Modelo| {
            m.vertices
                .iter()
                .map(|v| v.color)
                .collect::<std::collections::BTreeSet<u32>>()
        };
        assert!(colores(&con).contains(&rgba(0xFF, 0xB0, 0x20)));
        assert!(!colores(&sin).contains(&rgba(0xFF, 0xB0, 0x20)));
    }

    /// Para mirarla: `PIXPIN_CAD_PNG=<png> cargo test -p pixpin-cad --lib barra_con_aa -- --ignored`.
    #[test]
    #[ignore = "pinta la barra del visor de planos a PNG"]
    fn barra_con_aa_a_png() {
        let ruta = std::env::var("PIXPIN_CAD_PNG").unwrap_or_else(|_| "barra-planos.png".into());
        let e = 2.0;
        let avisos = [false, false, false, false, false, false, true, false];
        let m = barra_con_avisos(
            e,
            &BOTONES,
            None,
            false,
            &[false, false, false, false, false, false, false, true],
            &avisos,
        );
        let (w, h) = (ancho_barra(e, BOTONES.len()), ALTO_BARRA * e);
        let mut l = crate::pruebas_png::Lienzo::nuevo(
            &m,
            [w / 2.0, h / 2.0],
            w,
            w.ceil() as usize,
            h.ceil() as usize,
        );
        l.pintar(&m);
        // La barra va hacia abajo y el lienzo de pruebas hacia arriba.
        let filas: Vec<Vec<[u8; 4]>> = l.px.chunks(l.w).map(|f| f.to_vec()).rev().collect();
        l.px = filas.concat();
        l.guardar(&ruta);
    }
}
