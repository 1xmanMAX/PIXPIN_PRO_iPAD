//! La ficha de un chat: lo que sale al pulsar su nombre en la cabecera.
//!
//! Geometria pura, como `chat` e `historial`. Las medidas salen del analisis
//! del panel de «medios compartidos» de Telegram Desktop; **de ahi solo
//! medidas y tecnicas, nunca su codigo, que es GPL-3.0.**
//!
//! Las SECCIONES, en cambio, viven en `pixpin_proyecto::cuaderno::Seccion`:
//! las de PixPin Android (todo, fotos, archivos, voz, dibujos, fijados y
//! buzon) y, desde el 25-oct-2026, videos y enlaces, como Telegram. La
//! disposicion es la suya; lo que se lista es lo nuestro.
//!
//! La ficha es la **isla** ([`isla_en`], [`Isla`]): una tarjeta flotante con
//! su cabecera (avatar, nombre, lupa y aspa), la tira de pestanas y el
//! contenido ([`Lista`]), o la previa de una cosa ([`previa`]). Quien pinta
//! y quien reparte los clics usan estas mismas cuentas.
//!
//! Decisiones que no son evidentes (la pasada del 25-oct-2026, el usuario:
//! «funciona pero no funciona bien y no se ve bien»):
//!
//! - **Las medidas no dependen del tamano de la ventana**: la cabecera, las
//!   pestanas, las filas y los botones miden siempre lo mismo (por el DPI,
//!   claro). Lo unico que cambia con el ancho es cuantas columnas tiene la
//!   rejilla, de tres a cinco, como Telegram.
//! - **Las pestanas son texto con una rayita de color debajo**, en una sola
//!   fila que nunca parte ni recorta un rotulo: si no caben, la tira se
//!   desplaza (rueda) y la activa siempre se ve. Si sobran, el sitio se
//!   reparte entre todas, que llenan la tira.
//! - **La rejilla va de borde a borde con 2 px entre fotos** y celdas
//!   cuadradas: una foto apaisada y una vertical ocupan lo mismo.
//! - **Lo de cada mes va bajo su rotulo** («octubre de 2026»), en la rejilla
//!   y en las filas: es como se busca algo en un chat largo.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
///
/// Lo alta que es la caja de buscar dentro de la cabecera, y la letra que
/// lleva. La misma que el buscador de la lista de chats: es la misma clase
/// de caja y desigualarlas solo se notaria para mal.
pub const BUSCAR_ALTO: u32 = 32;
pub const BUSCAR_TAM: f32 = 13.0;
pub const BUSCAR_TEXTO_X: u32 = 12;

// --- La isla ------------------------------------------------------------------

/// Lo estrecha y lo ancha que puede ser la isla.
pub const ISLA_ANCHO_MINIMO: u32 = 360;
pub const ISLA_ANCHO_MAXIMO: u32 = 720;
/// La parte del ancho de la ventana que se lleva (en por ciento).
pub const ISLA_FRACCION: u32 = 58;
/// Ni en un monitor altisimo se estira mas de esto.
pub const ISLA_ALTO_MAXIMO: u32 = 860;
/// Lo que se deja como minimo a los lados, y arriba y abajo.
pub const ISLA_MARGEN_X: u32 = 32;
pub const ISLA_MARGEN_Y: u32 = 24;
pub const ISLA_RADIO: u32 = 16;
/// El aire de la tarjeta por dentro, a los lados (la rejilla no lo lleva:
/// va de borde a borde).
pub const ISLA_RELLENO: u32 = 16;
/// La cabecera: el avatar redondo, el nombre y lo que tiene; la lupa y el
/// aspa a la derecha, botones redondos de 32.
pub const ISLA_CABECERA: u32 = 64;
pub const ISLA_AVATAR: u32 = 40;
pub const ISLA_BOTON: u32 = 32;
pub const ISLA_BOTON_HUECO: u32 = 4;
pub const ISLA_TITULO_TAM: f32 = 15.0;
pub const ISLA_SUBTITULO_TAM: f32 = 12.5;
/// La tira de pestanas: texto con la rayita de color debajo.
pub const ISLA_TIRA: u32 = 44;
pub const ISLA_PESTANA_RELLENO: u32 = 14;
pub const ISLA_PESTANA_TAM: f32 = 13.5;
pub const ISLA_SUBRAYADO: u32 = 3;
/// La rejilla: de tres a cinco columnas de al menos esto, con 2 entre ellas.
pub const ISLA_CELDA_MINIMA: u32 = 120;
pub const ISLA_CELDA_HUECO: u32 = 2;
pub const ISLA_COLUMNAS_MINIMO: u32 = 3;
pub const ISLA_COLUMNAS_MAXIMO: u32 = 5;
/// El rotulo de cada mes.
pub const ISLA_MES: u32 = 36;
pub const ISLA_MES_TAM: f32 = 13.0;
/// Las filas: archivos, voz y lo demas en dos lineas; los enlaces en tres.
pub const ISLA_FILA: u32 = 60;
pub const ISLA_FILA_ENLACE: u32 = 72;
pub const ISLA_FILA_MINIATURA: u32 = 44;
/// Lo que se deja debajo de todo, para que lo ultimo no toque el borde.
pub const ISLA_PIE_DE_LISTA: u32 = 8;
/// La barra de desplazamiento, encima del contenido.
pub const ISLA_BARRA: u32 = 4;
pub const ISLA_BARRA_MINIMA: u32 = 28;
/// La previa: las flechas, encima de la cosa a los lados.
pub const ISLA_FLECHA: u32 = 40;
pub const ISLA_PREVIA_MARGEN: u32 = 12;

fn e(v: u32, escala_por_cien: u32) -> u32 {
    v * escala_por_cien / 100
}

/// Donde cae la isla dentro de la ventana, y si se lo queda todo porque la
/// ventana no da ni para la mas estrecha con sus margenes.
pub fn isla_en(ventana: Rect, escala_por_cien: u32) -> (Rect, bool) {
    let e = |v: u32| e(v, escala_por_cien);
    if ventana.ancho < e(ISLA_ANCHO_MINIMO) + 2 * e(ISLA_MARGEN_X) {
        return (ventana, true);
    }
    let ancho = (ventana.ancho * ISLA_FRACCION / 100)
        .clamp(e(ISLA_ANCHO_MINIMO), e(ISLA_ANCHO_MAXIMO))
        .min(ventana.ancho - 2 * e(ISLA_MARGEN_X));
    let alto = ventana
        .alto
        .saturating_sub(2 * e(ISLA_MARGEN_Y))
        .min(e(ISLA_ALTO_MAXIMO))
        .max(1);
    (
        Rect {
            x: ventana.x + (ventana.ancho as i32 - ancho as i32) / 2,
            y: ventana.y + (ventana.alto as i32 - alto as i32) / 2,
            ancho,
            alto,
        },
        false,
    )
}

/// Como queda repartida la isla. Una sola cuenta para pintarla y para
/// repartir sus clics (ver `ficha_del_chat::disposicion` en la ventana del
/// chat: hubo un tiempo en que eran dos, y los botones no respondian).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Isla {
    pub panel: Rect,
    pub cabecera: Rect,
    /// El avatar del chat; en la previa de una cosa, la flecha de volver.
    pub avatar: Rect,
    /// Donde va el nombre y, debajo, lo que tiene.
    pub titulo: Rect,
    pub buscar: Rect,
    pub cerrar: Rect,
    /// La caja de escribir lo que se busca, en el sitio del titulo.
    pub caja_buscar: Rect,
    /// En la previa, los dos botones de la cosa, a la izquierda del aspa:
    /// verla en el chat y abrirla.
    pub en_el_chat: Rect,
    pub abrir: Rect,
    /// El titulo de la previa, que acaba antes de esos dos botones.
    pub titulo_previa: Rect,
    /// La tira de pestanas, de lado a lado (sin el relleno).
    pub tira: Rect,
    pub contenido: Rect,
    /// La ventana no daba para la isla y la ocupa entera.
    pub pantalla_completa: bool,
}

impl Isla {
    pub fn calcular(caja: Rect, pantalla_completa: bool, escala_por_cien: u32) -> Isla {
        let e = |v: u32| e(v, escala_por_cien);
        let cabecera = Rect {
            alto: e(ISLA_CABECERA).min(caja.alto),
            ..caja
        };
        let medio = |lado: u32| cabecera.y + (cabecera.alto as i32 - lado as i32) / 2;
        let boton = e(ISLA_BOTON).min(cabecera.alto);
        let paso = (boton + e(ISLA_BOTON_HUECO)) as i32;
        let cerrar = Rect {
            x: cabecera.derecha() - e(ISLA_RELLENO - 4) as i32 - boton as i32,
            y: medio(boton),
            ancho: boton,
            alto: boton,
        };
        let buscar = Rect {
            x: cerrar.x - paso,
            ..cerrar
        };
        let abrir = buscar;
        let en_el_chat = Rect {
            x: abrir.x - paso,
            ..abrir
        };
        let lado = e(ISLA_AVATAR).min(cabecera.alto);
        let avatar = Rect {
            x: cabecera.x + e(ISLA_RELLENO) as i32,
            y: medio(lado),
            ancho: lado,
            alto: lado,
        };
        let tx = avatar.derecha() + e(12) as i32;
        let titulo = Rect {
            x: tx,
            y: cabecera.y,
            ancho: (buscar.x - e(8) as i32 - tx).max(0) as u32,
            alto: cabecera.alto,
        };
        let titulo_previa = Rect {
            ancho: (en_el_chat.x - e(8) as i32 - tx).max(0) as u32,
            ..titulo
        };
        let alto_caja = e(BUSCAR_ALTO).min(cabecera.alto);
        let caja_buscar = Rect {
            x: avatar.x,
            y: medio(alto_caja),
            ancho: (buscar.x - e(8) as i32 - avatar.x).max(0) as u32,
            alto: alto_caja,
        };
        let franja = e(ISLA_TIRA).min(caja.alto.saturating_sub(cabecera.alto));
        let tira = Rect {
            x: caja.x + e(ISLA_RELLENO / 2) as i32,
            y: cabecera.abajo(),
            ancho: caja.ancho.saturating_sub(e(ISLA_RELLENO)),
            alto: franja,
        };
        let arriba = tira.abajo();
        Isla {
            panel: caja,
            cabecera,
            avatar,
            titulo,
            buscar,
            cerrar,
            caja_buscar,
            en_el_chat,
            abrir,
            titulo_previa,
            tira,
            contenido: Rect {
                x: caja.x,
                y: arriba,
                ancho: caja.ancho,
                alto: (caja.abajo() - arriba).max(0) as u32,
            },
            pantalla_completa,
        }
    }

    /// **Donde cae cada pestana** en la tira, en el orden de los anchos de
    /// texto (que mide quien pinta), con la tira desplazada `desplazado`
    /// pixeles. Cada una lleva su texto entero y su relleno: nunca se parte
    /// ni se recorta un rotulo. Si caben todas, el sitio que sobra se
    /// reparte y llenan la tira (y no se desplaza); si no, van seguidas y la
    /// tira se puede desplazar hasta `tope`.
    pub fn pestanas(
        &self,
        anchos_de_texto: &[f32],
        desplazado: i32,
        escala_por_cien: u32,
    ) -> Pestanas {
        let relleno = (ISLA_PESTANA_RELLENO * escala_por_cien / 100) as f32;
        let naturales: Vec<f32> = anchos_de_texto
            .iter()
            .map(|w| w.max(0.0).ceil() + 2.0 * relleno)
            .collect();
        let total: f32 = naturales.iter().sum();
        let sitio = self.tira.ancho as f32;
        let (anchos, tope) = if total <= sitio && !naturales.is_empty() {
            let sobra = (sitio - total) / naturales.len() as f32;
            (naturales.iter().map(|w| w + sobra).collect(), 0)
        } else {
            (naturales, (total - sitio).ceil().max(0.0) as i32)
        };
        let desplazado = desplazado.clamp(0, tope);
        let mut x = self.tira.x as f32 - desplazado as f32;
        let rects = anchos
            .iter()
            .map(|ancho| {
                let izquierda = x.round() as i32;
                x += ancho;
                Rect {
                    x: izquierda,
                    y: self.tira.y,
                    ancho: (x.round() as i32 - izquierda).max(0) as u32,
                    alto: self.tira.alto,
                }
            })
            .collect();
        Pestanas {
            rects,
            tope,
            desplazado,
        }
    }

    /// Lo que ocupa la previa de una cosa: la tira y el contenido juntos
    /// (en la previa no hay pestanas).
    pub fn hueco_de_la_previa(&self) -> Rect {
        let y = self.cabecera.abajo();
        Rect {
            x: self.panel.x,
            y,
            ancho: self.panel.ancho,
            alto: (self.panel.abajo() - y).max(0) as u32,
        }
    }
}

/// Las pestanas ya colocadas en su tira.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pestanas {
    pub rects: Vec<Rect>,
    /// Hasta donde se puede desplazar la tira (cero si caben todas).
    pub tope: i32,
    /// Lo desplazada que esta de verdad, ya sujeto entre cero y `tope`.
    pub desplazado: i32,
}

impl Pestanas {
    /// Lo que hay que desplazar la tira para que la pestana `n` se vea
    /// entera, con un poco de aire; si ya se ve, lo de ahora.
    pub fn para_ver(&self, n: usize, tira: Rect, escala_por_cien: u32) -> i32 {
        let Some(r) = self.rects.get(n) else {
            return self.desplazado;
        };
        let aire = (ISLA_PESTANA_RELLENO * escala_por_cien / 100) as i32;
        let mut d = self.desplazado;
        if r.x < tira.x {
            d -= tira.x - r.x + aire;
        } else if r.derecha() > tira.derecha() {
            d += r.derecha() - tira.derecha() + aire;
        }
        d.clamp(0, self.tope)
    }

    /// La pestana bajo `p`, sin contar lo que se sale de la tira.
    pub fn en(&self, p: Punto, tira: Rect) -> Option<usize> {
        if !tira.contiene(p) {
            return None;
        }
        self.rects.iter().position(|r| r.contiene(p))
    }
}

/// **La rayita** de la pestana activa: tan ancha como su texto, centrada y
/// pegada abajo, con las puntas redondas.
pub fn subrayado(pestana: Rect, ancho_de_texto: f32, escala_por_cien: u32) -> Rect {
    let alto = e(ISLA_SUBRAYADO, escala_por_cien).max(1);
    let ancho = (ancho_de_texto.ceil().max(0.0) as u32).min(pestana.ancho);
    Rect {
        x: pestana.x + (pestana.ancho as i32 - ancho as i32) / 2,
        y: pestana.abajo() - alto as i32,
        ancho,
        alto,
    }
}

// --- El contenido: rejilla o filas, por meses --------------------------------

/// Una pieza del contenido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pieza {
    /// El rotulo de un mes; lleva el numero de la primera cosa de ese mes
    /// (para saber cual es).
    Mes(usize),
    /// La cosa numero `n` de la seccion, su celda o su fila.
    Cosa(usize),
}

/// **Como queda el contenido de una seccion**, sin desplazar: cada rotulo
/// de mes y cada cosa en su sitio. La misma cuenta para pintar, para el
/// clic, para el resalte del raton y para mover el foco con las flechas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lista {
    pub piezas: Vec<(Pieza, Rect)>,
    /// Lo que mide todo junto (para el tope de la rueda).
    pub alto: u32,
    /// Cuantas columnas tiene (una, en filas).
    pub columnas: u32,
}

/// Cuantas columnas y de que lado en una rejilla de borde a borde: de tres a
/// cinco, las que quepan con al menos la celda minima. Devuelve tambien el
/// margen que queda a la izquierda al centrarla (lo que no se reparte por
/// el redondeo).
pub fn columnas_isla(ancho: u32, escala_por_cien: u32) -> (u32, u32, u32) {
    let hueco = e(ISLA_CELDA_HUECO, escala_por_cien);
    let minima = e(ISLA_CELDA_MINIMA, escala_por_cien).max(1);
    let columnas =
        ((ancho + hueco) / (minima + hueco)).clamp(ISLA_COLUMNAS_MINIMO, ISLA_COLUMNAS_MAXIMO);
    let lado = (ancho.saturating_sub((columnas - 1) * hueco) / columnas).max(1);
    let ocupa = columnas * lado + (columnas - 1) * hueco;
    (columnas, lado, ancho.saturating_sub(ocupa) / 2)
}

/// **Coloca el contenido** en `area`: `meses` lleva la clave del mes de
/// cada cosa, en orden (cada vez que cambia sale un rotulo); `cuadricula`
/// dice si va en rejilla o en filas de `alto_fila` (logico).
pub fn lista_isla(
    area: Rect,
    meses: &[i64],
    cuadricula: bool,
    alto_fila: u32,
    escala_por_cien: u32,
) -> Lista {
    let e = |v: u32| e(v, escala_por_cien);
    let alto_mes = e(ISLA_MES);
    let hueco = e(ISLA_CELDA_HUECO) as i32;
    let (columnas, lado, margen) = if cuadricula {
        columnas_isla(area.ancho, escala_por_cien)
    } else {
        (1, e(alto_fila).max(1), 0)
    };
    let fila_x = area.x + e(ISLA_RELLENO / 2) as i32;
    let fila_ancho = area.ancho.saturating_sub(e(ISLA_RELLENO));
    let mut piezas = Vec::with_capacity(meses.len() + 8);
    let mut y = area.y;
    let mut columna = 0;
    for (n, mes) in meses.iter().enumerate() {
        if n == 0 || meses[n - 1] != *mes {
            if columna != 0 {
                // Se cierra la fila a medias del mes de antes.
                y += lado as i32 + hueco;
                columna = 0;
            }
            piezas.push((
                Pieza::Mes(n),
                Rect {
                    x: fila_x,
                    y,
                    ancho: fila_ancho,
                    alto: alto_mes,
                },
            ));
            y += alto_mes as i32;
        }
        let caja = if cuadricula {
            Rect {
                x: area.x + margen as i32 + (columna * (lado + hueco as u32)) as i32,
                y,
                ancho: lado,
                alto: lado,
            }
        } else {
            Rect {
                x: fila_x,
                y,
                ancho: fila_ancho,
                alto: lado,
            }
        };
        piezas.push((Pieza::Cosa(n), caja));
        columna += 1;
        if columna == columnas {
            columna = 0;
            y += lado as i32 + if cuadricula { hueco } else { 0 };
        }
    }
    if columna != 0 {
        y += lado as i32;
    }
    let alto = if meses.is_empty() {
        0
    } else {
        (y - area.y).max(0) as u32 + e(ISLA_PIE_DE_LISTA)
    };
    Lista {
        piezas,
        alto,
        columnas,
    }
}

/// **Filas sin rotulos de mes**, una bajo otra, para lo que no va por
/// fechas: la pestana de proyectos, que sigue el orden de la lista de chats
/// («Mensajes guardados» y el asistente delante, aunque sean viejos) y con
/// rotulos de mes repetiria meses. Arriba lleva el mismo aire que el pie,
/// para que la primera fila no pegue con la tira de pestanas.
pub fn filas_isla(area: Rect, cuantas: usize, alto_fila: u32, escala_por_cien: u32) -> Lista {
    let e = |v: u32| e(v, escala_por_cien);
    let alto = e(alto_fila).max(1);
    let aire = e(ISLA_PIE_DE_LISTA);
    let fila_x = area.x + e(ISLA_RELLENO / 2) as i32;
    let fila_ancho = area.ancho.saturating_sub(e(ISLA_RELLENO));
    let piezas = (0..cuantas)
        .map(|n| {
            (
                Pieza::Cosa(n),
                Rect {
                    x: fila_x,
                    y: area.y + aire as i32 + (n as u32 * alto) as i32,
                    ancho: fila_ancho,
                    alto,
                },
            )
        })
        .collect();
    Lista {
        piezas,
        alto: if cuantas == 0 {
            0
        } else {
            2 * aire + cuantas as u32 * alto
        },
        columnas: 1,
    }
}

impl Lista {
    /// Las piezas que se ven en `area` con el desplazamiento `scroll`, ya
    /// movidas a su sitio en la pantalla.
    pub fn visibles(&self, area: Rect, scroll: i32) -> Vec<(Pieza, Rect)> {
        self.piezas
            .iter()
            .map(|(p, r)| {
                (
                    *p,
                    Rect {
                        y: r.y - scroll,
                        ..*r
                    },
                )
            })
            .filter(|(_, r)| r.abajo() > area.y && r.y < area.abajo())
            .collect()
    }

    /// Donde esta la cosa `n`, sin desplazar.
    pub fn caja_de(&self, n: usize) -> Option<Rect> {
        self.piezas
            .iter()
            .find(|(p, _)| *p == Pieza::Cosa(n))
            .map(|(_, r)| *r)
    }

    /// **A que cosa se va con las flechas** desde la `actual`: a los lados,
    /// la de antes o la de despues; arriba y abajo, la de la fila de encima
    /// o de debajo que caiga mas cerca en horizontal (con los meses en
    /// medio, la fila de abajo no siempre esta a `columnas` de distancia).
    pub fn vecina(&self, actual: usize, dx: i32, dy: i32) -> Option<usize> {
        let cosas: Vec<(usize, Rect)> = self
            .piezas
            .iter()
            .filter_map(|(p, r)| match p {
                Pieza::Cosa(n) => Some((*n, *r)),
                Pieza::Mes(_) => None,
            })
            .collect();
        let (_, aqui) = *cosas.iter().find(|(n, _)| *n == actual)?;
        if dy == 0 {
            let k = actual as i64 + dx.signum() as i64;
            return (k >= 0)
                .then(|| cosas.iter().find(|(n, _)| *n as i64 == k).map(|(n, _)| *n))
                .flatten();
        }
        let centro = |r: &Rect| r.x + r.ancho as i32 / 2;
        let fila = cosas
            .iter()
            .filter(|(_, r)| if dy > 0 { r.y > aqui.y } else { r.y < aqui.y })
            .map(|(_, r)| r.y)
            .min_by_key(|y| (y - aqui.y).abs())?;
        cosas
            .iter()
            .filter(|(_, r)| r.y == fila)
            .min_by_key(|(_, r)| (centro(r) - centro(&aqui)).abs())
            .map(|(n, _)| *n)
    }

    /// El desplazamiento que deja ver entera la cosa `n` en un area de
    /// `alto` (lo de ahora si ya se ve).
    pub fn para_ver(&self, n: usize, area: Rect, scroll: i32) -> i32 {
        let Some(r) = self.caja_de(n) else {
            return scroll;
        };
        let arriba = r.y - area.y;
        let abajo = r.abajo() - area.y;
        let tope = (self.alto as i32 - area.alto as i32).max(0);
        let s = if arriba < scroll {
            arriba
        } else if abajo > scroll + area.alto as i32 {
            abajo - area.alto as i32
        } else {
            scroll
        };
        s.clamp(0, tope)
    }
}

/// **La barra de desplazamiento**, encima del contenido y pegada a su borde
/// derecho: solo si hay mas de lo que cabe.
pub fn barra_de_desplazamiento(
    area: Rect,
    alto_total: u32,
    scroll: i32,
    escala_por_cien: u32,
) -> Option<Rect> {
    if alto_total <= area.alto || area.alto == 0 {
        return None;
    }
    let ancho = e(ISLA_BARRA, escala_por_cien).max(1);
    let aire = e(3, escala_por_cien) as i32;
    let carril = area.alto.saturating_sub(2 * aire as u32);
    let alto = ((carril as u64 * area.alto as u64 / alto_total as u64) as u32)
        .max(e(ISLA_BARRA_MINIMA, escala_por_cien))
        .min(carril);
    let tope = (alto_total - area.alto) as i64;
    let recorrido = (carril - alto) as i64;
    let y = scroll.clamp(0, tope as i32) as i64 * recorrido / tope.max(1);
    Some(Rect {
        x: area.derecha() - aire - ancho as i32,
        y: area.y + aire + y as i32,
        ancho,
        alto,
    })
}

/// Lo de dentro de una fila de la isla: la miniatura (o la chapa) de 44 a la
/// izquierda y dos lineas de texto o, en una fila de enlace, tres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilaArchivo {
    pub fila: Rect,
    pub miniatura: Rect,
    pub nombre: Punto,
    pub estado: Punto,
    /// La tercera linea (el texto que acompana a un enlace), si cabe.
    pub tercera: Option<Punto>,
    pub ancho_texto: u32,
}

/// Coloca lo de dentro de una fila de la isla.
pub fn dentro_de_la_fila(fila: Rect, escala_por_cien: u32) -> FilaArchivo {
    let e = |v: u32| e(v, escala_por_cien);
    let lado = e(ISLA_FILA_MINIATURA).min(fila.alto);
    let x = fila.x + e(8) as i32;
    let miniatura = Rect {
        x,
        y: fila.y + (fila.alto as i32 - lado as i32) / 2,
        ancho: lado,
        alto: lado,
    };
    let texto_x = miniatura.derecha() + e(12) as i32;
    let derecha = fila.derecha() - e(8) as i32;
    let tres = fila.alto >= e(ISLA_FILA_ENLACE);
    // Las lineas, de 18 la primera y de 16 las otras, con 2 entre ellas y
    // centradas en la fila.
    let alto_texto = if tres {
        18 + 2 + 16 + 2 + 16
    } else {
        18 + 2 + 16
    };
    let arriba = fila.y + (fila.alto as i32 - e(alto_texto) as i32) / 2;
    FilaArchivo {
        fila,
        miniatura,
        nombre: Punto {
            x: texto_x,
            y: arriba,
        },
        estado: Punto {
            x: texto_x,
            y: arriba + e(20) as i32,
        },
        tercera: tres.then(|| Punto {
            x: texto_x,
            y: arriba + e(38) as i32,
        }),
        ancho_texto: (derecha - texto_x).max(0) as u32,
    }
}

// --- La previa ---------------------------------------------------------------

/// La previa de una cosa dentro de la isla: la cosa en grande y las flechas
/// encima, a los lados (como el visor de Telegram). Sus botones van en la
/// cabecera ([`Isla::abrir`], [`Isla::en_el_chat`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Previa {
    /// Donde se ensena la cosa en grande (la foto, el dibujo, su ficha).
    pub vista: Rect,
    pub anterior: Rect,
    pub siguiente: Rect,
}

/// Reparte el hueco de la previa (`Isla::hueco_de_la_previa`).
pub fn previa(hueco: Rect, escala_por_cien: u32) -> Previa {
    let e = |v: u32| e(v, escala_por_cien);
    let margen = e(ISLA_PREVIA_MARGEN);
    let vista = Rect {
        x: hueco.x + margen as i32,
        y: hueco.y + margen as i32,
        ancho: hueco.ancho.saturating_sub(2 * margen),
        alto: hueco.alto.saturating_sub(2 * margen),
    };
    let flecha = e(ISLA_FLECHA).min(vista.ancho / 3).min(vista.alto);
    let medio_y = vista.y + (vista.alto as i32 - flecha as i32) / 2;
    let anterior = Rect {
        x: vista.x + e(8) as i32,
        y: medio_y,
        ancho: flecha,
        alto: flecha,
    };
    let siguiente = Rect {
        x: vista.derecha() - e(8) as i32 - flecha as i32,
        ..anterior
    };
    Previa {
        vista,
        anterior,
        siguiente,
    }
}

/// Donde cabe una imagen de `ancho` x `alto` dentro de `caja` sin recortarla
/// ni deformarla, centrada (la previa de una foto; la rejilla recorta).
pub fn encajar(ancho: u32, alto: u32, caja: Rect) -> Rect {
    if ancho == 0 || alto == 0 || caja.ancho == 0 || caja.alto == 0 {
        return Rect {
            ancho: 0,
            alto: 0,
            ..caja
        };
    }
    let factor = (caja.ancho as f64 / ancho as f64).min(caja.alto as f64 / alto as f64);
    let w = ((ancho as f64 * factor).round() as u32).clamp(1, caja.ancho);
    let h = ((alto as f64 * factor).round() as u32).clamp(1, caja.alto);
    Rect {
        x: caja.x + (caja.ancho as i32 - w as i32) / 2,
        y: caja.y + (caja.alto as i32 - h as i32) / 2,
        ancho: w,
        alto: h,
    }
}

/// La duracion de un video o una nota de voz como se lee en su insignia:
/// «0:07», «12:30», «1:02:03». Sin duracion, nada.
pub fn duracion_corta(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let s = (ms + 500) / 1000;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

// --- Las fotos de las notas ricas en la ficha (25-oct-2026) -----------------
//
// Una nota rica lleva sus fotos dentro del texto (`![…](pixpin:files/…)`,
// ver `nota_rica` y `docs/nota-rica-del-chat.md`). En «Fotos» sale CADA UNA
// como una miniatura mas, en el sitio de su nota: quien busca la foto de la
// obra no piensa en si la mando sola o con texto.

/// Una cosa de la ficha: un mensaje entero o una foto de una nota rica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cosa {
    /// Su posicion en los mensajes del chat.
    pub mensaje: usize,
    /// En una nota rica, que foto suya es (en el orden de
    /// `nota_rica::imagenes`, el mismo que cuenta la burbuja); `None`, el
    /// mensaje entero.
    pub foto: Option<usize>,
}

impl Cosa {
    /// El mensaje entero.
    pub fn entera(mensaje: usize) -> Cosa {
        Cosa {
            mensaje,
            foto: None,
        }
    }
}

/// Las fotos de una nota rica, en el orden en que se ven en su burbuja. Lo
/// que no es una foto (un PDF, un audio) no cuenta. Sin `![` no se parte el
/// texto: la ficha pregunta por cada nota en cada fotograma.
pub fn fotos_de_la_nota(texto: &str) -> Vec<crate::nota_rica::Imagen> {
    if !texto.contains("![") {
        return Vec::new();
    }
    let bloques = crate::nota_rica::bloques(texto.trim());
    crate::nota_rica::imagenes(&bloques).cloned().collect()
}

/// **Lo que sale en «Fotos»**: los mensajes de foto (`fotos`) y, de cada
/// nota (`notas`), cada una de sus fotos, todo en el orden del chat. Las dos
/// listas son posiciones en los mensajes, de menor a mayor; `cuantas_fotos`
/// dice cuantas lleva la nota de esa posicion. Una nota sin fotos no sale.
pub fn con_las_fotos_de_las_notas(
    fotos: &[usize],
    notas: &[usize],
    cuantas_fotos: impl Fn(usize) -> usize,
) -> Vec<Cosa> {
    let mut salida = Vec::with_capacity(fotos.len());
    let (mut f, mut n) = (fotos.iter().peekable(), notas.iter().peekable());
    loop {
        match (f.peek(), n.peek()) {
            (Some(&&a), Some(&&b)) if a <= b => {
                salida.push(Cosa::entera(a));
                f.next();
            }
            (_, Some(&&b)) => {
                salida.extend((0..cuantas_fotos(b)).map(|k| Cosa {
                    mensaje: b,
                    foto: Some(k),
                }));
                n.next();
            }
            (Some(&&a), None) => {
                salida.push(Cosa::entera(a));
                f.next();
            }
            (None, None) => return salida,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ventana() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1024,
            alto: 768,
        }
    }

    fn isla(escala: u32) -> Isla {
        let (caja, completa) = isla_en(ventana(), escala);
        Isla::calcular(caja, completa, escala)
    }

    #[test]
    fn la_isla_crece_con_la_ventana_sin_pasarse_y_se_centra() {
        let (normal, completa) = isla_en(ventana(), 100);
        assert!(!completa);
        assert_eq!(normal.ancho, 1024 * ISLA_FRACCION / 100);
        let sobra = normal.x - (ventana().derecha() - normal.derecha());
        assert!(sobra.abs() <= 1, "centrada: {sobra}");
        assert_eq!(normal.y, ventana().abajo() - normal.abajo());
        let enorme = Rect {
            x: 0,
            y: 0,
            ancho: 3000,
            alto: 2000,
        };
        let (grande, _) = isla_en(enorme, 100);
        assert_eq!(grande.ancho, ISLA_ANCHO_MAXIMO);
        assert_eq!(grande.alto, ISLA_ALTO_MAXIMO);
        let (doble, _) = isla_en(enorme, 200);
        assert!(doble.ancho >= ISLA_ANCHO_MINIMO * 2);
        // Caso negativo: en una ventana pequena se lo queda todo.
        let pequena = Rect {
            x: 0,
            y: 0,
            ancho: ISLA_ANCHO_MINIMO + 2 * ISLA_MARGEN_X - 1,
            alto: 500,
        };
        assert_eq!(isla_en(pequena, 100), (pequena, true));
    }

    #[test]
    fn la_cabecera_mide_lo_mismo_en_cualquier_ventana_y_no_se_pisa() {
        let i = isla(100);
        assert_eq!(i.cabecera.alto, ISLA_CABECERA);
        assert_eq!((i.cerrar.ancho, i.cerrar.alto), (ISLA_BOTON, ISLA_BOTON));
        assert_eq!(i.avatar.ancho, ISLA_AVATAR);
        assert_eq!(i.avatar.x - i.panel.x, ISLA_RELLENO as i32);
        assert_eq!(i.buscar.derecha() + ISLA_BOTON_HUECO as i32, i.cerrar.x);
        assert!(i.avatar.derecha() < i.titulo.x);
        assert!(i.titulo.derecha() < i.buscar.x, "el nombre no pisa la lupa");
        assert!(i.caja_buscar.derecha() < i.buscar.x);
        // En la previa, sus dos botones a la izquierda del aspa, y el
        // titulo antes de ellos.
        assert_eq!(i.abrir.derecha() + ISLA_BOTON_HUECO as i32, i.cerrar.x);
        assert!(i.en_el_chat.derecha() < i.abrir.x);
        assert!(i.titulo_previa.derecha() < i.en_el_chat.x);
        // Tira y contenido, uno bajo otro hasta abajo.
        assert_eq!(i.tira.y, i.cabecera.abajo());
        assert_eq!(i.contenido.y, i.tira.abajo());
        assert_eq!(i.contenido.abajo(), i.panel.abajo());
        // Al 150 % todo mide la vez y media, sea cual sea la ventana.
        let g = isla(150);
        assert_eq!(g.cabecera.alto, ISLA_CABECERA * 3 / 2);
        assert_eq!(g.cerrar.ancho, ISLA_BOTON * 3 / 2);
        let h = i.hueco_de_la_previa();
        assert_eq!((h.y, h.abajo()), (i.cabecera.abajo(), i.panel.abajo()));
    }

    #[test]
    fn las_pestanas_que_caben_llenan_la_tira_y_no_se_desplazan() {
        let i = isla(100);
        let p = i.pestanas(&[40.0, 60.0, 50.0], 99, 100);
        assert_eq!(p.tope, 0);
        assert_eq!(p.desplazado, 0, "si caben, no hay nada que desplazar");
        assert_eq!(p.rects[0].x, i.tira.x);
        assert!((p.rects[2].derecha() - i.tira.derecha()).abs() <= 1);
        assert!(p.rects.windows(2).all(|w| w[0].derecha() == w[1].x));
        // Cada una al menos con su texto y su relleno.
        assert!(p.rects[1].ancho >= 60 + 2 * ISLA_PESTANA_RELLENO);
        // Caso negativo: sin pestanas no hay nada.
        assert!(i.pestanas(&[], 0, 100).rects.is_empty());
    }

    #[test]
    fn las_que_no_caben_no_se_recortan_y_la_tira_se_desplaza_hasta_la_activa() {
        let i = isla(100);
        let anchos = [90.0; 10];
        let p = i.pestanas(&anchos, 0, 100);
        assert!(p.tope > 0);
        assert!(
            p.rects
                .iter()
                .all(|r| r.ancho >= 90 + 2 * ISLA_PESTANA_RELLENO),
            "ningun rotulo se aprieta"
        );
        // La ultima no se ve; para verla hay que desplazar, sin pasarse.
        assert!(p.rects[9].derecha() > i.tira.derecha());
        let d = p.para_ver(9, i.tira, 100);
        assert!(d > 0 && d <= p.tope);
        let vista = i.pestanas(&anchos, d, 100);
        assert!(vista.rects[9].derecha() <= i.tira.derecha());
        // Y de vuelta a la primera.
        assert_eq!(vista.para_ver(0, i.tira, 100), 0);
        // Lo que asoma fuera de la tira no se pulsa.
        let fuera = Punto {
            x: i.tira.derecha() + 5,
            y: i.tira.y + 5,
        };
        assert_eq!(p.en(fuera, i.tira), None);
        assert_eq!(
            p.en(
                Punto {
                    x: p.rects[0].x + 3,
                    y: i.tira.y + 3
                },
                i.tira
            ),
            Some(0)
        );
        // Desplazar mas alla del tope se queda en el tope.
        assert_eq!(i.pestanas(&anchos, 1_000_000, 100).desplazado, p.tope);
    }

    #[test]
    fn la_rayita_va_bajo_el_texto_centrada() {
        let r = Rect {
            x: 100,
            y: 50,
            ancho: 80,
            alto: ISLA_TIRA,
        };
        let s = subrayado(r, 40.0, 100);
        assert_eq!(s.ancho, 40);
        assert_eq!(s.abajo(), r.abajo());
        assert_eq!(s.x - r.x, r.derecha() - s.derecha());
        assert_eq!(s.alto, ISLA_SUBRAYADO);
    }

    #[test]
    fn la_rejilla_tiene_de_tres_a_cinco_columnas_y_va_de_borde_a_borde() {
        for (ancho, columnas) in [(300, 3), (380, 3), (500, 4), (640, 5), (2000, 5)] {
            let (c, lado, margen) = columnas_isla(ancho, 100);
            assert_eq!(c, columnas, "a {ancho}");
            let ocupa = c * lado + (c - 1) * ISLA_CELDA_HUECO;
            assert!(ancho - ocupa < c, "de borde a borde a {ancho}");
            assert_eq!(margen, (ancho - ocupa) / 2);
        }
    }

    fn area() -> Rect {
        Rect {
            x: 10,
            y: 100,
            ancho: 640,
            alto: 400,
        }
    }

    #[test]
    fn cada_mes_lleva_su_rotulo_y_sus_cosas_debajo() {
        // Siete fotos: cinco de un mes y dos del siguiente.
        let meses = [5, 5, 5, 5, 5, 4, 4];
        let l = lista_isla(area(), &meses, true, ISLA_FILA, 100);
        assert_eq!(l.columnas, 5);
        let rotulos: Vec<usize> = l
            .piezas
            .iter()
            .filter_map(|(p, _)| match p {
                Pieza::Mes(n) => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(rotulos, [0, 5]);
        let a = l.caja_de(0).unwrap();
        assert_eq!(a.y, area().y + ISLA_MES as i32, "bajo su rotulo");
        let b = l.caja_de(1).unwrap();
        assert_eq!(b.x - a.derecha(), ISLA_CELDA_HUECO as i32);
        assert_eq!(a.ancho, a.alto, "cuadradas");
        // Las del mes siguiente, bajo su rotulo y empezando en la primera
        // columna.
        let f = l.caja_de(5).unwrap();
        assert_eq!(f.x, a.x);
        assert!(f.y >= a.abajo() + ISLA_MES as i32);
        // Caso negativo: sin cosas no hay nada que medir.
        let vacia = lista_isla(area(), &[], true, ISLA_FILA, 100);
        assert!(vacia.piezas.is_empty());
        assert_eq!(vacia.alto, 0);
    }

    #[test]
    fn las_filas_van_una_bajo_otra_y_solo_se_dan_las_que_se_ven() {
        let meses = vec![1; 100];
        let l = lista_isla(area(), &meses, false, ISLA_FILA, 100);
        let a = l.caja_de(0).unwrap();
        let b = l.caja_de(1).unwrap();
        assert_eq!(b.y, a.abajo());
        assert_eq!(a.alto, ISLA_FILA);
        assert_eq!(l.alto, ISLA_MES + 100 * ISLA_FILA + ISLA_PIE_DE_LISTA);
        let v = l.visibles(area(), 0);
        assert!(v.len() < 12, "solo las que entran: {}", v.len());
        let lejos = l.visibles(area(), 50 * ISLA_FILA as i32);
        assert!(lejos.iter().all(|(_, r)| r.abajo() > area().y));
        assert!(lejos.iter().any(|(p, _)| *p == Pieza::Cosa(50)));
    }

    #[test]
    fn las_filas_de_proyectos_van_seguidas_sin_rotulos_y_se_pulsan_donde_se_ven() {
        let l = filas_isla(area(), 30, ISLA_FILA, 100);
        assert!(
            l.piezas.iter().all(|(p, _)| matches!(p, Pieza::Cosa(_))),
            "sin rotulos de mes"
        );
        let a = l.caja_de(0).unwrap();
        assert_eq!(a.y, area().y + ISLA_PIE_DE_LISTA as i32, "con aire arriba");
        assert_eq!(l.caja_de(1).unwrap().y, a.abajo());
        assert_eq!(a.alto, ISLA_FILA);
        assert_eq!(l.alto, 2 * ISLA_PIE_DE_LISTA + 30 * ISLA_FILA);
        // Lo que se pinta desplazado es lo que recibe el clic: la fila 12,
        // bajada a la vista, esta donde la da `visibles`.
        let scroll = l.para_ver(12, area(), 0);
        let (_, r) = l
            .visibles(area(), scroll)
            .into_iter()
            .find(|(p, _)| *p == Pieza::Cosa(12))
            .unwrap();
        assert!(area().contiene(Punto {
            x: r.x + 5,
            y: r.y + 5
        }));
        assert_eq!(r.y, l.caja_de(12).unwrap().y - scroll);
        // Las flechas: arriba y abajo de una en una, y nada bajo la ultima.
        assert_eq!(l.vecina(0, 0, 1), Some(1));
        assert_eq!(l.vecina(5, 0, -1), Some(4));
        assert_eq!(l.vecina(29, 0, 1), None);
        // Caso negativo: sin proyectos no hay nada que medir.
        let vacia = filas_isla(area(), 0, ISLA_FILA, 100);
        assert!(vacia.piezas.is_empty());
        assert_eq!(vacia.alto, 0);
        // Y con la escala crecen las filas, no el numero de ellas.
        let grande = filas_isla(area(), 3, ISLA_FILA, 150);
        assert_eq!(grande.caja_de(0).unwrap().alto, ISLA_FILA * 3 / 2);
    }

    #[test]
    fn las_flechas_mueven_el_foco_por_la_rejilla_aunque_haya_meses_en_medio() {
        // Tres de un mes y cuatro del siguiente, en cinco columnas.
        let meses = [2, 2, 2, 1, 1, 1, 1];
        let l = lista_isla(area(), &meses, true, ISLA_FILA, 100);
        assert_eq!(l.vecina(0, 1, 0), Some(1));
        assert_eq!(l.vecina(1, -1, 0), Some(0));
        // Abajo desde la segunda: la del mes siguiente en su columna.
        assert_eq!(l.vecina(1, 0, 1), Some(4));
        assert_eq!(l.vecina(4, 0, -1), Some(1));
        // Caso negativo: no hay nada antes de la primera ni bajo la ultima.
        assert_eq!(l.vecina(0, -1, 0), None);
        assert_eq!(l.vecina(6, 0, 1), None);
        assert_eq!(l.vecina(6, 1, 0), None);
    }

    #[test]
    fn para_ver_una_cosa_se_desplaza_lo_justo() {
        let meses = vec![1; 100];
        let l = lista_isla(area(), &meses, false, ISLA_FILA, 100);
        assert_eq!(l.para_ver(0, area(), 0), 0, "ya se ve");
        let s = l.para_ver(20, area(), 0);
        let r = l.caja_de(20).unwrap();
        assert_eq!(r.abajo() - s, area().abajo());
        // Y subiendo, que quede arriba.
        assert_eq!(l.para_ver(3, area(), s), l.caja_de(3).unwrap().y - area().y);
    }

    #[test]
    fn la_barra_de_desplazamiento_solo_sale_si_hay_mas_de_lo_que_cabe() {
        assert_eq!(barra_de_desplazamiento(area(), 300, 0, 100), None);
        let arriba = barra_de_desplazamiento(area(), 1600, 0, 100).unwrap();
        let abajo = barra_de_desplazamiento(area(), 1600, 1200, 100).unwrap();
        assert!(arriba.y < abajo.y);
        assert!(arriba.alto >= ISLA_BARRA_MINIMA);
        assert!(abajo.abajo() <= area().abajo());
        assert!(arriba.derecha() <= area().derecha());
        // Un cuarto de lo que hay se ve: la barra mide en torno a un cuarto.
        assert!((arriba.alto as i32 - 100).abs() <= 3, "{}", arriba.alto);
    }

    #[test]
    fn en_una_fila_la_chapa_va_a_la_izquierda_y_el_texto_detras() {
        let fila = Rect {
            x: 0,
            y: 0,
            ancho: 500,
            alto: ISLA_FILA,
        };
        let f = dentro_de_la_fila(fila, 100);
        assert_eq!(f.miniatura.ancho, ISLA_FILA_MINIATURA);
        assert_eq!(f.miniatura.y - fila.y, fila.abajo() - f.miniatura.abajo());
        assert!(f.nombre.x > f.miniatura.derecha());
        assert!(f.estado.y > f.nombre.y);
        assert!(f.tercera.is_none(), "dos lineas");
        let enlace = dentro_de_la_fila(
            Rect {
                alto: ISLA_FILA_ENLACE,
                ..fila
            },
            100,
        );
        let t = enlace.tercera.expect("un enlace lleva tres lineas");
        assert!(t.y > enlace.estado.y && t.y + 16 <= ISLA_FILA_ENLACE as i32);
    }

    #[test]
    fn la_previa_pone_las_flechas_encima_de_la_cosa_a_los_lados() {
        let i = isla(100);
        let h = i.hueco_de_la_previa();
        let p = previa(h, 100);
        assert!(p.anterior.x >= p.vista.x && p.siguiente.derecha() <= p.vista.derecha());
        assert!(p.anterior.derecha() < p.siguiente.x);
        assert_eq!(
            p.anterior.y - p.vista.y,
            p.vista.abajo() - p.anterior.abajo(),
            "a media altura"
        );
        assert!(p.vista.abajo() <= h.abajo());
    }

    #[test]
    fn la_duracion_se_lee_como_en_un_reproductor() {
        assert_eq!(duracion_corta(7_000), "0:07");
        assert_eq!(duracion_corta(750_000), "12:30");
        assert_eq!(duracion_corta(3_723_000), "1:02:03");
        // Caso negativo: sin duracion no se dice nada.
        assert_eq!(duracion_corta(0), "");
    }

    #[test]
    fn las_fotos_de_una_nota_salen_cada_una_en_el_sitio_de_su_nota() {
        // Mensajes: 0 foto, 1 nota con dos fotos, 2 nota sin fotos, 3 foto,
        // 4 nota con una.
        let fotos = [0, 3];
        let notas = [1, 2, 4];
        let cuantas = |n: usize| match n {
            1 => 2,
            4 => 1,
            _ => 0,
        };
        let c = con_las_fotos_de_las_notas(&fotos, &notas, cuantas);
        let nota = |mensaje, k| Cosa {
            mensaje,
            foto: Some(k),
        };
        assert_eq!(
            c,
            vec![
                Cosa::entera(0),
                nota(1, 0),
                nota(1, 1),
                Cosa::entera(3),
                nota(4, 0)
            ]
        );
        // Caso negativo: sin notas, las fotos tal cual; sin nada, nada.
        assert_eq!(
            con_las_fotos_de_las_notas(&[5, 7], &[], cuantas),
            vec![Cosa::entera(5), Cosa::entera(7)]
        );
        assert!(con_las_fotos_de_las_notas(&[], &[2], cuantas).is_empty());
    }

    #[test]
    fn de_una_nota_solo_cuentan_sus_fotos_y_en_el_orden_de_la_burbuja() {
        let nota = "Visita\n![](pixpin:files/guardados/pc/c/notas/1-a.png)\n![planta|300](pixpin:files/guardados/pc/c/notas/2-b.jpg)\n\nY el plano\n![Plano.pdf](pixpin:files/guardados/pc/c/notas/3-Plano.pdf)\n![](notas/4-c.webp)";
        let f = fotos_de_la_nota(nota);
        let enlaces: Vec<&str> = f.iter().map(|i| i.enlace.as_str()).collect();
        assert_eq!(
            enlaces,
            [
                "pixpin:files/guardados/pc/c/notas/1-a.png",
                "pixpin:files/guardados/pc/c/notas/2-b.jpg",
                "notas/4-c.webp"
            ]
        );
        // El mismo orden en que la burbuja las cuenta para abrirlas.
        let b = crate::nota_rica::bloques(nota.trim());
        let de_la_burbuja: Vec<_> = crate::nota_rica::imagenes(&b).cloned().collect();
        assert_eq!(f, de_la_burbuja);
        // Caso negativo: una nota llana no tiene fotos.
        assert!(fotos_de_la_nota("hola, mira https://x.com/a.png").is_empty());
    }
}
