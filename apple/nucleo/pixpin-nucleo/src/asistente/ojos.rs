//! **Los ojos del asistente: el lienzo pintado para el modelo** (E1; el
//! `asistente/ojos.rs` de Windows).
//!
//! Lo pide `ver_lienzo` y la revision del turno (`pixpin_ia::ojos`). Quien
//! tiene la escena (un lienzo abierto o uno leido del disco) solo **copia**
//! lo que cae en la zona ([`copiar`]); aqui se empaqueta como el
//! [`LienzoPlano`] de siempre, con la rejilla numerada en las coordenadas del
//! modelo y la etiqueta `#id` de cada elemento ([`para_pintar`]), y lo pinta
//! Swift con **el mismo pintor del lienzo** (`PintorLienzo`, lo que ve el
//! usuario en el editor) en un bitmap sin ventana ([`super::turno::AppIa::pintar_vista`]).

use pixpin_ia::edicion::Contexto;
use pixpin_ia::ojos::{self, Mirada, Vista};
use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::Figura;
use pixpin_motor2d::pintado;

use crate::lienzo::{ImagenDelLienzo, LienzoPlano, empaquetar};

/// **Una raya de la rejilla**, en pixeles de la imagen.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct RayaVista {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// **Un rotulo**: un numero de la rejilla o la etiqueta `#id` de un
/// elemento (`etiqueta`, sobre amarillo), con su esquina de arriba a la
/// izquierda en pixeles de la imagen.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RotuloVista {
    pub texto: String,
    pub x: f32,
    pub y: f32,
    pub etiqueta: bool,
}

/// **Lo que Swift pinta para el modelo**: la zona `(x0, y0)–(x1, y1)` del
/// mundo en una imagen de `ancho` x `alto` (`escala` pixeles por unidad),
/// con el papel y lo dibujado (`plano`, sus fotos en `imagenes`) y encima la
/// rejilla y las etiquetas.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct VistaParaPintar {
    pub plano: LienzoPlano,
    pub imagenes: Vec<ImagenDelLienzo>,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub ancho: u32,
    pub alto: u32,
    pub escala: f32,
    pub rayas: Vec<RayaVista>,
    pub rotulos: Vec<RotuloVista>,
}

/// **Lo que hay que pintar**, ya copiado de quien tiene la escena.
#[derive(Debug, Clone)]
pub struct Encargo {
    /// Solo lo que cae en la zona, con sus ids de siempre.
    pub escena: Escena,
    /// Las fotos que usa, con su ruta absoluta.
    pub imagenes: Vec<ImagenDelLienzo>,
    /// Lo que se mira, en coordenadas de la escena.
    pub caja: (f32, f32, f32, f32),
    /// Desde donde mide el modelo (para numerar la rejilla).
    pub marco: (f32, f32),
    pub marcas: bool,
}

/// **Copia lo que hace falta para pintar una mirada**: los elementos que
/// caen en su caja (con un poco de holgura por los trazos) y las fotos que
/// usan (`ruta(id)` da la de cada una).
pub fn encargo_de(escena: &Escena, m: &Mirada, ctx: &Contexto, ruta: impl Fn(u64) -> Option<String>) -> Result<Encargo, String> {
    let caja = ojos::caja_de(escena, m, ctx)?;
    Ok(copiar(escena, caja, ctx.marco, m.marcas, ruta))
}

/// La copia de la zona `caja` de una escena.
pub fn copiar(escena: &Escena, caja: (f32, f32, f32, f32), marco: (f32, f32), marcas: bool, ruta: impl Fn(u64) -> Option<String>) -> Encargo {
    let mut copia = Escena::nueva();
    copia.fondo = escena.fondo;
    copia.escala = escena.escala.clone();
    copia.referencias_visibles = escena.referencias_visibles;
    copia.siguiente_id = escena.siguiente_id;
    let mut imagenes: Vec<ImagenDelLienzo> = Vec::new();
    for e in escena.a_la_vista() {
        let (x0, y0, x1, y1) = e.caja();
        let h = e.grosor * 2.0 + 4.0;
        if x1 + h < caja.0 || x0 - h > caja.2 || y1 + h < caja.1 || y0 - h > caja.3 {
            continue;
        }
        if let Figura::Imagen { id_objeto } = e.figura
            && !imagenes.iter().any(|i| i.id_objeto == id_objeto)
            && let Some(r) = ruta(id_objeto)
        {
            imagenes.push(ImagenDelLienzo { id_objeto, ruta: r });
        }
        copia.elementos.push(e.clone());
    }
    Encargo {
        escena: copia,
        imagenes,
        caja,
        marco,
        marcas,
    }
}

/// **Lo que se le da a Swift para pintar**: el plano de la copia y, si
/// `marcas`, la rejilla cada [`ojos::paso_de_rejilla`] (numerada en las
/// coordenadas del modelo) y las etiquetas `#id` encima de la esquina de
/// cada elemento, sin salirse de la imagen (`pintar_marcas` de Windows).
pub fn para_pintar(enc: Encargo) -> VistaParaPintar {
    let (x0, y0, x1, y1) = enc.caja;
    let (ancho, alto, s) = ojos::tamano(enc.caja, ojos::LADO_MAXIMO);
    let (w, h) = (ancho as f32, alto as f32);
    let mut rayas = Vec::new();
    let mut rotulos = Vec::new();
    if enc.marcas {
        let paso = ojos::paso_de_rejilla(enc.caja);
        let (mx, my) = enc.marco;
        let mut u = ((x0 - mx) / paso).ceil() * paso;
        while u + mx <= x1 {
            let px = (u + mx - x0) * s;
            rayas.push(RayaVista { x0: px, y0: 0.0, x1: px, y1: h });
            rotulos.push(RotuloVista {
                texto: format!("{}", u.round() as i64),
                x: px + 3.0,
                y: 3.0,
                etiqueta: false,
            });
            u += paso;
        }
        let mut v = ((y0 - my) / paso).ceil() * paso;
        while v + my <= y1 {
            let py = (v + my - y0) * s;
            rayas.push(RayaVista { x0: 0.0, y0: py, x1: w, y1: py });
            if py > 18.0 {
                rotulos.push(RotuloVista {
                    texto: format!("{}", v.round() as i64),
                    x: 3.0,
                    y: py + 2.0,
                    etiqueta: false,
                });
            }
            v += paso;
        }
        // Las etiquetas: unos 7 px por letra y 15 de alto a 11 pt.
        for (id, (ex, ey)) in ojos::etiquetas(&enc.escena, enc.caja) {
            let texto = format!("#{id}");
            let tw = 7.0 * texto.chars().count() as f32;
            let th = 15.0;
            let px = ((ex - x0) * s).clamp(2.0, (w - tw - 6.0).max(2.0));
            let py = ((ey - y0) * s - th - 3.0).clamp(2.0, (h - th - 4.0).max(2.0));
            rotulos.push(RotuloVista {
                texto,
                x: px + 3.0,
                y: py + 1.0,
                etiqueta: true,
            });
        }
    }
    let ordenes = pintado::ordenes_de_escena(&enc.escena);
    VistaParaPintar {
        plano: empaquetar(&ordenes, enc.escena.fondo, enc.escena.caja()),
        imagenes: enc.imagenes,
        x0,
        y0,
        x1,
        y1,
        ancho,
        alto,
        escala: s,
        rayas,
        rotulos,
    }
}

/// **La vista para el modelo**: lo que pinto Swift (`png`) con su leyenda.
pub fn vista(v: &VistaParaPintar, marco: (f32, f32), marcas: bool, png: Vec<u8>, ms: u128) -> Vista {
    let texto = ojos::leyenda((v.x0, v.y0, v.x1, v.y1), marco, v.ancho, v.alto, v.escala, marcas);
    Vista {
        png,
        ancho: v.ancho,
        alto: v.alto,
        texto,
        ms,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn escena_con(n: usize) -> Escena {
        let doc = pixpin_ia::lienzo::documento(
            (0..n)
                .map(|i| {
                    serde_json::json!({"id": format!("e{i}"), "type": "rectangle", "x": i as f64 * 300.0, "y": 0.0,
                        "width": 100.0, "height": 80.0, "strokeColor": "#1e1e1e", "version": 1, "isDeleted": false})
                })
                .collect(),
        );
        pixpin_motor2d::excalidraw::a_escena(&pixpin_motor2d::excalidraw::leer(&doc).unwrap())
    }

    #[test]
    fn la_vista_lleva_lo_de_la_zona_la_rejilla_y_las_etiquetas() {
        let escena = escena_con(3);
        let ctx = Contexto::default();
        // Todo: los tres, con margen.
        let enc = encargo_de(&escena, &Mirada::default(), &ctx, |_| None).unwrap();
        assert_eq!(enc.escena.elementos.len(), 3);
        let v = para_pintar(enc);
        assert!(v.ancho <= ojos::LADO_MAXIMO && v.alto >= 16);
        assert!(!v.rayas.is_empty());
        let etiquetas: Vec<&RotuloVista> = v.rotulos.iter().filter(|r| r.etiqueta).collect();
        assert_eq!(etiquetas.len(), 3);
        assert!(etiquetas.iter().all(|r| r.texto.starts_with('#') && r.x >= 2.0 && r.y >= 2.0));
        assert!(!v.plano.datos.is_empty());
        let vista = vista(&v, (0.0, 0.0), true, vec![1, 2, 3], 5);
        assert!(vista.texto.starts_with(ojos::MARCA), "{}", vista.texto);
        // Una caja que solo toca el primero: solo el.
        let m = Mirada::de(pixpin_ia::ojos::Zona::Caja([0.0, 0.0, 120.0, 90.0]));
        let enc = encargo_de(&escena, &m, &ctx, |_| None).unwrap();
        assert_eq!(enc.escena.elementos.len(), 1);
        // Sin marcas, ni rejilla ni etiquetas.
        let sin = para_pintar(encargo_de(&escena, &Mirada { marcas: false, ..Default::default() }, &ctx, |_| None).unwrap());
        assert!(sin.rayas.is_empty() && sin.rotulos.is_empty());
        // Un lienzo vacio no se mira (caso negativo).
        assert!(encargo_de(&Escena::nueva(), &Mirada::default(), &ctx, |_| None).is_err());
    }
}
