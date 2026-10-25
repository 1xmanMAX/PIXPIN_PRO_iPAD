//! El icono de la esquina y el panel de la pila.
//!
//! Es una sola ventana con dos tamanos: cerrada es el montoncito de
//! miniaturas en la esquina; abierta es el panel con todas las capturas.
//! Una sola porque abrir el panel no tiene que poder dejar dos ventanas
//! desincronizadas, y porque un clic fuera del panel lo cierra sin mas.
//!
//! La receta de «no robar el foco» esta copiada de `pixpin-pin::paleta`, que
//! ya la resuelve: `WS_EX_NOACTIVATE` **y** `WM_MOUSEACTIVATE` devolviendo
//! `MA_NOACTIVATE`, y `SW_SHOWNOACTIVATE` al mostrar. Cinturon y tirantes, y
//! aqui importa el doble: el usuario esta escribiendo en otra aplicacion
//! mientras el icono aparece, y perder el foco a media frase seria peor que
//! no tener pila.
//!
//! **Pasante por definicion:** la ventana ocupa exactamente su rectangulo, y
//! fuera de el no hay nada que tape ningun clic. Por eso no hace falta
//! `WS_EX_TRANSPARENT`: no hay ninguna zona muerta que dejar pasar.
//!
//! # El consumo
//!
//! Sin pila no hay ventana, y sin ventana no hay ni un mensaje ni un
//! temporizador. Con pila hay UN temporizador de un disparo, que se rearma
//! y que se mata en cuanto dispara. Nada se repinta por fotograma: se pinta
//! al aparecer, al entrar una captura y al tocar el panel.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Once;

use pixpin_geom::Rect;
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::pila::{Esquina, Pila, rect_del_icono};

/// Lado del icono cerrado, en pixeles logicos (los de 100 %).
pub const LADO_ICONO_LOGICO: u32 = 84;
/// Separacion del icono con los bordes del monitor, en pixeles logicos.
pub const MARGEN_ICONO_LOGICO: u32 = 24;

/// Identificador del unico temporizador de esta ventana.
const ID_TEMPORIZADOR: usize = 1;

/// Medidas del panel, en pixeles logicos.
const PANEL_MARGEN: f32 = 14.0;
/// El lado de la celda manda tambien el ancho del panel, y con el el de los
/// botones: con celdas de 76 el rotulo «Copiar elegidas» salia cortado con
/// puntos suspensivos al 100 %.
const PANEL_CELDA: f32 = 82.0;
const PANEL_HUECO: f32 = 10.0;
const PANEL_COLUMNAS: usize = 4;
const PANEL_CABECERA: f32 = 30.0;
const PANEL_BOTON_ALTO: f32 = 34.0;

#[derive(Debug, thiserror::Error)]
pub enum ErrorPila {
    #[error("no se pudo crear la ventana de la pila")]
    Creacion(#[source] windows::core::Error),
    #[error("no se pudo preparar el dibujo de la pila")]
    Dibujo(#[from] pixpin_render::ErrorRender),
}

/// Lo que el usuario pide desde la ventana y que ella sola no puede hacer.
///
/// Marcar, desmarcar y quitar los hace la propia ventana sobre la pila
/// compartida (son cambios de la pila y nada mas); lo que sale de aqui es lo
/// que necesita el portapapeles o el ejecutable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionPila {
    /// Se pulso «Copiar las elegidas».
    CopiarElegidas,
    /// Se pulso «Copiar todas».
    CopiarTodas,
    /// La pila cambio (se marco, se desmarco o se quito una, o se cerro el
    /// panel): hay que repintar y, si quedo vacia, cerrar.
    Cambiada,
    /// Un clic en el recuadro lo armo: bordes en azul y, desde ya, cada
    /// captura se suma a la tanda. El segundo clic llega como `Cerrar`.
    Armada,
    /// Se acabo el tiempo o se pulso «Quitar»: la pila se va.
    Cerrar,
}

/// Los rotulos ya traducidos. Vienen de fuera porque el catalogo Fluent vive
/// en `pixpin-store`, que es de la misma capa y no se puede ver desde aqui.
#[derive(Debug, Clone, Default)]
pub struct TextosPila {
    pub titulo: String,
    pub copiar_elegidas: String,
    pub copiar_todas: String,
    pub quitar: String,
}

/// Donde cae cada cosa dentro de la ventana, en pixeles de la ventana.
///
/// Es logica pura y va aparte del dibujo a proposito: es el calculo que hay
/// que poder probar sin pantalla, y es el mismo que usa el reparto del clic.
/// Si pintar y acertar el clic no salieran de aqui, un dia dejarian de
/// coincidir y el boton respondeia un centimetro mas abajo de donde se ve.
#[derive(Debug, Clone, PartialEq)]
pub struct Disposicion {
    pub abierto: bool,
    pub ancho: u32,
    pub alto: u32,
    pub escala: f32,
    /// Una por captura, en el orden de la pila.
    pub celdas: Vec<RectF>,
    /// La «x» de quitar de cada celda, en el mismo orden.
    pub quitar_una: Vec<RectF>,
    pub copiar_elegidas: RectF,
    pub copiar_todas: RectF,
    pub quitar_todas: RectF,
}

/// Que hay debajo de un punto de la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zona {
    /// El icono cerrado: el clic lo abre.
    Icono,
    /// Marcar o desmarcar la captura `i`.
    Celda(usize),
    /// Quitar la captura `i` de la pila.
    QuitarUna(usize),
    CopiarElegidas,
    CopiarTodas,
    QuitarTodas,
    Nada,
}

/// Calcula la disposicion. `escala_por_cien` es la del monitor (150 en el del
/// usuario); `cuantas` es el numero de capturas.
pub fn disponer(abierto: bool, cuantas: usize, escala_por_cien: u32) -> Disposicion {
    let escala = escala_por_cien.max(100) as f32 / 100.0;
    if !abierto {
        let lado = (LADO_ICONO_LOGICO as f32 * escala).round() as u32;
        return Disposicion {
            abierto: false,
            ancho: lado,
            alto: lado,
            escala,
            celdas: Vec::new(),
            quitar_una: Vec::new(),
            copiar_elegidas: RectF {
                x: 0.0,
                y: 0.0,
                ancho: 0.0,
                alto: 0.0,
            },
            copiar_todas: RectF {
                x: 0.0,
                y: 0.0,
                ancho: 0.0,
                alto: 0.0,
            },
            quitar_todas: RectF {
                x: 0.0,
                y: 0.0,
                ancho: 0.0,
                alto: 0.0,
            },
        };
    }

    let m = PANEL_MARGEN * escala;
    let celda = PANEL_CELDA * escala;
    let hueco = PANEL_HUECO * escala;
    let cabecera = PANEL_CABECERA * escala;
    let boton_alto = PANEL_BOTON_ALTO * escala;

    // Siempre cuatro columnas, aunque haya una sola captura: un panel que
    // cambia de ancho segun cuantas hay baila de sitio en cada captura nueva
    // y el boton se mueve bajo el dedo.
    let ancho_f = m * 2.0 + PANEL_COLUMNAS as f32 * celda + (PANEL_COLUMNAS as f32 - 1.0) * hueco;
    let filas = cuantas.div_ceil(PANEL_COLUMNAS).max(1);
    let alto_rejilla = filas as f32 * celda + (filas as f32 - 1.0) * hueco;
    let alto_f = m + cabecera + alto_rejilla + hueco + boton_alto + m;

    let mut celdas = Vec::with_capacity(cuantas);
    let mut quitar_una = Vec::with_capacity(cuantas);
    let lado_x = 18.0 * escala;
    for i in 0..cuantas {
        let col = i % PANEL_COLUMNAS;
        let fila = i / PANEL_COLUMNAS;
        let r = RectF {
            x: m + col as f32 * (celda + hueco),
            y: m + cabecera + fila as f32 * (celda + hueco),
            ancho: celda,
            alto: celda,
        };
        // La «x» muerde la esquina superior derecha de la celda; se dibuja y
        // se reparte el clic desde el mismo rectangulo.
        quitar_una.push(RectF {
            x: r.x + r.ancho - lado_x,
            y: r.y,
            ancho: lado_x,
            alto: lado_x,
        });
        celdas.push(r);
    }

    let y_botones = m + cabecera + alto_rejilla + hueco;
    let ancho_boton = (ancho_f - m * 2.0 - hueco * 2.0) / 3.0;
    let boton = |n: f32| RectF {
        x: m + n * (ancho_boton + hueco),
        y: y_botones,
        ancho: ancho_boton,
        alto: boton_alto,
    };

    Disposicion {
        abierto: true,
        ancho: ancho_f.round() as u32,
        alto: alto_f.round() as u32,
        escala,
        celdas,
        quitar_una,
        copiar_elegidas: boton(0.0),
        copiar_todas: boton(1.0),
        quitar_todas: boton(2.0),
    }
}

/// Que hay bajo el punto `(x, y)`, en pixeles de la ventana.
pub fn zona_en(d: &Disposicion, x: f32, y: f32) -> Zona {
    let dentro = |r: &RectF| x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto;
    if !d.abierto {
        return if x >= 0.0 && y >= 0.0 && x < d.ancho as f32 && y < d.alto as f32 {
            Zona::Icono
        } else {
            Zona::Nada
        };
    }
    // La «x» va ANTES que la celda: esta encima, y preguntarlo al reves haria
    // que quitar una captura solo la desmarcara.
    for (i, r) in d.quitar_una.iter().enumerate() {
        if dentro(r) {
            return Zona::QuitarUna(i);
        }
    }
    for (i, r) in d.celdas.iter().enumerate() {
        if dentro(r) {
            return Zona::Celda(i);
        }
    }
    if dentro(&d.copiar_elegidas) {
        return Zona::CopiarElegidas;
    }
    if dentro(&d.copiar_todas) {
        return Zona::CopiarTodas;
    }
    if dentro(&d.quitar_todas) {
        return Zona::QuitarTodas;
    }
    Zona::Nada
}

/// Colores del icono y del panel. Oscuros y opacos: el icono aparece encima
/// de lo que sea que el usuario tenga delante, y uno translucido sobre una
/// pagina blanca no se veria.
const FONDO: Color = Color {
    r: 0.13,
    g: 0.14,
    b: 0.16,
    a: 0.97,
};
const BORDE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.18,
};
const TEXTO: Color = Color {
    r: 0.93,
    g: 0.94,
    b: 0.95,
    a: 1.0,
};
const TEXTO_FLOJO: Color = Color {
    r: 0.66,
    g: 0.68,
    b: 0.72,
    a: 1.0,
};
const BOTON: Color = Color {
    r: 0.22,
    g: 0.24,
    b: 0.28,
    a: 1.0,
};

/// Encaja `(ancho, alto)` dentro de `caja` sin deformar y centrado. Una
/// captura apaisada dentro de una celda cuadrada estirada se ve mal y ademas
/// enganar sobre lo que se copio.
fn encajar(caja: RectF, ancho: u32, alto: u32) -> RectF {
    if ancho == 0 || alto == 0 {
        return caja;
    }
    let escala = (caja.ancho / ancho as f32).min(caja.alto / alto as f32);
    let w = ancho as f32 * escala;
    let h = alto as f32 * escala;
    RectF {
        x: caja.x + (caja.ancho - w) / 2.0,
        y: caja.y + (caja.alto - h) / 2.0,
        ancho: w,
        alto: h,
    }
}

/// Pinta la pila entera (icono cerrado o panel abierto).
///
/// Es publica y no toca la ventana a proposito: una prueba la llama contra un
/// destino fuera de pantalla y guarda el PNG, que es la unica forma de mirar
/// lo que se dibuja sin abrir la aplicacion.
///
/// `bitmaps` tiene que traer uno por captura y en el mismo orden; si faltan,
/// esas celdas salen vacias en vez de romperse.
pub fn pintar_pila(
    p: &Pintor,
    pila: &Pila,
    bitmaps: &[ID2D1Bitmap1],
    d: &Disposicion,
    textos: &TextosPila,
) {
    p.limpiar_transparente();
    if d.abierto {
        pintar_panel(p, pila, bitmaps, d, textos);
    } else {
        pintar_icono(p, pila, bitmaps, d);
    }
}

fn pintar_icono(p: &Pintor, pila: &Pila, bitmaps: &[ID2D1Bitmap1], d: &Disposicion) {
    let e = d.escala;
    let lado = d.ancho as f32;
    let radio = 10.0 * e;
    let cuantas = pila.cuantas();

    // El montoncito: hasta dos tarjetas detras, asomando hacia arriba y a la
    // derecha, para que se vea de un vistazo que hay mas de una sin tener que
    // leer el numero. Van todas del mismo tamano y se dibujan de la de mas
    // atras a la de delante; corrieron ademas de arriba abajo porque un
    // montoncito que solo se corre en horizontal parece una tarjeta ancha.
    let asomo = 6.0 * e;
    let detras = (cuantas.saturating_sub(1)).min(2);
    let lado_tarjeta = lado - asomo * detras as f32;
    for n in (1..=detras).rev() {
        let k = n as f32;
        let atras = RectF {
            x: asomo * k,
            y: asomo * (detras as f32 - k),
            ancho: lado_tarjeta,
            alto: lado_tarjeta,
        };
        // Cada escalon hacia atras, un poco mas claro: es lo que separa una
        // tarjeta de la siguiente sobre un escritorio oscuro, donde un borde
        // fino solo no se ve.
        let claro = 0.10 * k;
        p.rellenar_redondeado(
            atras,
            radio,
            Color {
                r: FONDO.r + claro,
                g: FONDO.g + claro,
                b: FONDO.b + claro,
                a: 1.0,
            },
        );
        p.trazar(atras, 1.0 * e, BORDE);
    }

    let caja = RectF {
        x: 0.0,
        y: asomo * detras as f32,
        ancho: lado_tarjeta,
        alto: lado_tarjeta,
    };
    p.rellenar_redondeado(caja, radio, FONDO);

    // La miniatura de la ultima, con un margen para que se vea el marco.
    let hueco = 6.0 * e;
    let dentro = RectF {
        x: caja.x + hueco,
        y: caja.y + hueco,
        ancho: caja.ancho - 2.0 * hueco,
        alto: caja.alto - 2.0 * hueco,
    };
    if let (Some(c), Some(b)) = (pila.ultima(), bitmaps.last()) {
        p.bitmap(
            b,
            encajar(dentro, c.miniatura.ancho, c.miniatura.alto),
            None,
            false,
        );
    }
    if pila.armada() {
        pintar_halo(p, caja, e);
    } else {
        p.trazar(caja, 1.0 * e, BORDE);
    }

    // El numero, solo a partir de dos: con una sola no aporta nada y quita
    // sitio a la miniatura, que es lo que el usuario mira.
    if cuantas >= 2 {
        let r = 13.0 * e;
        let centro = (lado - r - 1.0 * e, r + 1.0 * e);
        p.circulo(centro, r, Color::ACENTO);
        let n = cuantas.to_string();
        let tam = 15.0 * e;
        let (w, h) = p.medir_texto(&n, tam);
        p.texto(
            &n,
            centro.0 - w / 2.0,
            centro.1 - h / 2.0,
            tam,
            Color::BLANCO,
        );
    }
}

/// El halo azul del recuadro armado: un borde firme y dos difuminados hacia
/// dentro. Va por dentro de la tarjeta porque la ventana mide justo lo que el
/// icono: un brillo por fuera quedaria cortado por el borde de la ventana.
fn pintar_halo(p: &Pintor, caja: RectF, e: f32) {
    for (n, alfa) in [(2.0f32, 0.30f32), (1.0, 0.55), (0.0, 1.0)] {
        let dentro = n * 2.5 * e;
        let r = RectF {
            x: caja.x + dentro,
            y: caja.y + dentro,
            ancho: caja.ancho - 2.0 * dentro,
            alto: caja.alto - 2.0 * dentro,
        };
        p.trazar(
            r,
            if n == 0.0 { 3.0 * e } else { 2.5 * e },
            Color {
                a: alfa,
                ..Color::ACENTO
            },
        );
    }
}

fn pintar_panel(
    p: &Pintor,
    pila: &Pila,
    bitmaps: &[ID2D1Bitmap1],
    d: &Disposicion,
    textos: &TextosPila,
) {
    let e = d.escala;
    let marco = RectF {
        x: 0.0,
        y: 0.0,
        ancho: d.ancho as f32,
        alto: d.alto as f32,
    };
    p.rellenar_redondeado(marco, 12.0 * e, FONDO);
    p.trazar(marco, 1.0 * e, BORDE);

    p.texto_linea(
        &textos.titulo,
        PANEL_MARGEN * e,
        PANEL_MARGEN * e,
        15.0 * e,
        marco.ancho - PANEL_MARGEN * 2.0 * e,
        TEXTO,
    );

    for (i, celda) in d.celdas.iter().enumerate() {
        let Some(c) = pila.capturas().get(i) else {
            continue;
        };
        p.rellenar_redondeado(*celda, 8.0 * e, BOTON);
        if let Some(b) = bitmaps.get(i) {
            let hueco = 4.0 * e;
            let dentro = RectF {
                x: celda.x + hueco,
                y: celda.y + hueco,
                ancho: celda.ancho - 2.0 * hueco,
                alto: celda.alto - 2.0 * hueco,
            };
            p.bitmap(
                b,
                encajar(dentro, c.miniatura.ancho, c.miniatura.alto),
                None,
                false,
            );
        }
        // Marcada: aro de acento alrededor y un punto lleno abajo a la
        // izquierda. Dos senales y no una porque el aro solo se distingue mal
        // sobre una captura clara.
        if c.elegida {
            p.trazar(*celda, 2.5 * e, Color::ACENTO);
            p.circulo(
                (celda.x + 11.0 * e, celda.y + celda.alto - 11.0 * e),
                6.0 * e,
                Color::ACENTO,
            );
        } else {
            p.trazar(*celda, 1.0 * e, BORDE);
            p.anillo(
                (celda.x + 11.0 * e, celda.y + celda.alto - 11.0 * e),
                6.0 * e,
                1.5 * e,
                TEXTO_FLOJO,
            );
        }
        if let Some(x) = d.quitar_una.get(i) {
            let c2 = (x.x + x.ancho / 2.0, x.y + x.alto / 2.0);
            p.circulo(c2, x.ancho / 2.0, Color::NEGRO);
            let b = x.ancho * 0.22;
            p.linea((c2.0 - b, c2.1 - b), (c2.0 + b, c2.1 + b), 1.6 * e, TEXTO);
            p.linea((c2.0 - b, c2.1 + b), (c2.0 + b, c2.1 - b), 1.6 * e, TEXTO);
        }
    }

    let boton = |r: RectF, rotulo: &str, encendido: bool| {
        p.rellenar_redondeado(r, 7.0 * e, if encendido { Color::ACENTO } else { BOTON });
        let tam = 11.5 * e;
        let (w, h) = p.medir_texto(rotulo, tam);
        p.texto_linea(
            rotulo,
            r.x + ((r.ancho - w) / 2.0).max(4.0 * e),
            r.y + (r.alto - h) / 2.0,
            tam,
            r.ancho - 8.0 * e,
            if encendido { Color::BLANCO } else { TEXTO },
        );
    };
    let hay_elegidas = pila.capturas().iter().any(|c| c.elegida);
    boton(d.copiar_elegidas, &textos.copiar_elegidas, hay_elegidas);
    boton(d.copiar_todas, &textos.copiar_todas, false);
    boton(d.quitar_todas, &textos.quitar, false);
}

// ---------------------------------------------------------------------------
// La ventana
// ---------------------------------------------------------------------------

struct PilaInterno {
    motor: Rc<MotorRender>,
    superficie: Superficie,
    pila: Rc<RefCell<Pila>>,
    /// Un bitmap por captura, en el orden de la pila. Se rehace entero cuando
    /// la pila cambia: son como mucho 24 miniaturas de 160 px, y rehacerlas
    /// solo pasa cuando el usuario hace algo, nunca por fotograma.
    bitmaps: Vec<ID2D1Bitmap1>,
    abierto: bool,
    esquina: Esquina,
    escala_por_cien: u32,
    textos: TextosPila,
    al_actuar: Box<dyn Fn(AccionPila)>,
}

/// La ventanita de la esquina. Se destruye al soltarla.
pub struct IconoPila {
    hwnd: HWND,
}

static REGISTRO: Once = Once::new();

impl IconoPila {
    /// Crea el icono en la esquina del monitor donde se hizo la captura.
    ///
    /// `al_actuar` se llama DESDE el procedimiento de ventana, es decir desde
    /// el bucle principal del ejecutable: tiene que apuntar el pedido y
    /// volver, no ponerse a trabajar (la trampa de siempre, la misma que
    /// documenta `CambioPin` en `pixpin-pin`).
    pub fn nuevo(
        d3d: &ID3D11Device,
        motor: Rc<MotorRender>,
        pila: Rc<RefCell<Pila>>,
        esquina: Esquina,
        textos: TextosPila,
        al_actuar: Box<dyn Fn(AccionPila)>,
    ) -> Result<IconoPila, ErrorPila> {
        REGISTRO.call_once(registrar_clase);
        let (monitor, escala_por_cien) = pila.borrow().monitor().unwrap_or((
            Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080,
            },
            100,
        ));
        let d = disponer(false, pila.borrow().cuantas(), escala_por_cien);
        let sitio = rect_del_icono(
            monitor,
            escala_por_cien,
            esquina,
            LADO_ICONO_LOGICO,
            MARGEN_ICONO_LOGICO,
        );

        // SAFETY: la clase quedo registrada en call_once; estilos constantes
        // documentados; modulo propio.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("PixPinPila"),
                w!(""),
                WS_POPUP,
                sitio.x,
                sitio.y,
                d.ancho as i32,
                d.alto as i32,
                None,
                None,
                Some(GetModuleHandleW(None).map_err(ErrorPila::Creacion)?.into()),
                None,
            )
            .map_err(ErrorPila::Creacion)?
        };

        let superficie = Superficie::nueva(&motor, d3d, hwnd, d.ancho, d.alto)?;
        let interno = Box::new(PilaInterno {
            motor,
            superficie,
            pila,
            bitmaps: Vec::new(),
            abierto: false,
            esquina,
            escala_por_cien,
            textos,
            al_actuar,
        });
        // SAFETY: la ventana es propia y viva; el Box se cede al USERDATA y
        // se recupera exactamente una vez en WM_NCDESTROY.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(interno) as isize);
        }
        let icono = IconoPila { hwnd };
        icono.refrescar();
        // SAFETY: mostrar SIN activar. Es la linea que evita que aparecer en
        // la esquina le quite el teclado a la aplicacion donde se escribe.
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(icono)
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn abierto(&self) -> bool {
        interno_de(self.hwnd).map(|i| i.abierto).unwrap_or(false)
    }

    /// La escala del monitor para la que se hizo la ventana. Una tanda nueva
    /// en un monitor con la MISMA escala reutiliza la ventana (crearla cuesta
    /// una superficie de composicion entera, decenas de ms en el hilo de los
    /// gestos); con otra escala hay que hacerla de nuevo.
    pub fn escala_por_cien(&self) -> Option<u32> {
        interno_de(self.hwnd).map(|i| i.escala_por_cien)
    }

    /// Vuelve del panel al recuadro, sin tocar la pila. Es lo que pasa tras
    /// copiar con la tanda armada: se sigue agrupando.
    pub fn cerrar_panel(&self) {
        let Some(i) = interno_de(self.hwnd) else {
            return;
        };
        if i.abierto {
            i.abierto = false;
            recolocar(self.hwnd, i);
            pintar(i);
        }
    }

    /// Rehace las miniaturas, recoloca la ventana y repinta. Se llama cuando
    /// entra una captura nueva y cuando el panel se abre o se cierra.
    pub fn refrescar(&self) {
        let Some(i) = interno_de(self.hwnd) else {
            return;
        };
        rehacer_bitmaps(i);
        recolocar(self.hwnd, i);
        pintar(i);
    }

    /// Cambia el titulo del panel. Va aparte de los demas rotulos porque es
    /// el unico que depende de cuantas capturas hay; no repinta, lo hace el
    /// `refrescar` que viene detras.
    pub fn poner_titulo(&self, titulo: String) {
        if let Some(i) = interno_de(self.hwnd) {
            i.textos.titulo = titulo;
        }
    }

    /// Arma el temporizador de un disparo que esconde la pila. `ms` a cero lo
    /// desarma: es lo que hace que sin pila no quede nada corriendo.
    pub fn armar_desvanecido(&self, ms: u32) {
        armar(self.hwnd, ms);
    }
}

impl Drop for IconoPila {
    fn drop(&mut self) {
        // SAFETY: destruir una ventana propia desde su hilo; WM_NCDESTROY
        // libera el Box. El temporizador muere con la ventana.
        unsafe {
            let _ = KillTimer(Some(self.hwnd), ID_TEMPORIZADOR);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn armar(hwnd: HWND, ms: u32) {
    // SAFETY: temporizador de una ventana propia. Volver a llamar con el
    // mismo id REARMA la cuenta, que es justo lo que se quiere cuando entra
    // otra captura.
    unsafe {
        if ms == 0 {
            let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR);
        } else {
            SetTimer(Some(hwnd), ID_TEMPORIZADOR, ms, None);
        }
    }
}

/// El interno colgado del USERDATA, si la ventana sigue viva.
fn interno_de<'a>(hwnd: HWND) -> Option<&'a mut PilaInterno> {
    // SAFETY: el puntero lo puso IconoPila::nuevo y solo WM_NCDESTROY lo
    // retira; entre ambos es un Box valido. Todo ocurre en el hilo de
    // interfaz.
    unsafe {
        let crudo = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PilaInterno;
        crudo.as_mut()
    }
}

fn registrar_clase() {
    // SAFETY: registro unico (Once) de una clase con WndProc propio; los
    // campos no usados quedan a cero, que es lo que la API espera.
    unsafe {
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento_pila),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinPila"),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

fn rehacer_bitmaps(i: &mut PilaInterno) {
    let pila = i.pila.borrow();
    let mut nuevos = Vec::with_capacity(pila.cuantas());
    for c in pila.capturas() {
        match i.motor.bitmap_desde_pixeles(
            c.miniatura.ancho,
            c.miniatura.alto,
            &c.miniatura.pixeles,
        ) {
            Ok(b) => nuevos.push(b),
            // Una miniatura que no se puede subir a la GPU deja su celda
            // vacia; perder el icono entero por una captura rara seria peor.
            Err(e) => {
                tracing::warn!(?e, "una miniatura de la pila no se pudo preparar");
                break;
            }
        }
    }
    drop(pila);
    i.bitmaps = nuevos;
}

/// Pone la ventana del tamano que le toca y pegada a su esquina.
fn recolocar(hwnd: HWND, i: &PilaInterno) {
    let cuantas = i.pila.borrow().cuantas();
    let (monitor, _) = i.pila.borrow().monitor().unwrap_or((
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        },
        i.escala_por_cien,
    ));
    let d = disponer(i.abierto, cuantas, i.escala_por_cien);
    // El panel crece hacia dentro desde la MISMA esquina que el icono: asi la
    // esquina que el usuario mira no se mueve al abrirlo.
    let ancla = rect_del_icono(
        monitor,
        i.escala_por_cien,
        i.esquina,
        LADO_ICONO_LOGICO,
        MARGEN_ICONO_LOGICO,
    );
    let derecha = matches!(i.esquina, Esquina::ArribaDerecha | Esquina::AbajoDerecha);
    let abajo = matches!(i.esquina, Esquina::AbajoIzquierda | Esquina::AbajoDerecha);
    let x = if derecha {
        ancla.x + ancla.ancho as i32 - d.ancho as i32
    } else {
        ancla.x
    };
    let y = if abajo {
        ancla.y + ancla.alto as i32 - d.alto as i32
    } else {
        ancla.y
    };
    // Y sin salirse del monitor por el lado contrario, que es lo que pasa con
    // el panel abierto en un monitor bajo.
    let x = x
        .max(monitor.x)
        .min(monitor.x + monitor.ancho as i32 - d.ancho as i32);
    let y = y
        .max(monitor.y)
        .min(monitor.y + monitor.alto as i32 - d.alto as i32);

    let _ = i.superficie.redimensionar(d.ancho, d.alto);
    // SAFETY: mover una ventana propia sin activarla y sin tocar el orden Z.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            d.ancho as i32,
            d.alto as i32,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

fn pintar(i: &PilaInterno) {
    let cuantas = i.pila.borrow().cuantas();
    let d = disponer(i.abierto, cuantas, i.escala_por_cien);
    let Ok(destino) = i.superficie.empezar(&i.motor) else {
        return;
    };
    let pila = i.pila.borrow();
    let _ = i.motor.dibujar(&destino, |p| {
        pintar_pila(p, &pila, &i.bitmaps, &d, &i.textos);
    });
    drop(pila);
    let _ = i.superficie.presentar();
}

extern "system" fn procedimiento_pila(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match mensaje {
        // Ni el clic la activa: el usuario esta escribiendo en otra
        // aplicacion y el foco tiene que quedarse donde esta.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONUP => {
            atender_clic(hwnd, lparam);
            LRESULT(0)
        }
        // El clic derecho abre (o cierra) el panel para elegir cuales copiar:
        // el izquierdo del recuadro es el interruptor de agrupar.
        WM_RBUTTONUP => {
            alternar_panel(hwnd);
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ID_TEMPORIZADOR => {
            // Lo primero, matarlo: de un disparo. Si no, la ventana seguiria
            // recibiendo un mensaje cada N segundos para siempre.
            // SAFETY: temporizador propio de una ventana propia.
            unsafe {
                let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR);
            }
            if let Some(i) = interno_de(hwnd) {
                // Con el panel abierto NO se va sola: el usuario esta
                // mirandola y eligiendo, y desaparecer a media eleccion seria
                // el peor momento posible.
                // Y armada tampoco: es el usuario quien la mantiene en
                // trabajo, y solo el la suelta.
                if !i.abierto && !i.pila.borrow().armada() {
                    (i.al_actuar)(AccionPila::Cerrar);
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca la ventana como pintada; el dibujo
            // real lo hace el swapchain, no GDI.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            if let Some(i) = interno_de(hwnd) {
                pintar(i);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // SAFETY: recupera el Box cedido en IconoPila::nuevo exactamente
            // una vez y deja el USERDATA a cero antes de soltarlo.
            unsafe {
                let crudo = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut PilaInterno;
                if !crudo.is_null() {
                    drop(Box::from_raw(crudo));
                }
            }
            LRESULT(0)
        }
        // NUNCA PostQuitMessage: cerrar la pila no apaga la aplicacion.
        // SAFETY: delegar al procedimiento por defecto es el protocolo.
        _ => unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) },
    }
}

/// Abre el panel desde el recuadro, o lo cierra de vuelta al recuadro.
fn alternar_panel(hwnd: HWND) {
    let Some(i) = interno_de(hwnd) else {
        return;
    };
    if i.abierto {
        i.abierto = false;
        recolocar(hwnd, i);
        pintar(i);
        // Quien manda en el desvanecido lo vuelve a armar si toca.
        (i.al_actuar)(AccionPila::Cambiada);
    } else {
        // Abrir el panel desarma el desvanecido: mientras se elige, la pila
        // no se va sola.
        i.abierto = true;
        armar(hwnd, 0);
        recolocar(hwnd, i);
        pintar(i);
    }
}

fn atender_clic(hwnd: HWND, lparam: LPARAM) {
    let Some(i) = interno_de(hwnd) else {
        return;
    };
    let x = (lparam.0 & 0xFFFF) as i16 as f32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
    let cuantas = i.pila.borrow().cuantas();
    let d = disponer(i.abierto, cuantas, i.escala_por_cien);
    match zona_en(&d, x, y) {
        Zona::Icono => {
            // El interruptor de agrupar (2-oct). Sin armar, el clic lo arma:
            // bordes en azul, ya no se va solo y las capturas que vengan se
            // suman. Armado, el clic lo suelta todo y el recuadro se va.
            let armada = i.pila.borrow().armada();
            if armada {
                (i.al_actuar)(AccionPila::Cerrar);
            } else if i.pila.borrow_mut().armar(true) {
                armar(hwnd, 0);
                pintar(i);
                (i.al_actuar)(AccionPila::Armada);
            }
        }
        Zona::Celda(n) => {
            i.pila.borrow_mut().alternar(n);
            pintar(i);
            (i.al_actuar)(AccionPila::Cambiada);
        }
        Zona::QuitarUna(n) => {
            i.pila.borrow_mut().quitar(n);
            rehacer_bitmaps(i);
            recolocar(hwnd, i);
            pintar(i);
            (i.al_actuar)(AccionPila::Cambiada);
        }
        Zona::CopiarElegidas => (i.al_actuar)(AccionPila::CopiarElegidas),
        Zona::CopiarTodas => (i.al_actuar)(AccionPila::CopiarTodas),
        Zona::QuitarTodas => (i.al_actuar)(AccionPila::Cerrar),
        Zona::Nada => {}
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cerrada_la_ventana_es_un_cuadrado_del_lado_del_icono() {
        let d = disponer(false, 3, 100);
        assert_eq!((d.ancho, d.alto), (LADO_ICONO_LOGICO, LADO_ICONO_LOGICO));
        assert!(d.celdas.is_empty(), "cerrada no hay celdas que pulsar");
        assert_eq!(zona_en(&d, 10.0, 10.0), Zona::Icono);
        // Caso negativo: fuera del cuadrado no hay nada. La ventana no tapa
        // ni un pixel mas de lo que ocupa.
        assert_eq!(zona_en(&d, -1.0, 10.0), Zona::Nada);
        assert_eq!(zona_en(&d, 10.0, LADO_ICONO_LOGICO as f32), Zona::Nada);
    }

    #[test]
    fn al_ciento_cincuenta_por_ciento_todo_se_escala() {
        let d = disponer(false, 1, 150);
        assert_eq!(d.ancho, (LADO_ICONO_LOGICO as f32 * 1.5) as u32);
        let abierto = disponer(true, 4, 150);
        let cien = disponer(true, 4, 100);
        assert!(
            abierto.ancho > cien.ancho && abierto.alto > cien.alto,
            "el panel del monitor al 150 % tiene que salir mas grande"
        );
    }

    #[test]
    fn el_panel_pone_cuatro_por_fila_y_no_cambia_de_ancho() {
        let uno = disponer(true, 1, 100);
        let ocho = disponer(true, 8, 100);
        assert_eq!(
            uno.ancho, ocho.ancho,
            "el ancho no puede bailar entre capturas o el boton se mueve bajo el dedo"
        );
        assert!(ocho.alto > uno.alto, "ocho ocupan dos filas");
        // La quinta empieza fila nueva, justo debajo de la primera.
        assert_eq!(ocho.celdas[4].x, ocho.celdas[0].x);
        assert!(ocho.celdas[4].y > ocho.celdas[0].y);
    }

    #[test]
    fn cada_celda_responde_a_su_clic_y_la_equis_gana_a_la_celda() {
        let d = disponer(true, 6, 100);
        let c = d.celdas[3];
        // El centro de la celda la marca o la desmarca.
        assert_eq!(
            zona_en(&d, c.x + c.ancho / 2.0, c.y + c.alto / 2.0),
            Zona::Celda(3)
        );
        // La esquina superior derecha la quita: la «x» esta encima.
        let x = d.quitar_una[3];
        assert_eq!(
            zona_en(&d, x.x + x.ancho / 2.0, x.y + x.alto / 2.0),
            Zona::QuitarUna(3),
            "si esto diera Celda, la «x» solo desmarcaria y nadie podria quitar una"
        );
    }

    #[test]
    fn los_tres_botones_caen_donde_se_pulsan_y_no_se_solapan() {
        let d = disponer(true, 3, 100);
        for (r, esperada) in [
            (d.copiar_elegidas, Zona::CopiarElegidas),
            (d.copiar_todas, Zona::CopiarTodas),
            (d.quitar_todas, Zona::QuitarTodas),
        ] {
            assert_eq!(
                zona_en(&d, r.x + r.ancho / 2.0, r.y + r.alto / 2.0),
                esperada
            );
        }
        assert!(d.copiar_elegidas.x + d.copiar_elegidas.ancho <= d.copiar_todas.x);
        assert!(d.copiar_todas.x + d.copiar_todas.ancho <= d.quitar_todas.x);
        assert!(
            d.quitar_todas.x + d.quitar_todas.ancho <= d.ancho as f32,
            "el tercer boton no se sale del panel"
        );
    }

    #[test]
    fn todo_lo_pulsable_cae_dentro_del_panel() {
        // Caso negativo con el panel lleno: nada puede quedarse fuera de la
        // ventana, porque lo que cae fuera no recibe clics.
        let d = disponer(true, crate::pila::TOPE_CAPTURAS, 150);
        for r in d.celdas.iter().chain(d.quitar_una.iter()).chain([
            &d.copiar_elegidas,
            &d.copiar_todas,
            &d.quitar_todas,
        ]) {
            assert!(r.x >= 0.0 && r.y >= 0.0, "nada empieza fuera: {r:?}");
            assert!(
                r.x + r.ancho <= d.ancho as f32 + 1.0 && r.y + r.alto <= d.alto as f32 + 1.0,
                "nada acaba fuera: {r:?} en {}x{}",
                d.ancho,
                d.alto
            );
        }
    }

    #[test]
    fn un_hueco_del_panel_no_es_ningun_boton() {
        // Entre la cabecera y la primera fila no hay nada que pulsar.
        let d = disponer(true, 4, 100);
        assert_eq!(zona_en(&d, 2.0, 2.0), Zona::Nada);
    }

    #[test]
    fn encajar_no_deforma_una_captura_apaisada() {
        let caja = RectF {
            x: 10.0,
            y: 20.0,
            ancho: 100.0,
            alto: 100.0,
        };
        let r = encajar(caja, 200, 100);
        assert_eq!((r.ancho, r.alto), (100.0, 50.0), "mantiene la proporcion");
        assert_eq!(r.y, 45.0, "y queda centrada en vertical");
        // Caso negativo: una miniatura de cero no divide por cero.
        assert_eq!(encajar(caja, 0, 0), caja);
    }
}
