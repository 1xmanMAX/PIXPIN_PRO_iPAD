//! **Las herramientas que lleva quien tiene la escena** en el editor de
//! Swift: la bolita, el cuentagotas de estilo, el laser, la zona, y las dos
//! varitas (la lupa y el foco) con el arrastre que apunta una lupa a otro
//! sitio.
//!
//! Copia de `dibujo/mano.rs` (`Mano::atender`) y `dibujo/lupa.rs` de
//! Windows: ninguna de ellas pasa por `Gesto::evento` —no `deja_rastro()`—
//! y la geometria es del motor (`bolita`, `estilo`, `puntero_laser`, `zona`,
//! `lupa_elemento`). Aqui solo esta que hace cada clic con la escena. El
//! mosaico y el marco si nacen del gesto: no pasan por aqui.
//!
//! Como en Windows, la zona no saca la foto: dice que caja se pidio
//! ([`ManoHerramientas::zona_pedida`]) y la foto la hace quien pinta
//! (Swift), que la devuelve con `EditorLienzo::poner_copia_de_zona`.

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{Gesto, Herramienta};
use pixpin_motor2d::lupa_elemento as lupa;
use pixpin_motor2d::vector::Punto2;

use crate::editor::FasePuntero;

/// Cuanto margen tiene el dedo al coger la zona mirada de una lupa, en
/// pixeles de pantalla (`UMBRAL_GUIA` del movil).
const MARGEN_PX: f32 = 10.0;

/// El arrastre de la zona mirada en curso: que lupa, desde donde y a donde
/// miraba al empezar.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Moviendo {
    id: u64,
    inicio: Punto2,
    foco: Punto2,
}

/// Lo que la mano recuerda entre un aviso del puntero y el siguiente.
#[derive(Debug, Default)]
pub(crate) struct ManoHerramientas {
    /// Se esta apuntando una lupa a otro sitio.
    moviendo: Option<Moviendo>,
    /// Con la Zona puesta se arrastra una copia ya sacada: la lleva la mano
    /// de siempre hasta soltar, y luego vuelve la Zona.
    pub(crate) zona_moviendo: bool,
    /// La caja `(x0, y0, x1, y1)` de la zona que se solto, a la espera de
    /// que Swift saque su foto.
    pub(crate) zona_pedida: Option<(f32, f32, f32, f32)>,
}

/// Lo que paso con un aviso que era de estas herramientas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Atendido {
    /// Hay que repintar (cambio el dibujo o lo de encima).
    pub repintar: bool,
    /// Cambio el documento (para guardar).
    pub documento: bool,
}

impl Atendido {
    fn si(repintar: bool) -> Option<Atendido> {
        Some(Atendido {
            repintar,
            documento: false,
        })
    }

    fn cambio(hecho: bool) -> Option<Atendido> {
        Some(Atendido {
            repintar: hecho,
            documento: hecho,
        })
    }
}

/// **Convierte la figura `id` en una lupa** con el estilo puesto (la tinta
/// y el grueso de la montura). Devuelve el id de la lupa, ya elegida. La
/// figura de origen se borra en el mismo paso: deshacer la devuelve a su
/// sitio y quita la lupa de una vez.
pub fn convertir_en_lupa(escena: &mut Escena, gesto: &mut Gesto, id: u64) -> Option<u64> {
    let fuente = escena.buscar(id)?.clone();
    // De fabrica, como el movil (`ItemStyle`): el doble, redonda y con
    // flecha. La forma copiada de la figura manda sobre «redonda».
    let (cristal, caja) =
        lupa::desde_figura(&fuente, gesto.estilo.aumento, true, gesto.estilo.guia)?;
    let estilo = &gesto.estilo;
    let nueva = Elemento {
        figura: Figura::Lupa { cristal },
        x: caja.0,
        y: caja.1,
        ancho: caja.2,
        alto: caja.3,
        trazo: estilo.trazo,
        grosor: estilo.grosor.de_forma(),
        opacidad: estilo.opacidad,
        semilla: fuente.semilla,
        ..Default::default()
    };
    escena.abrir_paso();
    escena.borrar_apuntando(id);
    let nuevo = escena.anadir(nueva);
    escena.cerrar_paso();
    gesto.seleccion.limpiar();
    gesto.seleccion.poner(nuevo);
    // Hecha la lupa, lo siguiente es apartarla: con la mano, como el movil
    // (`selectTool(Tool.SELECTION)`).
    gesto.herramienta = Herramienta::Mano;
    Some(nuevo)
}

/// **Convierte la figura `id` en un foco** (`focoDesdeFigura` del movil): el
/// hueco es la figura tal cual y el marco nace `MARCO_DEL_FOCO` veces mas
/// grande alrededor, que es el anillo que se oscurece. Como la lupa: la
/// figura se va con el, en un paso, y queda elegido con la mano puesta.
pub fn convertir_en_foco(escena: &mut Escena, gesto: &mut Gesto, id: u64) -> Option<u64> {
    let fuente = escena.buscar(id)?.clone();
    let (mut cristal, caja) = lupa::desde_figura(
        &fuente,
        lupa::MARCO_DEL_FOCO,
        false,
        lupa::GuiaDeLupa::Ninguna,
    )?;
    // Con cuanto oscurece el movil de fabrica (`ItemStyle.oscurecer`).
    cristal.oscurecer = Some(lupa::OSCURECER_POR_DEFECTO);
    let nuevo_foco = Elemento {
        figura: Figura::Foco { cristal },
        x: caja.0,
        y: caja.1,
        ancho: caja.2,
        alto: caja.3,
        trazo: gesto.estilo.trazo,
        grosor: gesto.estilo.grosor.de_forma(),
        semilla: fuente.semilla,
        ..Default::default()
    };
    escena.abrir_paso();
    escena.borrar_apuntando(id);
    let nuevo = escena.anadir(nuevo_foco);
    escena.cerrar_paso();
    gesto.seleccion.limpiar();
    gesto.seleccion.poner(nuevo);
    gesto.herramienta = Herramienta::Mano;
    Some(nuevo)
}

/// La lupa elegida, si hay exactamente una elegida y es lupa.
fn lupa_elegida(gesto: &Gesto, escena: &Escena) -> Option<u64> {
    let ids = gesto.seleccion.ids();
    let [id] = ids else { return None };
    escena
        .buscar(*id)
        .filter(|e| !e.borrado && lupa::de(e).is_some())
        .map(|e| e.id)
}

/// Si el punto coge la zona mirada de la lupa `e` y no su cristal. Solo
/// **fuera del cristal**: una lupa recien puesta esta encima de lo que mira,
/// y ahi arrastrar significa apartarla, que es el gesto que se hace primero.
pub fn coge_la_zona(e: &Elemento, p: Punto2, zoom: f32) -> bool {
    let Some(cr) = lupa::de(e) else { return false };
    let caja = lupa::caja_de(e);
    let margen = MARGEN_PX / zoom.max(0.0001);
    let dentro = lupa::dentro_del_contorno(p, &lupa::puntos_del_cristal(cr, caja));
    !dentro && lupa::toca_el_foco(cr, caja, p, margen)
}

impl ManoHerramientas {
    /// Si se esta apuntando una lupa a otro sitio.
    #[cfg(test)]
    pub fn ocupada(&self) -> bool {
        self.moviendo.is_some()
    }

    /// **Atiende el aviso si es de estas herramientas.** `Some` si era
    /// suyo; `None` si le toca al gesto. `p` va en el mundo; `zoom`, los
    /// pixeles de pantalla por unidad (los radios son de pantalla).
    pub fn atender(
        &mut self,
        fase: FasePuntero,
        p: Punto2,
        zoom: f32,
        gesto: &mut Gesto,
        escena: &mut Escena,
    ) -> Option<Atendido> {
        let zoom = zoom.max(0.0001);
        // Apuntando una lupa: todo el raton es suyo hasta soltar.
        if let Some(m) = self.moviendo {
            return match fase {
                FasePuntero::Mover => {
                    let foco =
                        Punto2::nuevo(m.foco.x + (p.x - m.inicio.x), m.foco.y + (p.y - m.inicio.y));
                    if let Some(e) = escena.buscar_mut(m.id)
                        && let Figura::Lupa { cristal } = &mut e.figura
                    {
                        cristal.foco = Some(foco);
                        e.tocar();
                    }
                    Atendido::si(true)
                }
                FasePuntero::Soltar => {
                    self.moviendo = None;
                    escena.cerrar_paso();
                    Atendido::cambio(true)
                }
                FasePuntero::Cancelar => {
                    self.moviendo = None;
                    escena.cancelar_paso();
                    Atendido::si(true)
                }
                FasePuntero::Pulsar => Atendido::si(false),
            };
        }

        match gesto.herramienta {
            // **La bolita** (`Tool.BOLITA` del movil): mientras se arrastra,
            // lo que toca entra o sale de la seleccion, una vez por barrido.
            Herramienta::Bolita => {
                let radio = pixpin_motor2d::bolita::RADIO_DE_LA_BOLITA / zoom;
                match fase {
                    FasePuntero::Pulsar => {
                        gesto.bolita.empezar();
                        gesto.bolita.pasar(p, radio, escena, &mut gesto.seleccion);
                        Atendido::si(true)
                    }
                    FasePuntero::Mover if gesto.bolita.centro.is_some() => {
                        gesto.bolita.pasar(p, radio, escena, &mut gesto.seleccion);
                        Atendido::si(true)
                    }
                    // Sin barrido, mover el raton no repinta nada.
                    FasePuntero::Mover => Atendido::si(false),
                    FasePuntero::Soltar | FasePuntero::Cancelar => {
                        gesto.bolita.terminar();
                        Atendido::si(true)
                    }
                }
            }

            // **El cuentagotas de estilo.** Un clic toma el estilo de lo que
            // hay debajo; los siguientes lo pegan. Es el orden natural —se
            // apunta al modelo y luego a los destinos— y deja usarla seguida.
            Herramienta::CopiarEstilo => {
                if fase != FasePuntero::Pulsar {
                    return Atendido::si(false);
                }
                let Some(id) = pixpin_motor2d::impacto::elemento_en(&escena.elementos, p) else {
                    return Atendido::si(false);
                };
                match &gesto.estilo_tomado {
                    None => {
                        if let Some(e) = escena.buscar(id) {
                            gesto.estilo_tomado = Some(pixpin_motor2d::estilo::copiar(e));
                        }
                        Atendido::si(true)
                    }
                    Some(copiado) => {
                        let mut uno = pixpin_motor2d::seleccion::Seleccion::nueva();
                        uno.poner(id);
                        let hecho = pixpin_motor2d::estilo::pegar_a(escena, &uno, copiado) > 0;
                        Atendido::cambio(hecho)
                    }
                }
            }

            // **El laser** (F14): la estela sigue al raton pulsado y se
            // apaga sola. No toca la escena: Swift repinta mientras
            // `animando()`.
            Herramienta::Laser => {
                let cambio = match fase {
                    FasePuntero::Pulsar => {
                        gesto.laser.pulsar(p);
                        true
                    }
                    FasePuntero::Mover => gesto.laser.mover(p),
                    FasePuntero::Soltar | FasePuntero::Cancelar => {
                        gesto.laser.soltar();
                        true
                    }
                };
                Atendido::si(cambio)
            }

            // **Las dos varitas** (`Tool.LUPA` y `Tool.SPOTLIGHT`): tocan
            // una figura cerrada y la convierten. Sobre el vacio no hacen
            // nada, pero el clic es suyo: pasarlo al gesto soltaria lo
            // elegido sin dibujar nada.
            Herramienta::Lupa | Herramienta::Foco => {
                if fase != FasePuntero::Pulsar {
                    return Atendido::si(false);
                }
                let hecho = lupa::figura_bajo(&escena.elementos, p)
                    .and_then(|id| {
                        if gesto.herramienta == Herramienta::Lupa {
                            convertir_en_lupa(escena, gesto, id)
                        } else {
                            convertir_en_foco(escena, gesto, id)
                        }
                    })
                    .is_some();
                Atendido::cambio(hecho)
            }

            // **La zona** (F8): se arrastra el rectangulo y al soltar se pide
            // la foto. Una copia ya sacada se arrastra con la Zona puesta,
            // como en el movil: la lleva la mano de siempre hasta soltar.
            Herramienta::Zona => match fase {
                FasePuntero::Pulsar => {
                    let copia = pixpin_motor2d::impacto::elemento_en(&escena.elementos, p)
                        .and_then(|id| escena.buscar(id))
                        .is_some_and(pixpin_motor2d::zona::es_copia);
                    if copia {
                        self.zona_moviendo = true;
                        gesto.herramienta = Herramienta::Mano;
                        None
                    } else {
                        gesto.seleccion.limpiar();
                        gesto.zona = Some((p, p));
                        Atendido::si(true)
                    }
                }
                FasePuntero::Mover => match gesto.zona {
                    Some((a, _)) => {
                        gesto.zona = Some((a, p));
                        Atendido::si(true)
                    }
                    None => Atendido::si(false),
                },
                FasePuntero::Soltar => match gesto.zona.take() {
                    Some((a, b)) => {
                        if pixpin_motor2d::zona::cuenta(a, b, zoom) {
                            self.zona_pedida = Some(pixpin_motor2d::zona::caja(a, b));
                        }
                        Atendido::si(true)
                    }
                    None => Atendido::si(false),
                },
                FasePuntero::Cancelar => Atendido::si(gesto.zona.take().is_some()),
            },

            // Con la mano y una lupa elegida, la zona mirada se coge antes
            // que la figura de debajo: lo mirado suele caer encima de algo
            // dibujado, y sin esto no habria forma de apuntarla a otro sitio.
            Herramienta::Mano if fase == FasePuntero::Pulsar => {
                let id = lupa_elegida(gesto, escena)?;
                let e = escena.buscar(id)?;
                if !coge_la_zona(e, p, zoom) {
                    return None;
                }
                let foco = lupa::foco_de(lupa::de(e)?, lupa::caja_de(e));
                escena.abrir_paso();
                escena.apuntar_edicion(id);
                self.moviendo = Some(Moviendo {
                    id,
                    inicio: p,
                    foco,
                });
                Atendido::si(true)
            }
            _ => None,
        }
    }

    /// Tras pasar el aviso al gesto: la copia de zona ya se solto y vuelve
    /// la Zona a la mano.
    pub fn tras_el_gesto(&mut self, fase: FasePuntero, gesto: &mut Gesto) {
        if self.zona_moviendo && matches!(fase, FasePuntero::Soltar | FasePuntero::Cancelar) {
            self.zona_moviendo = false;
            gesto.herramienta = Herramienta::Zona;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn escena_con_circulo() -> (Escena, u64) {
        let mut e = Escena::nueva();
        let id = e.anadir(Elemento {
            figura: Figura::Elipse,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            grosor: 2.0,
            ..Default::default()
        });
        (e, id)
    }

    fn en(x: f32, y: f32) -> Punto2 {
        Punto2::nuevo(x, y)
    }

    fn con(h: Herramienta) -> Gesto {
        let mut g = Gesto::nuevo();
        g.herramienta = h;
        g
    }

    #[test]
    fn tocar_un_circulo_con_la_lupa_lo_convierte_y_deshacer_lo_devuelve() {
        let (mut escena, circulo) = escena_con_circulo();
        let mut gesto = con(Herramienta::Lupa);
        let mut mano = ManoHerramientas::default();
        let r = mano.atender(
            FasePuntero::Pulsar,
            en(50.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(r, Atendido::cambio(true));
        let vivos: Vec<_> = escena.visibles().collect();
        assert_eq!(vivos.len(), 1, "el circulo se convirtio, no se duplico");
        assert!(matches!(vivos[0].figura, Figura::Lupa { .. }));
        assert_eq!(
            gesto.herramienta,
            Herramienta::Mano,
            "lo siguiente es apartarla"
        );
        assert_eq!(gesto.seleccion.ids(), &[vivos[0].id]);
        // Un solo paso: deshacer quita la lupa y devuelve el circulo.
        assert!(escena.deshacer());
        let vivos: Vec<_> = escena.visibles().collect();
        assert_eq!(vivos.len(), 1);
        assert_eq!(vivos[0].id, circulo);
    }

    #[test]
    fn tocar_un_circulo_con_el_foco_lo_convierte_en_un_marco_que_oscurece_alrededor() {
        let (mut escena, circulo) = escena_con_circulo();
        let mut gesto = con(Herramienta::Foco);
        let mut mano = ManoHerramientas::default();
        let r = mano.atender(
            FasePuntero::Pulsar,
            en(50.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(r, Atendido::cambio(true));
        let vivos: Vec<_> = escena.visibles().collect();
        assert_eq!(vivos.len(), 1);
        let Figura::Foco { cristal } = &vivos[0].figura else {
            panic!("{:?}", vivos[0].figura);
        };
        let (x0, _, x1, _) = vivos[0].caja();
        assert!(
            ((x1 - x0) - 100.0 * lupa::MARCO_DEL_FOCO).abs() < 2.0,
            "{x0} {x1}"
        );
        assert_eq!(
            lupa::oscurecimiento_de(cristal),
            lupa::OSCURECER_POR_DEFECTO
        );
        assert!(escena.deshacer());
        assert_eq!(escena.visibles().next().unwrap().id, circulo);
        // Caso negativo: sobre el vacio no nace nada, pero el clic es suyo.
        gesto.herramienta = Herramienta::Foco;
        let r = mano.atender(
            FasePuntero::Pulsar,
            en(900.0, 900.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(r, Atendido::cambio(false));
        assert_eq!(escena.cuantos_visibles(), 1);
    }

    #[test]
    fn apartada_la_lupa_arrastrar_su_zona_la_apunta_a_otro_sitio_sin_mover_el_cristal() {
        let (mut escena, _) = escena_con_circulo();
        let mut gesto = con(Herramienta::Lupa);
        let mut mano = ManoHerramientas::default();
        mano.atender(
            FasePuntero::Pulsar,
            en(50.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        let id = gesto.seleccion.ids()[0];
        escena.mover(id, 400.0, 0.0);
        let antes = escena.buscar(id).unwrap().clone();
        // Pulsar en la zona mirada (el circulo de antes) la coge.
        assert_eq!(
            mano.atender(
                FasePuntero::Pulsar,
                en(50.0, 50.0),
                1.0,
                &mut gesto,
                &mut escena
            ),
            Atendido::si(true)
        );
        assert!(mano.ocupada());
        mano.atender(
            FasePuntero::Mover,
            en(80.0, 60.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        mano.atender(
            FasePuntero::Soltar,
            en(80.0, 60.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        let despues = escena.buscar(id).unwrap();
        let Figura::Lupa { cristal } = &despues.figura else {
            panic!()
        };
        let f = cristal.foco.unwrap();
        assert!(
            (f.x - 80.0).abs() < 0.01 && (f.y - 60.0).abs() < 0.01,
            "{f:?}"
        );
        assert_eq!(
            (despues.x, despues.y),
            (antes.x, antes.y),
            "el cristal no se movio"
        );
        assert!(escena.deshacer());
        let Figura::Lupa { cristal } = &escena.buscar(id).unwrap().figura else {
            panic!()
        };
        assert!((cristal.foco.unwrap().x - 50.0).abs() < 0.5);
    }

    #[test]
    fn con_la_lupa_encima_de_lo_que_mira_pulsar_dentro_no_coge_la_zona() {
        let (mut escena, _) = escena_con_circulo();
        let mut gesto = con(Herramienta::Lupa);
        let mut mano = ManoHerramientas::default();
        mano.atender(
            FasePuntero::Pulsar,
            en(50.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        let r = mano.atender(
            FasePuntero::Pulsar,
            en(50.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(r, None, "apartarla es cosa del gesto");
        assert!(!mano.ocupada());
    }

    #[test]
    fn el_cuentagotas_toma_de_la_primera_y_pega_en_las_siguientes() {
        let mut escena = Escena::nueva();
        let rojo = pixpin_motor2d::elemento::ColorRgba::opaco(1.0, 0.0, 0.0);
        let a = escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            ancho: 50.0,
            alto: 50.0,
            trazo: rojo,
            grosor: 2.0,
            ..Default::default()
        });
        let b = escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            x: 200.0,
            ancho: 50.0,
            alto: 50.0,
            grosor: 2.0,
            ..Default::default()
        });
        let mut gesto = con(Herramienta::CopiarEstilo);
        let mut mano = ManoHerramientas::default();
        // El primer clic solo toma: no cambia el documento.
        let r = mano.atender(
            FasePuntero::Pulsar,
            en(0.0, 25.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(r, Atendido::si(true));
        assert!(gesto.estilo_tomado.is_some());
        let r = mano.atender(
            FasePuntero::Pulsar,
            en(200.0, 25.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(r, Atendido::cambio(true));
        assert_eq!(escena.buscar(b).unwrap().trazo, rojo);
        assert_eq!(escena.buscar(a).unwrap().trazo, rojo);
        // Un clic en el aire no hace nada.
        let r = mano.atender(
            FasePuntero::Pulsar,
            en(900.0, 900.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(r, Atendido::si(false));
    }

    #[test]
    fn la_bolita_elige_lo_que_toca_al_pasar() {
        let (mut escena, circulo) = escena_con_circulo();
        let mut gesto = con(Herramienta::Bolita);
        let mut mano = ManoHerramientas::default();
        mano.atender(
            FasePuntero::Pulsar,
            en(-200.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert!(gesto.seleccion.ids().is_empty());
        mano.atender(
            FasePuntero::Mover,
            en(0.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(gesto.seleccion.ids(), &[circulo]);
        mano.atender(
            FasePuntero::Soltar,
            en(0.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert!(gesto.bolita.centro.is_none());
        assert_eq!(escena.cuantos_visibles(), 1, "elegir no cambia el dibujo");
    }

    #[test]
    fn la_zona_pide_su_caja_al_soltar_y_un_clic_no_cuenta() {
        let mut escena = Escena::nueva();
        let mut gesto = con(Herramienta::Zona);
        let mut mano = ManoHerramientas::default();
        mano.atender(
            FasePuntero::Pulsar,
            en(10.0, 10.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        mano.atender(
            FasePuntero::Mover,
            en(110.0, 60.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        mano.atender(
            FasePuntero::Soltar,
            en(110.0, 60.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(mano.zona_pedida.take(), Some((10.0, 10.0, 110.0, 60.0)));
        // Caso negativo: un clic suelto no saca nada.
        mano.atender(
            FasePuntero::Pulsar,
            en(10.0, 10.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        mano.atender(
            FasePuntero::Soltar,
            en(12.0, 11.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(mano.zona_pedida, None);
        assert_eq!(escena.cuantos_visibles(), 0);
    }

    #[test]
    fn el_laser_no_deja_nada_en_el_dibujo() {
        let mut escena = Escena::nueva();
        let mut gesto = con(Herramienta::Laser);
        let mut mano = ManoHerramientas::default();
        mano.atender(
            FasePuntero::Pulsar,
            en(0.0, 0.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        mano.atender(
            FasePuntero::Mover,
            en(50.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert!(gesto.laser.vivo());
        assert!(!gesto.laser.ordenes(1.0).is_empty());
        mano.atender(
            FasePuntero::Soltar,
            en(50.0, 50.0),
            1.0,
            &mut gesto,
            &mut escena,
        );
        assert_eq!(escena.cuantos_visibles(), 0);
        assert!(!escena.hay_que_deshacer());
    }
}
