//! La pantalla de informacion de un proyecto: lo que sale al pulsar su
//! nombre en la cabecera.
//!
//! Geometria pura, como `chat` e `historial`. Las medidas salen del analisis
//! de Telegram Desktop; **de ahi solo medidas y tecnicas, nunca su codigo,
//! que es GPL-3.0.**
//!
//! Las SECCIONES, en cambio, viven en `pixpin_proyecto::cuaderno::Seccion`:
//! las de PixPin Android (todo, fotos, archivos, voz, dibujos, fijados y
//! buzon) y, desde el 25-oct-2026, videos y enlaces, como Telegram. La
//! disposicion es la suya; lo que se lista es lo nuestro.
//!
//! Hoy la ficha del chat es la **isla** (`isla_en`, [`Isla`], `previa`, al
//! final): la capa y la columna de antes quedan para quien las use.
//!
//! Tres decisiones que no son evidentes:
//!
//! - **La pildora de la pestana activa NO es una rayita.** En Telegram es una
//!   pildora rellena que se desliza Y cambia de ancho entre pestana y
//!   pestana. Una rayita es mas facil y se ve distinto.
//! - **La cuadricula se centra.** Se calcula cuantas columnas caben con la
//!   celda minima, se reparte el sobrante entre ellas y el margen se recalcula
//!   para centrarla. Sin eso sobra un pico a la derecha que canta.
//! - **Las celdas son cuadradas.** Una foto apaisada y una vertical ocupan lo
//!   mismo; es lo que hace que una cuadricula se lea de un vistazo.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
/// La columna de informacion, a la derecha del todo.
pub const ANCHO_MINIMO: u32 = 292;
pub const ANCHO_MAXIMO: u32 = 392;
/// Por debajo de este ancho de ventana no cabe como columna y se ensena a
/// pantalla completa, tapando la conversacion.
pub const ANCHO_TRES_COLUMNAS: u32 = 932;
/// La cabecera del panel, con su boton de volver.
pub const CABECERA: u32 = 54;
pub const VOLVER_ANCHO: u32 = 60;
pub const TITULO_TAM: f32 = 14.0;

/// Como CAPA FLOTANTE: un recuadro redondeado centrado sobre la
/// conversacion, con velo detras. Es lo que se ve en Telegram cuando la
/// ventana no da para tres columnas, y es la forma que se copia aqui.
pub const CAPA_ANCHO_MINIMO: u32 = 324;
pub const CAPA_ANCHO_DESEADO: u32 = 392;
/// Lo que se le deja a cada lado. Si la ventana no da ni para el minimo mas
/// estos margenes, la capa pasa a pantalla completa.
pub const CAPA_MARGEN: u32 = 48;
/// Arriba y abajo el margen no es fijo: es la veinticuatroava parte del alto,
/// sujeta entre estos dos. En una ventana baja la capa casi la llena; en una
/// alta no se estira hasta parecer una columna.
pub const CAPA_MARGEN_ARRIBA_MINIMO: u32 = 20;
pub const CAPA_MARGEN_ARRIBA_MAXIMO: u32 = 40;
pub const CAPA_RADIO: u32 = 8;
/// La cabecera de la capa es un poco mas alta que la de la columna.
pub const CAPA_CABECERA: u32 = 56;
/// El titulo y su subtitulo, cuando lleva los dos.
pub const CAPA_TITULO_X: u32 = 16;
pub const CAPA_TITULO_Y: u32 = 8;
pub const CAPA_SUBTITULO_Y: u32 = 28;
pub const CAPA_TITULO_TAM: f32 = 14.0;
pub const CAPA_SUBTITULO_TAM: f32 = 13.0;
/// Los dos botones de la derecha. El aspa es de 48 y la lupa de 56: no es un
/// descuido, es lo que mide cada uno en Telegram.
pub const CAPA_BOTON_CERRAR: u32 = 48;
pub const CAPA_BOTON_BUSCAR: u32 = 56;
/// Lo alta que es la caja de buscar dentro de la cabecera, y la letra que
/// lleva. La misma que el buscador de la lista de proyectos: es la misma
/// clase de caja y desigualarlas solo se notaria para mal.
pub const BUSCAR_ALTO: u32 = 32;
pub const BUSCAR_TAM: f32 = 13.0;
pub const BUSCAR_TEXTO_X: u32 = 12;

/// La ficha de arriba: avatar grande, nombre y estado.
pub const FICHA_ALTO: u32 = 108;
pub const FICHA_AVATAR: u32 = 72;
pub const FICHA_AVATAR_X: u32 = 19;
pub const FICHA_AVATAR_Y: u32 = 18;
pub const FICHA_TEXTO_X: u32 = 109;
pub const FICHA_NOMBRE_Y: u32 = 32;
pub const FICHA_ESTADO_Y: u32 = 58;
pub const FICHA_NOMBRE_TAM: f32 = 16.0;
pub const FICHA_ESTADO_TAM: f32 = 13.0;

/// La tira de pestanas: 48 de alto, con una «isla» de 36 dentro.
pub const TIRA_ALTO: u32 = 48;
pub const TIRA_MARGEN_X: u32 = 8;
pub const TIRA_MARGEN_Y: u32 = 6;
/// Lo que se separa la pildora del borde de la isla.
pub const PILDORA_HUECO: u32 = 4;
/// Relleno a cada lado del texto de una pestana.
pub const PESTANA_RELLENO: u32 = 16;
pub const PESTANA_TAM: f32 = 13.0;
/// Lo que tarda la pildora en deslizarse de una pestana a otra.
pub const PILDORA_MS: u32 = 200;

/// La cuadricula de fotos.
pub const CELDA_MINIMA: u32 = 82;
pub const CELDA_HUECO: u32 = 2;
pub const REJILLA_MARGEN: u32 = 3;

/// Las filas de archivo: miniatura de 70 con 3 de aire arriba y abajo.
pub const ARCHIVO_ALTO: u32 = 77;
pub const ARCHIVO_MINIATURA: u32 = 70;
pub const ARCHIVO_MARGEN_X: u32 = 14;
pub const ARCHIVO_NOMBRE_Y: u32 = 7;
pub const ARCHIVO_ESTADO_Y: u32 = 24;
pub const ARCHIVO_FECHA_Y: u32 = 49;
pub const ARCHIVO_NOMBRE_TAM: f32 = 13.0;
pub const ARCHIVO_ESTADO_TAM: f32 = 13.0;

/// Como queda repartido el panel.
///
/// Que secciones hay y como se llaman vive en `pixpin_proyecto::cuaderno`:
/// son del dominio, no de la interfaz. Aqui solo se reparten rectangulos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposicion {
    /// Todo el panel.
    pub panel: Rect,
    pub cabecera: Rect,
    /// El boton de volver, dentro de la cabecera.
    pub volver: Rect,
    pub ficha: Rect,
    /// La tira de pestanas entera.
    pub tira: Rect,
    /// La «isla» de dentro de la tira, que es lo que lleva fondo. Va CENTRADA
    /// y solo tan ancha como sus pestanas: una isla a todo lo ancho con las
    /// pestanas apretadas a la izquierda se ve distinta.
    pub isla: Rect,
    /// Lo que queda debajo para el contenido.
    pub contenido: Rect,
    /// El panel ocupa la ventana entera porque no cabia como columna.
    pub pantalla_completa: bool,
}

/// El ancho que le toca al panel en una ventana de `ancho_ventana`.
pub fn ancho_panel(ancho_ventana: u32, escala_por_cien: u32) -> u32 {
    let e = |v: u32| v * escala_por_cien / 100;
    if ancho_ventana < e(ANCHO_TRES_COLUMNAS) {
        // No cabe de columna: se lo queda todo. Mejor eso que una columna de
        // cien pixeles donde no se lee nada.
        return ancho_ventana;
    }
    e(ANCHO_MINIMO).max(e(ANCHO_MAXIMO).min(ancho_ventana / 3))
}

/// Donde cae la capa flotante dentro de la ventana.
///
/// Se centra, con su ancho deseado si cabe y sus margenes a los lados. En una
/// ventana pequena no cabe ni el minimo con margenes: entonces se lo queda
/// todo, que es lo que hace Telegram en vez de encoger hasta lo ilegible.
pub fn capa_en(ventana: Rect, escala_por_cien: u32) -> (Rect, bool) {
    let e = |v: u32| v * escala_por_cien / 100;
    let minimo_con_margenes = e(CAPA_ANCHO_MINIMO) + 2 * e(CAPA_MARGEN);
    if ventana.ancho < minimo_con_margenes {
        return (ventana, true);
    }
    let ancho = e(CAPA_ANCHO_DESEADO).min(ventana.ancho - 2 * e(CAPA_MARGEN));
    // Alta pero no pegada a los bordes: se le deja el mismo aire arriba y
    // abajo que a los lados.
    // Arriba y abajo, la veinticuatroava parte del alto, sujeta entre 20 y 40.
    let margen =
        (ventana.alto / 24).clamp(e(CAPA_MARGEN_ARRIBA_MINIMO), e(CAPA_MARGEN_ARRIBA_MAXIMO));
    let alto = ventana.alto.saturating_sub(2 * margen).max(1);
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

impl Disposicion {
    /// Reparte el panel como CAPA: cabecera con titulo y subtitulo, dos
    /// botones a la derecha, la isla de pestanas y el contenido.
    ///
    /// La diferencia con la columna no es solo el sitio: aqui no hay flecha
    /// de volver a la izquierda —se cierra con el aspa— y la cabecera es un
    /// poco mas alta porque lleva dos lineas.
    pub fn capa(caja: Rect, escala_por_cien: u32) -> Disposicion {
        let e = |v: u32| v * escala_por_cien / 100;
        let cabecera = Rect {
            alto: e(CAPA_CABECERA).min(caja.alto),
            ..caja
        };
        let tira = Rect {
            x: caja.x,
            y: cabecera.abajo(),
            ancho: caja.ancho,
            alto: e(TIRA_ALTO).min(caja.alto.saturating_sub(cabecera.alto)),
        };
        let isla = Rect {
            x: tira.x + e(TIRA_MARGEN_X) as i32,
            y: tira.y + e(TIRA_MARGEN_Y) as i32,
            ancho: tira.ancho.saturating_sub(2 * e(TIRA_MARGEN_X)),
            alto: tira.alto.saturating_sub(2 * e(TIRA_MARGEN_Y)),
        };
        let arriba = tira.abajo();
        Disposicion {
            panel: caja,
            cabecera,
            // En la capa el aspa hace de «volver»: cierra.
            volver: Rect {
                x: cabecera.derecha() - e(CAPA_BOTON_CERRAR) as i32,
                y: cabecera.y,
                ancho: e(CAPA_BOTON_CERRAR).min(cabecera.ancho),
                alto: cabecera.alto,
            },
            // La capa no tiene ficha: el nombre ya esta en la cabecera.
            ficha: Rect {
                x: caja.x,
                y: cabecera.abajo(),
                ancho: caja.ancho,
                alto: 0,
            },
            tira,
            isla,
            contenido: Rect {
                x: caja.x,
                y: arriba,
                ancho: caja.ancho,
                alto: (caja.abajo() - arriba).max(0) as u32,
            },
            pantalla_completa: false,
        }
    }

    /// El boton de la lupa, a la izquierda del aspa.
    pub fn buscar(&self, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        Rect {
            x: self.volver.x - e(CAPA_BOTON_BUSCAR) as i32,
            y: self.cabecera.y,
            ancho: e(CAPA_BOTON_BUSCAR),
            alto: self.cabecera.alto,
        }
    }

    /// La caja de escribir lo que se busca, cuando la lupa esta encendida.
    ///
    /// Ocupa el sitio del titulo, entre el margen izquierdo y la lupa: es lo
    /// que hace Telegram, y tiene sentido porque mientras se busca el titulo
    /// no dice nada que no se sepa ya.
    pub fn caja_buscar(&self, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let margen = e(CAPA_TITULO_X) as i32;
        let x = self.cabecera.x + margen;
        let derecha = self.buscar(escala_por_cien).x - margen;
        let alto = e(BUSCAR_ALTO).min(self.cabecera.alto);
        Rect {
            x,
            y: self.cabecera.y + (self.cabecera.alto as i32 - alto as i32) / 2,
            ancho: (derecha - x).max(0) as u32,
            alto,
        }
    }

    /// Reparte el panel dentro de `hueco`, que es la columna de la derecha o
    /// la ventana entera.
    pub fn calcular(hueco: Rect, pantalla_completa: bool, escala_por_cien: u32) -> Disposicion {
        let e = |v: u32| v * escala_por_cien / 100;
        let cabecera = Rect {
            alto: e(CABECERA).min(hueco.alto),
            ..hueco
        };
        let ficha = Rect {
            x: hueco.x,
            y: cabecera.abajo(),
            ancho: hueco.ancho,
            alto: e(FICHA_ALTO).min(hueco.alto.saturating_sub(cabecera.alto)),
        };
        let tira = Rect {
            x: hueco.x,
            y: ficha.abajo(),
            ancho: hueco.ancho,
            alto: e(TIRA_ALTO).min(hueco.alto.saturating_sub(cabecera.alto + ficha.alto)),
        };
        let isla = Rect {
            x: tira.x + e(TIRA_MARGEN_X) as i32,
            y: tira.y + e(TIRA_MARGEN_Y) as i32,
            ancho: tira.ancho.saturating_sub(2 * e(TIRA_MARGEN_X)),
            alto: tira.alto.saturating_sub(2 * e(TIRA_MARGEN_Y)),
        };
        let arriba = tira.abajo();
        Disposicion {
            panel: hueco,
            cabecera,
            volver: Rect {
                x: cabecera.x,
                y: cabecera.y,
                ancho: e(VOLVER_ANCHO).min(cabecera.ancho),
                alto: cabecera.alto,
            },
            ficha,
            tira,
            isla,
            contenido: Rect {
                x: hueco.x,
                y: arriba,
                ancho: hueco.ancho,
                alto: (hueco.abajo() - arriba).max(0) as u32,
            },
            pantalla_completa,
        }
    }

    /// Donde cae cada pestana dentro de la isla, en el mismo orden que se le
    /// pasan los anchos de texto (que los mide quien pinta).
    ///
    /// Si no caben todas, se reparte el hueco a partes iguales: es preferible
    /// apretarlas a que la ultima se salga.
    pub fn pestanas(&self, anchos_de_texto: &[f32], escala_por_cien: u32) -> Vec<Rect> {
        let e = |v: u32| v * escala_por_cien / 100;
        if anchos_de_texto.is_empty() || self.isla.ancho == 0 {
            return Vec::new();
        }
        let relleno = 2.0 * e(PESTANA_RELLENO) as f32;
        let naturales: Vec<f32> = anchos_de_texto.iter().map(|w| w + relleno).collect();
        let total: f32 = naturales.iter().sum();
        let cabe = total <= self.isla.ancho as f32;

        let mut x = self.isla.x as f32;
        naturales
            .iter()
            .map(|natural| {
                let ancho = if cabe {
                    *natural
                } else {
                    self.isla.ancho as f32 / naturales.len() as f32
                };
                let r = Rect {
                    x: x.round() as i32,
                    y: self.isla.y,
                    ancho: ancho.round().max(0.0) as u32,
                    alto: self.isla.alto,
                };
                x += ancho;
                r
            })
            .collect()
    }

    /// La pildora de la pestana activa: su rectangulo, ya encogido.
    /// La pildora de la pestana activa: su rectangulo, encogido por los
    /// CUATRO lados, no solo arriba y abajo. Asi queda del ancho del texto mas
    /// su relleno menos los dos retranqueos, que es como se ve en Telegram.
    pub fn pildora(&self, pestana: Rect, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let hueco = e(PILDORA_HUECO);
        Rect {
            x: pestana.x + hueco as i32,
            y: pestana.y + hueco as i32,
            ancho: pestana.ancho.saturating_sub(2 * hueco),
            alto: pestana.alto.saturating_sub(2 * hueco),
        }
    }

    /// Que pestana hay bajo el punto.
    pub fn pestana_en(&self, p: Punto, pestanas: &[Rect]) -> Option<usize> {
        pestanas.iter().position(|r| r.contiene(p))
    }
}

/// Como queda la cuadricula de fotos en un ancho dado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejilla {
    pub columnas: u32,
    /// El lado de una celda: son cuadradas.
    pub lado: u32,
    /// Donde empieza la primera columna, ya centrada.
    pub margen: u32,
    /// Lo que se deja entre dos celdas, ya escalado.
    pub hueco: u32,
}

/// Reparte el ancho en columnas cuadradas.
pub fn rejilla(ancho: u32, escala_por_cien: u32) -> Rejilla {
    rejilla_con(
        ancho,
        CELDA_MINIMA,
        CELDA_HUECO,
        REJILLA_MARGEN,
        escala_por_cien,
    )
}

/// Reparte el ancho en columnas cuadradas de al menos `minima` de lado, con
/// `hueco` entre ellas y `margen` a cada lado (logicos, sin escalar).
pub fn rejilla_con(
    ancho: u32,
    minima: u32,
    hueco: u32,
    margen: u32,
    escala_por_cien: u32,
) -> Rejilla {
    let e = |v: u32| v * escala_por_cien / 100;
    let hueco = e(hueco);
    let minima = e(minima).max(1);
    let util = ancho.saturating_sub(2 * e(margen));
    // Cuantas caben, contando que entre dos hay un hueco.
    let columnas = ((util + hueco) / (minima + hueco)).max(1);
    let lado = ((util + hueco) / columnas).saturating_sub(hueco).max(1);
    let ocupa = columnas * lado + (columnas - 1) * hueco;
    Rejilla {
        columnas,
        lado,
        margen: ancho.saturating_sub(ocupa) / 2,
        hueco,
    }
}

impl Rejilla {
    /// La celda numero `indice`, con el desplazamiento ya restado.
    pub fn celda(&self, indice: usize, area: Rect, scroll: i32, _escala_por_cien: u32) -> Rect {
        let hueco = self.hueco;
        let fila = indice as u32 / self.columnas;
        let columna = indice as u32 % self.columnas;
        Rect {
            x: area.x + self.margen as i32 + (columna * (self.lado + hueco)) as i32,
            y: area.y + (fila * (self.lado + hueco)) as i32 - scroll,
            ancho: self.lado,
            alto: self.lado,
        }
    }

    /// Lo que ocupa todo junto.
    pub fn alto_total(&self, cuantas: usize, _escala_por_cien: u32) -> u32 {
        if cuantas == 0 {
            return 0;
        }
        let hueco = self.hueco;
        let filas = cuantas.div_ceil(self.columnas as usize) as u32;
        filas * self.lado + (filas - 1) * hueco
    }

    /// Que celdas se ven, para pintar solo esas.
    pub fn visibles(
        &self,
        area: Rect,
        cuantas: usize,
        scroll: i32,
        _escala_por_cien: u32,
    ) -> (usize, usize) {
        let hueco = self.hueco;
        let paso = (self.lado + hueco).max(1) as i32;
        let primera_fila = (scroll / paso).max(0) as usize;
        let filas_que_caben = (area.alto as i32 / paso + 2) as usize;
        let primera = primera_fila * self.columnas as usize;
        let cuantas_ver = filas_que_caben * self.columnas as usize;
        (
            primera.min(cuantas),
            cuantas_ver.min(cuantas.saturating_sub(primera)),
        )
    }
}

/// La fila de un archivo, con su miniatura y sus tres lineas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilaArchivo {
    pub fila: Rect,
    pub miniatura: Rect,
    pub nombre: Punto,
    pub estado: Punto,
    pub fecha: Punto,
    pub ancho_texto: u32,
}

/// Coloca lo de dentro de una fila de archivo.
pub fn fila_archivo(fila: Rect, escala_por_cien: u32) -> FilaArchivo {
    let e = |v: u32| v * escala_por_cien / 100;
    let miniatura = e(ARCHIVO_MINIATURA);
    let x = fila.x + e(ARCHIVO_MARGEN_X) as i32;
    let texto_x = x + miniatura as i32 + e(ARCHIVO_MARGEN_X) as i32;
    let derecha = fila.derecha() - e(ARCHIVO_MARGEN_X) as i32;
    FilaArchivo {
        fila,
        miniatura: Rect {
            x,
            y: fila.y + e(3) as i32,
            ancho: miniatura,
            alto: miniatura,
        },
        nombre: Punto {
            x: texto_x,
            y: fila.y + e(ARCHIVO_NOMBRE_Y) as i32,
        },
        estado: Punto {
            x: texto_x,
            y: fila.y + e(ARCHIVO_ESTADO_Y) as i32,
        },
        fecha: Punto {
            x: texto_x,
            y: fila.y + e(ARCHIVO_FECHA_Y) as i32,
        },
        ancho_texto: (derecha - texto_x).max(0) as u32,
    }
}

// --- La isla de la ficha del chat (25-oct-2026) -----------------------------
//
// El usuario: «cuando hago click en la cabecera del chat me sale Galeria,
// Archivos, Voz… su ui es mala. Mira Telegram: es una isla flotante que
// muestra fotos, videos, archivos, enlaces… muy bien presentado, y hacer
// click dentro no debe salir de esa zona». Es la misma capa con velo, pero
// mas ancha y que se adapta a la ventana: la rejilla pone mas columnas de
// miniaturas cuanto mas sitio hay, y lo que se pulsa dentro se ve DENTRO
// (`Previa`), con su flecha de volver a la rejilla.

/// Lo estrecha y lo ancha que puede ser la isla.
pub const ISLA_ANCHO_MINIMO: u32 = 360;
pub const ISLA_ANCHO_MAXIMO: u32 = 720;
/// La parte del ancho de la ventana que se lleva (en por ciento).
pub const ISLA_FRACCION: u32 = 58;
/// Ni en un monitor altisimo se estira mas de esto.
pub const ISLA_ALTO_MAXIMO: u32 = 860;
/// Lo que se deja como minimo a los lados, y arriba y abajo.
pub const ISLA_MARGEN_X: u32 = 32;
pub const ISLA_MARGEN_Y: u32 = 28;
pub const ISLA_RADIO: u32 = 16;
/// La cabecera: el avatar del chat, su nombre y lo que tiene; la lupa y el
/// aspa a la derecha.
pub const ISLA_CABECERA: u32 = 68;
pub const ISLA_AVATAR: u32 = 42;
pub const ISLA_AVATAR_X: u32 = 16;
pub const ISLA_BOTON: u32 = 40;
pub const ISLA_TITULO_TAM: f32 = 15.0;
pub const ISLA_SUBTITULO_TAM: f32 = 12.5;
/// La tira de pestanas, un selector segmentado.
pub const ISLA_TIRA: u32 = 48;
pub const ISLA_TIRA_MARGEN_X: u32 = 12;
pub const ISLA_TIRA_MARGEN_Y: u32 = 6;
/// El relleno de cada pestana, y lo minimo que se le deja si no caben.
pub const ISLA_PESTANA_RELLENO: u32 = 14;
pub const ISLA_PESTANA_RELLENO_MINIMO: u32 = 6;
pub const ISLA_PESTANA_TAM: f32 = 13.0;
/// La rejilla: miniaturas cuadradas de al menos esto, con su hueco.
pub const ISLA_CELDA_MINIMA: u32 = 112;
pub const ISLA_CELDA_HUECO: u32 = 4;
pub const ISLA_MARGEN: u32 = 12;
/// Las filas de archivos, enlaces y voz.
pub const ISLA_FILA: u32 = 62;
pub const ISLA_FILA_MINIATURA: u32 = 44;
/// La previa de una cosa: las flechas de los lados y el pie con sus botones.
pub const ISLA_FLECHA: u32 = 40;
pub const ISLA_PIE: u32 = 64;
pub const ISLA_BOTON_PIE: u32 = 40;
pub const ISLA_BOTON_PIE_ANCHO: u32 = 150;

/// Donde cae la isla dentro de la ventana, y si se lo queda todo porque la
/// ventana no da ni para la mas estrecha con sus margenes.
pub fn isla_en(ventana: Rect, escala_por_cien: u32) -> (Rect, bool) {
    let e = |v: u32| v * escala_por_cien / 100;
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
/// repartir sus clics (ver `disposicion_info` en la ventana del chat: hubo
/// un tiempo en que eran dos, y los botones no respondian).
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
    /// El selector de pestanas (ya con su aire alrededor).
    pub tira: Rect,
    pub contenido: Rect,
    /// La ventana no daba para la isla y la ocupa entera.
    pub pantalla_completa: bool,
}

impl Isla {
    pub fn calcular(caja: Rect, pantalla_completa: bool, escala_por_cien: u32) -> Isla {
        let e = |v: u32| v * escala_por_cien / 100;
        let cabecera = Rect {
            alto: e(ISLA_CABECERA).min(caja.alto),
            ..caja
        };
        let medio = |lado: u32| cabecera.y + (cabecera.alto as i32 - lado as i32) / 2;
        let boton = e(ISLA_BOTON).min(cabecera.alto);
        let cerrar = Rect {
            x: cabecera.derecha() - e(10) as i32 - boton as i32,
            y: medio(boton),
            ancho: boton,
            alto: boton,
        };
        let buscar = Rect {
            x: cerrar.x - boton as i32,
            ..cerrar
        };
        let lado = e(ISLA_AVATAR).min(cabecera.alto);
        let avatar = Rect {
            x: cabecera.x + e(ISLA_AVATAR_X) as i32,
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
        let alto_caja = e(BUSCAR_ALTO).min(cabecera.alto);
        let caja_buscar = Rect {
            x: titulo.x,
            y: medio(alto_caja),
            ancho: titulo.ancho,
            alto: alto_caja,
        };
        let franja = e(ISLA_TIRA).min(caja.alto.saturating_sub(cabecera.alto));
        let tira = Rect {
            x: caja.x + e(ISLA_TIRA_MARGEN_X) as i32,
            y: cabecera.abajo() + e(ISLA_TIRA_MARGEN_Y) as i32,
            ancho: caja.ancho.saturating_sub(2 * e(ISLA_TIRA_MARGEN_X)),
            alto: franja.saturating_sub(2 * e(ISLA_TIRA_MARGEN_Y)),
        };
        let arriba = cabecera.abajo() + franja as i32;
        Isla {
            panel: caja,
            cabecera,
            avatar,
            titulo,
            buscar,
            cerrar,
            caja_buscar,
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

    /// Donde cae cada pestana en la tira, en el orden de los anchos de texto
    /// (que mide quien pinta). Se reparten centradas con su relleno; si no
    /// caben, primero se les quita relleno y, si aun asi no caben, se
    /// reparte la tira a partes iguales: nunca se sale ninguna.
    pub fn pestanas(&self, anchos_de_texto: &[f32], escala_por_cien: u32) -> Vec<Rect> {
        let e = |v: u32| (v * escala_por_cien / 100) as f32;
        let n = anchos_de_texto.len();
        if n == 0 || self.tira.ancho == 0 {
            return Vec::new();
        }
        let sitio = self.tira.ancho as f32;
        let texto: f32 = anchos_de_texto.iter().sum();
        let relleno = ((sitio - texto) / (2.0 * n as f32))
            .min(e(ISLA_PESTANA_RELLENO))
            .max(0.0);
        let anchos: Vec<f32> = if relleno >= e(ISLA_PESTANA_RELLENO_MINIMO) {
            anchos_de_texto.iter().map(|w| w + 2.0 * relleno).collect()
        } else {
            vec![sitio / n as f32; n]
        };
        let total: f32 = anchos.iter().sum();
        let mut x = self.tira.x as f32 + ((sitio - total) / 2.0).max(0.0);
        anchos
            .iter()
            .map(|ancho| {
                let r = Rect {
                    x: x.round() as i32,
                    y: self.tira.y,
                    ancho: ancho.round().max(0.0) as u32,
                    alto: self.tira.alto,
                };
                x += ancho;
                r
            })
            .collect()
    }

    /// El fondo del selector: solo tan ancho como sus pestanas.
    pub fn fondo_de_la_tira(&self, pestanas: &[Rect]) -> Rect {
        match (pestanas.first(), pestanas.last()) {
            (Some(a), Some(b)) => Rect {
                x: a.x,
                y: self.tira.y,
                ancho: (b.derecha() - a.x).max(0) as u32,
                alto: self.tira.alto,
            },
            _ => Rect {
                ancho: 0,
                ..self.tira
            },
        }
    }

    /// La pildora de la pestana activa, encogida por los cuatro lados.
    pub fn pildora(&self, pestana: Rect, escala_por_cien: u32) -> Rect {
        let hueco = (PILDORA_HUECO * escala_por_cien / 100) as i32;
        Rect {
            x: pestana.x + hueco,
            y: pestana.y + hueco,
            ancho: (pestana.ancho as i32 - 2 * hueco).max(0) as u32,
            alto: (pestana.alto as i32 - 2 * hueco).max(0) as u32,
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

/// La rejilla de miniaturas de la isla: columnas cuadradas que se adaptan al
/// ancho (mas sitio, mas columnas), centradas.
pub fn rejilla_isla(ancho: u32, escala_por_cien: u32) -> Rejilla {
    rejilla_con(
        ancho,
        ISLA_CELDA_MINIMA,
        ISLA_CELDA_HUECO,
        ISLA_MARGEN,
        escala_por_cien,
    )
}

/// Las filas (archivos, enlaces, voz) de la isla, una debajo de otra: la de
/// numero `n`, con el desplazamiento ya restado.
pub fn fila_isla(n: usize, area: Rect, scroll: i32, escala_por_cien: u32) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let alto = e(ISLA_FILA).max(1);
    Rect {
        x: area.x + e(ISLA_MARGEN / 2) as i32,
        y: area.y + e(ISLA_MARGEN / 2) as i32 + (n as u32 * alto) as i32 - scroll,
        ancho: area.ancho.saturating_sub(e(ISLA_MARGEN)),
        alto,
    }
}

/// Que filas se ven: `(primera, cuantas)`, con una de mas por arriba y por
/// abajo que asoma al desplazar.
pub fn filas_visibles(
    area: Rect,
    cuantas: usize,
    scroll: i32,
    escala_por_cien: u32,
) -> (usize, usize) {
    let alto = (ISLA_FILA * escala_por_cien / 100).max(1) as i32;
    let primera = (scroll / alto).max(0) as usize;
    let caben = (area.alto as i32 / alto + 2) as usize;
    (
        primera.min(cuantas),
        caben.min(cuantas.saturating_sub(primera)),
    )
}

/// Lo que miden todas las filas juntas, con su aire arriba y abajo.
pub fn alto_de_las_filas(cuantas: usize, escala_por_cien: u32) -> u32 {
    let e = |v: u32| v * escala_por_cien / 100;
    if cuantas == 0 {
        return 0;
    }
    cuantas as u32 * e(ISLA_FILA) + e(ISLA_MARGEN)
}

/// Lo de dentro de una fila de la isla: la miniatura redonda a la izquierda
/// y dos lineas de texto.
pub fn dentro_de_la_fila(fila: Rect, escala_por_cien: u32) -> FilaArchivo {
    let e = |v: u32| v * escala_por_cien / 100;
    let lado = e(ISLA_FILA_MINIATURA).min(fila.alto);
    let x = fila.x + e(10) as i32;
    let miniatura = Rect {
        x,
        y: fila.y + (fila.alto as i32 - lado as i32) / 2,
        ancho: lado,
        alto: lado,
    };
    let texto_x = miniatura.derecha() + e(12) as i32;
    let derecha = fila.derecha() - e(10) as i32;
    FilaArchivo {
        fila,
        miniatura,
        nombre: Punto {
            x: texto_x,
            y: fila.y + e(12) as i32,
        },
        estado: Punto {
            x: texto_x,
            y: fila.y + e(34) as i32,
        },
        fecha: Punto {
            x: texto_x,
            y: fila.y + e(34) as i32,
        },
        ancho_texto: (derecha - texto_x).max(0) as u32,
    }
}

/// La previa de una cosa dentro de la isla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Previa {
    /// Donde se ensena la cosa en grande (la foto, el dibujo, su ficha).
    pub vista: Rect,
    /// Las flechas de la anterior y la siguiente, a los lados.
    pub anterior: Rect,
    pub siguiente: Rect,
    /// El pie, con sus dos botones: abrir y verla en el chat.
    pub pie: Rect,
    pub abrir: Rect,
    pub en_el_chat: Rect,
}

/// Reparte el hueco de la previa (`Isla::hueco_de_la_previa`).
pub fn previa(hueco: Rect, escala_por_cien: u32) -> Previa {
    let e = |v: u32| v * escala_por_cien / 100;
    let alto_pie = e(ISLA_PIE).min(hueco.alto);
    let pie = Rect {
        y: hueco.abajo() - alto_pie as i32,
        alto: alto_pie,
        ..hueco
    };
    let margen = e(ISLA_MARGEN) as i32;
    let flecha = e(ISLA_FLECHA);
    let vista = Rect {
        x: hueco.x + margen + flecha as i32,
        y: hueco.y + margen,
        ancho: (hueco.ancho as i32 - 2 * (margen + flecha as i32)).max(0) as u32,
        alto: (pie.y - hueco.y - 2 * margen).max(0) as u32,
    };
    let medio_y = vista.y + (vista.alto as i32 - flecha as i32) / 2;
    let anterior = Rect {
        x: hueco.x + margen / 2,
        y: medio_y,
        ancho: flecha,
        alto: flecha,
    };
    let siguiente = Rect {
        x: hueco.derecha() - margen / 2 - flecha as i32,
        ..anterior
    };
    let alto_boton = e(ISLA_BOTON_PIE).min(alto_pie);
    let ancho_boton =
        e(ISLA_BOTON_PIE_ANCHO).min(hueco.ancho.saturating_sub(3 * margen as u32) / 2);
    let y_boton = pie.y + (alto_pie as i32 - alto_boton as i32) / 2;
    let juntos = 2 * ancho_boton as i32 + margen;
    let x0 = pie.x + (pie.ancho as i32 - juntos) / 2;
    Previa {
        vista,
        anterior,
        siguiente,
        pie,
        en_el_chat: Rect {
            x: x0,
            y: y_boton,
            ancho: ancho_boton,
            alto: alto_boton,
        },
        abrir: Rect {
            x: x0 + ancho_boton as i32 + margen,
            y: y_boton,
            ancho: ancho_boton,
            alto: alto_boton,
        },
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hueco() -> Rect {
        Rect {
            x: 700,
            y: 24,
            ancho: 392,
            alto: 700,
        }
    }

    #[test]
    fn el_panel_es_columna_en_una_ventana_ancha_y_lo_ocupa_todo_en_una_estrecha() {
        let ancho = ancho_panel(1400, 100);
        assert!((ANCHO_MINIMO..=ANCHO_MAXIMO).contains(&ancho), "{ancho}");
        // Justo en el umbral ya cabe como columna.
        assert!(ancho_panel(ANCHO_TRES_COLUMNAS, 100) < ANCHO_TRES_COLUMNAS);
        // Estrecha: no cabe de columna y se lo queda todo.
        assert_eq!(ancho_panel(800, 100), 800);
        // Y escala con el DPI.
        assert!(ancho_panel(2800, 200) >= ANCHO_MINIMO * 2);
    }

    #[test]
    fn las_piezas_del_panel_van_en_orden_y_sin_solaparse() {
        let d = Disposicion::calcular(hueco(), false, 100);
        assert_eq!(d.cabecera.y, hueco().y);
        assert_eq!(d.ficha.y, d.cabecera.abajo());
        assert_eq!(d.tira.y, d.ficha.abajo());
        assert_eq!(d.contenido.y, d.tira.abajo());
        assert_eq!(d.contenido.abajo(), hueco().abajo());
        assert!(d.contenido.alto > 0);
        // La isla vive dentro de la tira, con su aire.
        assert!(d.isla.y > d.tira.y);
        assert!(d.isla.abajo() < d.tira.abajo());
        assert_eq!(d.isla.alto, TIRA_ALTO - 2 * TIRA_MARGEN_Y);
    }

    #[test]
    fn las_pestanas_se_reparten_y_se_aprietan_si_no_caben() {
        let d = Disposicion::calcular(hueco(), false, 100);
        // Holgadas: cada una con su ancho natural.
        let anchos = [40.0, 50.0];
        let p = d.pestanas(&anchos, 100);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].ancho, 40 + 2 * PESTANA_RELLENO);
        assert_eq!(p[1].x, p[0].derecha(), "pegadas, sin hueco entre ellas");

        // Las siete de verdad no caben en 392: se reparten a partes iguales
        // en vez de dejar que la ultima se salga.
        let muchas: Vec<f32> = (0..7).map(|_| 60.0).collect();
        let p = d.pestanas(&muchas, 100);
        assert_eq!(p.len(), 7);
        assert!(p[6].derecha() <= d.isla.derecha() + 1, "{:?}", p[6]);
        assert!(p.windows(2).all(|w| w[0].ancho == w[1].ancho));
    }

    #[test]
    fn la_pildora_se_encoge_dentro_de_su_pestana() {
        let d = Disposicion::calcular(hueco(), false, 100);
        let p = d.pestanas(&[40.0], 100);
        let pildora = d.pildora(p[0], 100);
        // Encogida por los cuatro lados, no solo arriba y abajo.
        assert_eq!(pildora.ancho, p[0].ancho - 2 * PILDORA_HUECO);
        assert_eq!(pildora.alto, p[0].alto - 2 * PILDORA_HUECO);
        assert!(pildora.y > p[0].y && pildora.abajo() < p[0].abajo());
        assert!(pildora.x > p[0].x && pildora.derecha() < p[0].derecha());
        // Y se acierta donde se pulsa.
        let dentro = Punto {
            x: p[0].x + 5,
            y: p[0].y + 5,
        };
        assert_eq!(d.pestana_en(dentro, &p), Some(0));
        let fuera = Punto {
            x: p[0].x + 5,
            y: d.contenido.y + 5,
        };
        assert_eq!(d.pestana_en(fuera, &p), None);
    }

    #[test]
    fn la_cuadricula_reparte_el_sobrante_y_queda_centrada() {
        // Con el panel a 392 caben cuatro columnas.
        let r = rejilla(392, 100);
        assert_eq!(r.columnas, 4);
        assert!(
            r.lado >= CELDA_MINIMA,
            "ninguna celda por debajo del minimo"
        );
        // Lo que ocupa, mas los dos margenes, es el ancho entero (o uno menos
        // por el redondeo): no sobra un pico a la derecha.
        let ocupa = r.columnas * r.lado + (r.columnas - 1) * CELDA_HUECO;
        assert!(392 - (ocupa + 2 * r.margen) <= 1, "sobra demasiado");

        // En un panel estrechisimo queda una sola columna, no cero.
        let r = rejilla(60, 100);
        assert_eq!(r.columnas, 1);
        assert!(r.lado >= 1);
    }

    #[test]
    fn de_mil_fotos_solo_se_pintan_las_que_entran() {
        let d = Disposicion::calcular(hueco(), false, 100);
        let r = rejilla(d.contenido.ancho, 100);
        let (primera, cuantas) = r.visibles(d.contenido, 1000, 0, 100);
        assert_eq!(primera, 0);
        assert!(cuantas < 60, "demasiadas: {cuantas}");

        // Bajando, se empieza por otra fila entera.
        let paso = (r.lado + CELDA_HUECO) as i32;
        let (primera, _) = r.visibles(d.contenido, 1000, paso * 3, 100);
        assert_eq!(primera, 3 * r.columnas as usize, "filas enteras");

        // Las celdas de una fila van en linea y las de la siguiente, debajo.
        let a = r.celda(0, d.contenido, 0, 100);
        let b = r.celda(1, d.contenido, 0, 100);
        let c = r.celda(r.columnas as usize, d.contenido, 0, 100);
        assert_eq!(a.y, b.y);
        assert_eq!(b.x - a.x, (r.lado + CELDA_HUECO) as i32);
        assert_eq!(c.x, a.x);
        assert!(c.y > a.y);
        assert_eq!(a.ancho, a.alto, "cuadradas");
    }

    #[test]
    fn sin_fotos_la_cuadricula_no_ocupa_nada() {
        let r = rejilla(392, 100);
        assert_eq!(r.alto_total(0, 100), 0);
        let d = Disposicion::calcular(hueco(), false, 100);
        assert_eq!(r.visibles(d.contenido, 0, 0, 100), (0, 0));
        // Una sola foto ocupa una fila.
        assert_eq!(r.alto_total(1, 100), r.lado);
    }

    #[test]
    fn la_fila_de_un_archivo_tiene_sus_tres_lineas_en_su_sitio() {
        let fila = Rect {
            x: 700,
            y: 200,
            ancho: 392,
            alto: ARCHIVO_ALTO,
        };
        let f = fila_archivo(fila, 100);
        assert_eq!(f.miniatura.ancho, ARCHIVO_MINIATURA);
        assert_eq!(f.miniatura.alto, ARCHIVO_MINIATURA);
        assert!(f.miniatura.abajo() <= fila.abajo(), "cabe en su fila");
        // Las tres lineas, de arriba abajo y a la derecha de la miniatura.
        assert!(f.nombre.y < f.estado.y && f.estado.y < f.fecha.y);
        assert_eq!(f.nombre.x, f.estado.x);
        assert!(f.nombre.x >= f.miniatura.derecha());
        assert!(f.ancho_texto > 0);
    }

    fn ventana() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1024,
            alto: 768,
        }
    }

    #[test]
    fn la_capa_se_centra_y_deja_su_aire_a_los_lados() {
        let (caja, completa) = capa_en(ventana(), 100);
        assert!(!completa);
        assert_eq!(caja.ancho, CAPA_ANCHO_DESEADO);
        // Centrada: lo que sobra a la izquierda es lo que sobra a la derecha.
        assert_eq!(caja.x, ventana().derecha() - caja.derecha());
        assert_eq!(caja.y, ventana().abajo() - caja.abajo());
        assert!(caja.alto < ventana().alto, "no llega a los bordes");
    }

    #[test]
    fn en_una_ventana_pequena_la_capa_se_lo_queda_todo() {
        // Caso negativo: por debajo del minimo mas sus margenes no cabe, y
        // encoger hasta lo ilegible seria peor que taparlo todo.
        let pequena = Rect {
            x: 0,
            y: 0,
            ancho: CAPA_ANCHO_MINIMO + 2 * CAPA_MARGEN - 1,
            alto: 500,
        };
        let (caja, completa) = capa_en(pequena, 100);
        assert!(completa);
        assert_eq!(caja, pequena);
    }

    #[test]
    fn en_la_capa_el_aspa_va_a_la_derecha_y_la_lupa_a_su_lado() {
        let (caja, _) = capa_en(ventana(), 100);
        let d = Disposicion::capa(caja, 100);
        // El aspa pegada al borde derecho de la cabecera.
        assert_eq!(d.volver.derecha(), d.cabecera.derecha());
        // La lupa, justo a su izquierda y sin solaparse.
        let lupa = d.buscar(100);
        assert_eq!(lupa.derecha(), d.volver.x);
        assert!(lupa.x < d.volver.x);
        // La capa no tiene ficha: el nombre ya va en la cabecera.
        assert_eq!(d.ficha.alto, 0);
        // Y el contenido empieza justo bajo la tira.
        assert_eq!(d.contenido.y, d.tira.abajo());
        assert_eq!(d.contenido.abajo(), caja.abajo());
    }

    #[test]
    fn la_caja_de_buscar_ocupa_el_sitio_del_titulo_sin_pisar_la_lupa() {
        let (caja, _) = capa_en(ventana(), 100);
        let d = Disposicion::capa(caja, 100);
        let b = d.caja_buscar(100);
        assert_eq!(b.x, d.cabecera.x + CAPA_TITULO_X as i32, "el mismo margen");
        assert!(b.derecha() <= d.buscar(100).x, "no se mete bajo la lupa");
        assert!(b.ancho > 0);
        // Centrada en la cabecera y mas baja que ella: es una caja dentro,
        // no una franja que la sustituya.
        assert!(b.alto < d.cabecera.alto);
        assert_eq!(b.y - d.cabecera.y, d.cabecera.abajo() - b.abajo());
    }

    #[test]
    fn la_isla_crece_con_la_ventana_sin_pasarse_y_se_centra() {
        let (normal, completa) = isla_en(ventana(), 100);
        assert!(!completa);
        assert_eq!(normal.ancho, 1024 * ISLA_FRACCION / 100);
        let sobra = normal.x - (ventana().derecha() - normal.derecha());
        assert!(sobra.abs() <= 1, "centrada: {sobra}");
        assert_eq!(normal.y, ventana().abajo() - normal.abajo());
        // En un monitor enorme se queda en su maximo, ancho y alto.
        let enorme = Rect {
            x: 0,
            y: 0,
            ancho: 3000,
            alto: 2000,
        };
        let (grande, _) = isla_en(enorme, 100);
        assert_eq!(grande.ancho, ISLA_ANCHO_MAXIMO);
        assert_eq!(grande.alto, ISLA_ALTO_MAXIMO);
        // Y al 200 % mide el doble de minimo.
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
    fn las_piezas_de_la_isla_no_se_pisan() {
        let (caja, completa) = isla_en(ventana(), 100);
        let i = Isla::calcular(caja, completa, 100);
        assert_eq!(i.cerrar.derecha() + 10, i.cabecera.derecha());
        assert_eq!(i.buscar.derecha(), i.cerrar.x, "la lupa junto al aspa");
        assert!(i.avatar.derecha() < i.titulo.x);
        assert!(i.titulo.derecha() < i.buscar.x, "el nombre no pisa la lupa");
        assert_eq!(i.caja_buscar.x, i.titulo.x);
        assert!(i.tira.y > i.cabecera.abajo() - 1 && i.tira.abajo() < i.contenido.y + 1);
        assert_eq!(i.contenido.abajo(), caja.abajo());
        // La previa usa la tira y el contenido juntos.
        let h = i.hueco_de_la_previa();
        assert_eq!((h.y, h.abajo()), (i.cabecera.abajo(), caja.abajo()));
    }

    #[test]
    fn las_pestanas_de_la_isla_se_centran_y_nunca_se_salen() {
        let (caja, completa) = isla_en(ventana(), 100);
        let i = Isla::calcular(caja, completa, 100);
        let pocas = i.pestanas(&[40.0, 60.0], 100);
        assert_eq!(pocas[0].ancho, 40 + 2 * ISLA_PESTANA_RELLENO);
        assert_eq!(pocas[1].x, pocas[0].derecha(), "pegadas");
        // Centradas en la tira.
        let fondo = i.fondo_de_la_tira(&pocas);
        let izq = fondo.x - i.tira.x;
        let der = i.tira.derecha() - fondo.derecha();
        assert!((izq - der).abs() <= 1, "{izq} {der}");
        // Muchas: menos relleno, y si aun asi no caben, a partes iguales.
        let muchas = i.pestanas(&[70.0; 10], 100);
        assert_eq!(muchas.len(), 10);
        assert!(muchas[9].derecha() <= i.tira.derecha() + 1);
        assert!(muchas[0].x >= i.tira.x);
        // La pildora, dentro de su pestana.
        let p = i.pildora(pocas[0], 100);
        assert!(p.x > pocas[0].x && p.derecha() < pocas[0].derecha());
        // Caso negativo: sin pestanas no hay fondo.
        assert_eq!(i.fondo_de_la_tira(&[]).ancho, 0);
    }

    #[test]
    fn la_rejilla_de_la_isla_pone_mas_columnas_cuanto_mas_ancha() {
        let estrecha = rejilla_isla(380, 100);
        let ancha = rejilla_isla(720, 100);
        assert!(ancha.columnas > estrecha.columnas);
        for r in [estrecha, ancha] {
            assert!(r.lado >= ISLA_CELDA_MINIMA);
            assert_eq!(r.hueco, ISLA_CELDA_HUECO);
        }
        // Las celdas, con su hueco de la isla y no el del panel viejo.
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 720,
            alto: 500,
        };
        let a = ancha.celda(0, area, 0, 100);
        let b = ancha.celda(1, area, 0, 100);
        assert_eq!(b.x - a.derecha(), ISLA_CELDA_HUECO as i32);
        // La de la fila siguiente, debajo y en la primera columna.
        let c = ancha.celda(ancha.columnas as usize, area, 0, 100);
        assert_eq!((c.x, c.y), (a.x, a.abajo() + ISLA_CELDA_HUECO as i32));
    }

    #[test]
    fn las_filas_de_la_isla_van_una_bajo_otra_y_solo_se_miran_las_que_se_ven() {
        let area = Rect {
            x: 100,
            y: 200,
            ancho: 500,
            alto: 310,
        };
        let a = fila_isla(0, area, 0, 100);
        let b = fila_isla(1, area, 0, 100);
        assert_eq!(b.y, a.abajo());
        assert!(a.x > area.x && a.derecha() < area.derecha());
        let (primera, cuantas) = filas_visibles(area, 100, 0, 100);
        assert_eq!(primera, 0);
        assert_eq!(cuantas, 310 / ISLA_FILA as usize + 2);
        let (primera, _) = filas_visibles(area, 100, 3 * ISLA_FILA as i32, 100);
        assert_eq!(primera, 3);
        // Caso negativo: no se pasa de las que hay.
        assert_eq!(filas_visibles(area, 2, 0, 100), (0, 2));
        assert_eq!(alto_de_las_filas(0, 100), 0);
        let f = dentro_de_la_fila(a, 100);
        assert!(f.miniatura.x > a.x && f.miniatura.abajo() <= a.abajo());
        assert!(f.nombre.x > f.miniatura.derecha() && f.estado.y > f.nombre.y);
    }

    #[test]
    fn la_previa_deja_las_flechas_a_los_lados_y_los_botones_abajo() {
        let (caja, completa) = isla_en(ventana(), 100);
        let i = Isla::calcular(caja, completa, 100);
        let h = i.hueco_de_la_previa();
        let p = previa(h, 100);
        assert!(p.anterior.derecha() <= p.vista.x);
        assert!(p.siguiente.x >= p.vista.derecha());
        assert!(p.vista.abajo() <= p.pie.y);
        assert_eq!(p.pie.abajo(), h.abajo());
        assert!(p.en_el_chat.derecha() < p.abrir.x, "sin pisarse");
        assert!(p.abrir.abajo() <= p.pie.abajo() && p.abrir.y >= p.pie.y);
        // Centrados: lo que sobra a cada lado del par es lo mismo.
        let izq = p.en_el_chat.x - p.pie.x;
        let der = p.pie.derecha() - p.abrir.derecha();
        assert!((izq - der).abs() <= 1);
    }

    #[test]
    fn encajar_no_recorta_ni_deforma() {
        let caja = Rect {
            x: 10,
            y: 20,
            ancho: 400,
            alto: 300,
        };
        // Apaisada: llena el ancho y sobra arriba y abajo por igual.
        let r = encajar(800, 400, caja);
        assert_eq!((r.ancho, r.alto), (400, 200));
        assert_eq!(r.y - caja.y, caja.abajo() - r.abajo());
        // Vertical: llena el alto.
        let r = encajar(100, 600, caja);
        assert_eq!(r.alto, 300);
        assert_eq!(r.ancho, 50);
        // Caso negativo: una imagen sin tamano no ocupa nada.
        assert_eq!(encajar(0, 10, caja).ancho, 0);
    }

    #[test]
    fn en_una_cabecera_estrechisima_la_caja_de_buscar_se_queda_en_nada() {
        // Caso negativo: si no cabe, ancho cero — nunca un rectangulo del
        // reves, que al pintarlo se saldria por la izquierda.
        let estrecha = Rect {
            x: 0,
            y: 0,
            ancho: 60,
            alto: 200,
        };
        let d = Disposicion::capa(estrecha, 100);
        assert_eq!(d.caja_buscar(100).ancho, 0);
    }
}
