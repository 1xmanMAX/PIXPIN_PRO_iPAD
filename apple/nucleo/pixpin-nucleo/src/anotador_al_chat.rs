//! **Lo anotado sobre la pantalla, a «Mensajes guardados»** (E3), como
//! `anotador_al_chat.rs` de Windows.
//!
//! No es un formato nuevo: es **una foto del chat con su lienzo**, lo mismo
//! que deja el movil al anotar una foto (`pixpin_proyecto::lienzo_de_la_foto`):
//!
//! 1. la foto de lo que habia DEBAJO (sin la tinta ni las barras) entra en
//!    «Mensajes guardados» como un mensaje IMAGEN corriente
//!    (`Nucleo::adjuntar_con`, con su cerrojo);
//! 2. se le crea su lienzo `foto-<id>` con la foto bloqueada como primer
//!    elemento y la tinta encima, suelta y editable
//!    (`lienzo_de_la_foto::con_dibujo`), y se apunta la `referencia`.
//!
//! La burbuja la ensena con la tinta (`foto_anotada`), al pulsarla se abre
//! ese lienzo y se sigue editando, y viaja al movil como cualquier foto
//! anotada alli.
//!
//! Lo que cambia con el Mac: la tinta del anotador vive en PUNTOS del
//! escritorio (Swift pone la camara quieta a 1:1 en cada pantalla) y la foto
//! en pixeles; quien llama dice donde empieza la foto en el escritorio y
//! cuantos pixeles hay por punto, y aqui se lleva la tinta a pixeles de la
//! foto ([`a_pixeles`]) antes de darsela al lienzo.
//!
//! La regla de cuando guardar (nada, nuevo, al dia) es de Swift
//! (`AnotadorPantalla.swift`), con la [`huella`] de la tinta que se le da
//! aqui: comparar la tinta entera cruzando el puente en cada salida seria
//! mover megas para nada.

use std::path::Path;
use std::sync::Arc;

use pixpin_motor2d::{Elemento, Escena, Figura, Punto2};
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Mensaje};

use crate::chats::{MensajeChat, validar_chat};
use crate::editor::EditorLienzo;
use crate::lienzo::CajaLienzo;
use crate::{ErrorNucleo, Nucleo};

/// El nombre de la foto en el chat. Uno fijo: el almacen le pone `(1)`,
/// `(2)`... si ya hay otro (`almacen::guardar_adjunto`).
pub const NOMBRE_DE_LA_CAPTURA: &str = "pantalla-anotada.png";

/// Lo que Swift necesita saber de la tinta para decidir si guardar y que
/// trozo de la pantalla.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TintaDePantalla {
    /// Una huella de la tinta que se guardaria ([`huella`]); `None` si no
    /// hay nada.
    pub huella: Option<u64>,
    /// La caja de cada trazo, en el mundo del anotador (puntos del
    /// escritorio): para saber que pantallas tocan.
    pub cajas: Vec<CajaLienzo>,
}

/// Donde quedo una pantalla anotada.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PantallaAnotada {
    /// El id de «Mensajes guardados».
    pub chat: String,
    /// El mensaje IMAGEN, ya con su `referencia`.
    pub mensaje: MensajeChat,
    /// El id del lienzo de la foto (`foto-<id del mensaje>`).
    pub lienzo: String,
    /// Cuantos trazos quedaron encima.
    pub trazos: u32,
    /// Los pixeles de la foto (la tinta va en ellos al poner al dia).
    pub ancho: u32,
    pub alto: u32,
}

/// **Pura**: la tinta que se guarda. Lo que se ve (lo borrado o deshecho
/// no) y sin imagenes pegadas: el lienzo de la foto solo lleva la foto como
/// fichero, y una imagen suelta sin fichero que viaje se perderia al
/// abrirlo (Windows las pega DENTRO de la foto; aqui aun no).
pub fn tinta_de(escena: &Escena) -> Vec<Elemento> {
    escena
        .visibles()
        .filter(|e| !matches!(e.figura, Figura::Imagen { .. }))
        .cloned()
        .collect()
}

/// **Pura**: una huella de la tinta, para saber si cambio desde lo ultimo
/// guardado sin pasar la tinta entera a Swift. `None` sin tinta. La misma
/// tinta da la misma huella; cualquier cambio (un trazo mas, uno movido, otro
/// color) da otra.
pub fn huella(tinta: &[Elemento]) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    if tinta.is_empty() {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for e in tinta {
        // `Elemento` es `Serialize`: su JSON lo dice todo de el.
        serde_json::to_string(e).unwrap_or_default().hash(&mut h);
    }
    Some(h.finish())
}

/// **Pura**: la tinta de PUNTOS del escritorio a PIXELES de la foto: se
/// corre `origen` (donde empieza la foto en el escritorio) y se multiplica
/// por `escala` (pixeles por punto). El grosor, igual: la raya se ve igual
/// de gorda sobre la foto.
pub fn a_pixeles(tinta: &[Elemento], origen: (f32, f32), escala: f32) -> Vec<Elemento> {
    // `escalar_desde` no toca lo bloqueado; aqui todo tiene que ir a su sitio.
    let sueltos: Vec<Elemento> = tinta
        .iter()
        .map(|e| Elemento {
            bloqueado: false,
            ..e.clone()
        })
        .collect();
    pixpin_motor2d::estirar_bloque::escalar_desde(
        &sueltos,
        Punto2::nuevo(origen.0, origen.1),
        escala,
        escala,
        Punto2::nuevo(0.0, 0.0),
    )
    .into_iter()
    .zip(tinta)
    .map(|(mut e, original)| {
        e.bloqueado = original.bloqueado;
        e.grosor *= escala;
        e
    })
    .collect()
}

/// El mensaje `id` del chat, leido del cuaderno.
fn mensaje_del_chat(raiz: &Path, chat: &str, id: &str) -> Result<Option<Mensaje>, ErrorNucleo> {
    let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, chat))?;
    Ok(c.mensajes.into_iter().find(|m| m.id == id))
}

fn argumento(detalle: impl ToString) -> ErrorNucleo {
    ErrorNucleo::Argumento {
        detalle: detalle.to_string(),
    }
}

#[uniffi::export]
impl EditorLienzo {
    /// **La tinta del anotador de pantalla**, resumida: su huella y la caja
    /// de cada trazo (en el mundo del lienzo). Sin texto a medias: quien
    /// llama lo termina antes (`terminar_texto`).
    pub fn tinta_de_pantalla(&self) -> TintaDePantalla {
        let st = self.tomar();
        let tinta = tinta_de(&st.escena);
        TintaDePantalla {
            huella: huella(&tinta),
            cajas: tinta
                .iter()
                .map(|e| {
                    let (x0, y0, x1, y1) = e.caja();
                    CajaLienzo { x0, y0, x1, y1 }
                })
                .collect(),
        }
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Guarda una pantalla anotada en «Mensajes guardados»** (creandolo si
    /// no esta): `foto` es el PNG de lo que habia debajo; la tinta es la del
    /// `editor` del anotador, que empieza en `origen_x, origen_y` (puntos del
    /// escritorio) y lleva `escala` pixeles de la foto por punto. Bloqueante
    /// (copia la foto y escribe el lienzo): Swift lo llama fuera del hilo
    /// principal.
    pub fn pantalla_anotada_al_chat(
        &self,
        foto: String,
        editor: Arc<EditorLienzo>,
        origen_x: f32,
        origen_y: f32,
        escala: f32,
    ) -> Result<PantallaAnotada, ErrorNucleo> {
        let (ancho, alto) = pixpin_codec::imagen::medidas(Path::new(&foto)).map_err(argumento)?;
        let tinta = a_pixeles(
            &tinta_de(&editor.tomar().escena),
            (origen_x, origen_y),
            escala,
        );
        let raiz = self.ruta();
        let aparato = self.identidad()?.yo.codigo();
        let ficha = almacen::asegurar_guardados(raiz, crate::ahora_ms(), &aparato)?;
        let m = self.adjuntar_con(
            ficha.id.clone(),
            foto,
            Some(NOMBRE_DE_LA_CAPTURA.into()),
            |_| {},
        )?;
        let mensaje = mensaje_del_chat(raiz, &ficha.id, &m.id)?
            .ok_or_else(|| argumento("la foto no entro en el chat"))?;
        let ruta = mensaje
            .ruta
            .as_deref()
            .map(|r| almacen::carpeta(raiz, &ficha.id).join(r))
            .ok_or_else(|| argumento("el mensaje de la foto no tiene fichero"))?;
        let hecho = pixpin_proyecto::lienzo_de_la_foto::con_dibujo(
            raiz,
            &ficha.id,
            &mensaje,
            &ruta,
            (ancho, alto),
            &tinta,
            crate::ahora_ms(),
        )?;
        // Se apunto la `referencia`: el cuaderno guardado en la cache ya no vale.
        let cuaderno_fichero = almacen::carpeta(raiz, &ficha.id).join("guardados.jsonl");
        self.cache_tomada().quitar(&cuaderno_fichero);
        let mensaje = mensaje_del_chat(raiz, &ficha.id, &m.id)?.unwrap_or(mensaje);
        Ok(PantallaAnotada {
            chat: ficha.id,
            mensaje: MensajeChat::from(mensaje),
            lienzo: hecho.id,
            trazos: hecho.adoptados as u32,
            ancho,
            alto,
        })
    }

    /// **Pone al dia el lienzo de una pantalla ya guardada** en esta sesion
    /// con la tinta de ahora: ni foto nueva ni mensaje nuevo (la de fondo es
    /// la de la primera vez, sobre la que se dibujo). Lo de antes se marca
    /// borrado, que es lo que le dice al movil «esto se quito». Devuelve
    /// cuantos trazos quedaron. Sin el lienzo, error: no se inventa uno sin
    /// foto. Bloqueante.
    pub fn poner_al_dia_pantalla_anotada(
        &self,
        guardada: PantallaAnotada,
        editor: Arc<EditorLienzo>,
        origen_x: f32,
        origen_y: f32,
        escala: f32,
    ) -> Result<u32, ErrorNucleo> {
        validar_chat(&guardada.chat)?;
        let l = &guardada.lienzo;
        if l.is_empty() || l == "." || l == ".." || l.contains(['/', '\\', '\0']) {
            return Err(argumento(format!("lienzo no valido: {l:?}")));
        }
        let tinta = a_pixeles(
            &tinta_de(&editor.tomar().escena),
            (origen_x, origen_y),
            escala,
        );
        let n = pixpin_proyecto::lienzo_de_la_foto::cambiar_dibujo(
            self.ruta(),
            &guardada.chat,
            l,
            (guardada.ancho, guardada.alto),
            &tinta,
        )?;
        Ok(n as u32)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::Clase;

    fn raiz(etiqueta: &str) -> std::path::PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-anotador-chat-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Una «pantalla» inventada de dos colores, como PNG en `carpeta`.
    fn pantalla(carpeta: &Path, ancho: u32, alto: u32) -> String {
        let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
        for _ in 0..alto {
            for x in 0..ancho {
                let c = if x < ancho / 2 {
                    [40, 90, 200]
                } else {
                    [235, 235, 240]
                };
                pixeles.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .unwrap();
        let f = carpeta.join(format!("debajo-{}.png", crate::ahora_ms()));
        std::fs::write(&f, png).unwrap();
        f.display().to_string()
    }

    fn raya(puntos: &[(f32, f32)]) -> Elemento {
        Elemento {
            figura: Figura::Lapiz {
                puntos: puntos.iter().map(|&(x, y)| Punto2::nuevo(x, y)).collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            trazo: pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
            grosor: 2.0,
            ..Elemento::default()
        }
    }

    fn flecha(de: (f32, f32), a: (f32, f32)) -> Elemento {
        Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(de.0, de.1), Punto2::nuevo(a.0, a.1)],
                punta_inicio: pixpin_motor2d::formas::TipoPunta::Ninguna,
                punta_fin: pixpin_motor2d::formas::TipoPunta::Flecha,
                codos: false,
            },
            x: de.0,
            y: de.1,
            ancho: (a.0 - de.0).abs(),
            alto: (a.1 - de.1).abs(),
            trazo: pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
            grosor: 4.0,
            ..Elemento::default()
        }
    }

    /// El anotador de una sesion: un editor sobre un `.excalidraw` temporal
    /// con `tinta` dentro.
    fn anotador(r: &Path, tinta: &[Elemento]) -> Arc<EditorLienzo> {
        let f = r.join(format!("anotador-{}.excalidraw", crate::ahora_ms()));
        let e = EditorLienzo::de_fichero(f.display().to_string(), true).unwrap();
        {
            let mut st = e.tomar();
            for t in tinta {
                st.escena.anadir(t.clone());
            }
        }
        e
    }

    fn nucleo(r: &Path) -> Arc<Nucleo> {
        Nucleo::abrir(r.join("datos").display().to_string(), "Mac".into()).unwrap()
    }

    fn lo_guardado(n: &Nucleo, g: &PantallaAnotada) -> (Vec<Mensaje>, Vec<Elemento>) {
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(n.ruta(), &g.chat)).unwrap();
        let texto = std::fs::read_to_string(almacen::lienzo(n.ruta(), &g.chat, &g.lienzo)).unwrap();
        let l = pixpin_motor2d::excalidraw::leer(&texto).unwrap();
        let visibles = pixpin_motor2d::excalidraw::a_escena(&l)
            .visibles()
            .cloned()
            .collect();
        (c.mensajes, visibles)
    }

    #[test]
    fn guardar_la_pantalla_anotada_deja_una_foto_en_mensajes_guardados_con_su_lienzo_editable() {
        let r = raiz("guardar");
        let n = nucleo(&r);
        // La foto empieza en (100, 0) del escritorio y va a 2 pixeles por
        // punto: la raya de (105, 2) a (125, 7) cae de (10, 4) a (50, 14).
        let e = anotador(&r, &[raya(&[(105.0, 2.0), (125.0, 7.0)])]);
        let g = n
            .pantalla_anotada_al_chat(pantalla(&r, 64, 18), e, 100.0, 0.0, 2.0)
            .unwrap();
        assert_eq!(g.lienzo, format!("foto-{}", g.mensaje.id));
        assert_eq!((g.trazos, g.ancho, g.alto), (1, 64, 18));
        // En el indice, «Mensajes guardados».
        let indice = almacen::Indice::leer(n.ruta());
        let ficha = indice.proyectos.iter().find(|f| f.id == g.chat).unwrap();
        assert!(ficha.es_guardados());
        // Un mensaje IMAGEN como una captura cualquiera, con su referencia.
        let (mensajes, els) = lo_guardado(&n, &g);
        assert_eq!(mensajes.len(), 1);
        assert_eq!(mensajes[0].clase, Some(Clase::Imagen));
        assert_eq!(mensajes[0].referencia.as_deref(), Some(g.lienzo.as_str()));
        assert_eq!(g.mensaje.referencia.as_deref(), Some(g.lienzo.as_str()));
        // La foto es la de debajo tal cual (un PNG con sus medidas).
        let foto = almacen::carpeta(n.ruta(), &g.chat).join(mensajes[0].ruta.as_deref().unwrap());
        assert_eq!(pixpin_codec::imagen::medidas(&foto).unwrap(), (64, 18));
        // La foto primero y bloqueada; la raya encima, editable y en sus
        // pixeles (la foto cabe entera: una unidad por pixel).
        assert_eq!(els.len(), 2);
        assert!(matches!(els[0].figura, Figura::Imagen { .. }) && els[0].bloqueado);
        assert!(!els[1].bloqueado);
        let (x0, y0, x1, y1) = els[1].caja();
        let centro = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        assert!(
            (centro.0 - 30.0).abs() < 1.0 && (centro.1 - 9.0).abs() < 1.0,
            "{centro:?}"
        );
        assert!((els[1].grosor - 4.0).abs() < 1e-3, "el grosor, a pixeles");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn dos_sesiones_seguidas_son_dos_fotos_y_no_pisan_la_primera() {
        let r = raiz("dos");
        let n = nucleo(&r);
        let tinta = [raya(&[(1.0, 1.0), (6.0, 3.0)])];
        let a = n
            .pantalla_anotada_al_chat(pantalla(&r, 8, 4), anotador(&r, &tinta), 0.0, 0.0, 1.0)
            .unwrap();
        let b = n
            .pantalla_anotada_al_chat(pantalla(&r, 8, 4), anotador(&r, &tinta), 0.0, 0.0, 1.0)
            .unwrap();
        assert_eq!(a.chat, b.chat, "las dos en Mensajes guardados");
        assert_ne!(a.mensaje.id, b.mensaje.id);
        assert_ne!(a.lienzo, b.lienzo);
        let (mensajes, _) = lo_guardado(&n, &a);
        let rutas: Vec<_> = mensajes.iter().map(|m| m.ruta.clone().unwrap()).collect();
        assert_eq!(rutas.len(), 2);
        assert_ne!(
            rutas[0], rutas[1],
            "la segunda no pisa el fichero de la primera"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_que_se_guarda_es_la_que_se_ve_sin_lo_borrado_ni_imagenes_sueltas() {
        let mut e = Escena::nueva();
        e.anadir(raya(&[(0.0, 0.0), (5.0, 5.0)]));
        let borrada = e.anadir(raya(&[(1.0, 1.0), (2.0, 2.0)]));
        e.borrar(borrada);
        e.anadir(Elemento {
            figura: Figura::Imagen { id_objeto: 7 },
            ancho: 10.0,
            alto: 10.0,
            ..Elemento::default()
        });
        let t = tinta_de(&e);
        assert_eq!(t.len(), 1);
        assert!(matches!(t[0].figura, Figura::Lapiz { .. }));
        // Caso negativo: una escena vacia no da nada.
        assert!(tinta_de(&Escena::nueva()).is_empty());
    }

    #[test]
    fn la_huella_cambia_con_la_tinta_y_sin_tinta_no_hay() {
        let una = vec![raya(&[(1.0, 1.0), (5.0, 5.0)])];
        let mut dos = una.clone();
        dos.push(raya(&[(9.0, 9.0), (2.0, 2.0)]));
        assert_eq!(
            huella(&una),
            huella(&una.clone()),
            "la misma tinta, la misma huella"
        );
        assert_ne!(huella(&una), huella(&dos));
        let mut movida = una.clone();
        movida[0].mover(3.0, 0.0);
        assert_ne!(huella(&una), huella(&movida), "moverla tambien cuenta");
        // Caso negativo: sin tinta, ninguna.
        assert_eq!(huella(&[]), None);
        // Y el editor la da con la caja de cada trazo.
        let r = raiz("huella");
        let e = anotador(&r, &dos);
        let t = e.tinta_de_pantalla();
        // (La escena les pone su id al entrar: se mira la suya.)
        assert_eq!(t.huella, huella(&tinta_de(&e.tomar().escena)));
        assert!(t.huella.is_some());
        assert_eq!(t.cajas.len(), 2);
        assert_eq!(anotador(&r, &[]).tinta_de_pantalla().huella, None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn guardar_seguir_dibujando_y_salir_pone_al_dia_el_mismo_lienzo_sin_otro_mensaje() {
        let r = raiz("al-dia");
        let n = nucleo(&r);
        let e = anotador(&r, &[raya(&[(10.0, 4.0), (50.0, 14.0)])]);
        let g = n
            .pantalla_anotada_al_chat(pantalla(&r, 64, 18), e.clone(), 0.0, 0.0, 1.0)
            .unwrap();
        // Se sigue dibujando en el mismo anotador.
        {
            let mut st = e.tomar();
            st.escena.anadir(raya(&[(2.0, 2.0), (8.0, 16.0)]));
            st.escena.anadir(flecha((30.0, 2.0), (62.0, 16.0)));
        }
        let trazos = n
            .poner_al_dia_pantalla_anotada(g.clone(), e.clone(), 0.0, 0.0, 1.0)
            .unwrap();
        assert_eq!(trazos, 3);
        let (mensajes, els) = lo_guardado(&n, &g);
        assert_eq!(mensajes.len(), 1, "el mismo mensaje, no otro");
        assert_eq!(
            els.len(),
            4,
            "la foto y los tres trazos, sin duplicar el primero"
        );
        assert!(els[0].bloqueado);
        // Limpiar tambien se lleva al lienzo: queda la foto sola.
        {
            let mut st = e.tomar();
            let ids: Vec<u64> = st.escena.visibles().map(|x| x.id).collect();
            for id in ids {
                st.escena.borrar(id);
            }
        }
        assert_eq!(
            n.poner_al_dia_pantalla_anotada(g.clone(), e, 0.0, 0.0, 1.0)
                .unwrap(),
            0
        );
        let (mensajes, els) = lo_guardado(&n, &g);
        assert_eq!((mensajes.len(), els.len()), (1, 1));
        assert!(els[0].bloqueado && matches!(els[0].figura, Figura::Imagen { .. }));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn poner_al_dia_sin_un_guardado_previo_no_inventa_un_mensaje() {
        let r = raiz("al-dia-sin-previo");
        let n = nucleo(&r);
        let chat = n.chat_guardados().unwrap();
        let falsa = PantallaAnotada {
            chat: chat.id.clone(),
            mensaje: MensajeChat::from(Mensaje::default()),
            lienzo: "foto-no-esta".into(),
            trazos: 0,
            ancho: 8,
            alto: 4,
        };
        let e = anotador(&r, &[raya(&[(1.0, 1.0), (2.0, 2.0)])]);
        assert!(
            n.poner_al_dia_pantalla_anotada(falsa.clone(), e.clone(), 0.0, 0.0, 1.0)
                .is_err()
        );
        assert!(
            !almacen::carpeta(n.ruta(), &chat.id)
                .join("guardados.jsonl")
                .exists(),
            "ni un mensaje"
        );
        // Caso negativo: un lienzo que sale de su carpeta no se toca.
        let fuera = PantallaAnotada {
            lienzo: "../x".into(),
            ..falsa
        };
        assert!(
            n.poner_al_dia_pantalla_anotada(fuera, e, 0.0, 0.0, 1.0)
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_pasa_de_puntos_del_escritorio_a_pixeles_de_la_foto() {
        let antes = raya(&[(105.0, 1.0), (107.0, 5.0)]);
        let t = a_pixeles(std::slice::from_ref(&antes), (100.0, 0.0), 2.0);
        // El centro de la raya: de (106, 3) en puntos a (12, 6) en pixeles.
        let (x0, y0, x1, y1) = t[0].caja();
        let centro = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        assert!(
            (centro.0 - 12.0).abs() < 0.5 && (centro.1 - 6.0).abs() < 0.5,
            "{centro:?}"
        );
        assert!((t[0].grosor - 4.0).abs() < 1e-3);
        // Caso negativo: a 1:1 desde el origen no se mueve nada.
        let quieta = raya(&[(3.0, 4.0), (5.0, 6.0)]);
        assert_eq!(
            a_pixeles(std::slice::from_ref(&quieta), (0.0, 0.0), 1.0)[0].caja(),
            quieta.caja()
        );
    }
}
