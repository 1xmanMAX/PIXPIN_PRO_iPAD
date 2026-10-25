//! **Las mini-apps** (tareas, contador, ruleta, gastos...) para los pines de
//! herramienta del Mac.
//!
//! No hay formato nuevo: el documento es el texto de un mensaje `MINIAPP`
//! (`pixpin_proyecto::mini`), escrito letra por letra como lo escriben el
//! movil y Windows, y aqui solo se exponen sus funciones. Cada operacion es
//! **documento entra, documento sale**: Swift no aprende el formato, solo
//! guarda el texto que le devuelven. Asi lo que el PC o el movil no entienden
//! (las claves de mas del contador, la fecha de cada tarea) se conserva.
//!
//! Para que un pin de herramienta no se pierda al cerrar la app (el Mac no
//! tiene el almacen de pines de Windows), vive en un mensaje del chat, como
//! la lista «sacada a la pantalla» de Windows (`pines/herramienta.rs`):
//! [`Nucleo::crear_miniapp`] lo crea y [`Nucleo::guardar_miniapp`] lo
//! reescribe con el cerrojo de los cuadernos (`cuaderno::cambiar`).
//!
//! # Rendimiento
//!
//! Los documentos son cortos (una lista, una cuenta): cruzan enteros, una
//! llamada por toque. Nada se queda en memoria en Rust.

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};
use pixpin_proyecto::mini::{self, contador, gastos, ruleta};

use crate::chats::{MensajeChat, hora_unica, validar_chat};
use crate::tareas::Tarea;
use crate::{ErrorNucleo, Nucleo};

/// Lo que ensena una mini-app sin abrirla (`mini::Resumen`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ResumenMini {
    /// Ya compuesto: «3 de 7», «60,50 €».
    pub texto: String,
    pub hechas: u32,
    pub de: u32,
    /// De 0 a 1, o nada si no hay avance que ensenar.
    pub avance: Option<f32>,
    pub vacia: bool,
}

/// Un contador: su numero y cuanto sube cada toque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct CuentaMini {
    pub valor: i64,
    pub paso: i64,
}

/// Lo que se le hace a un contador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccionContador {
    Mas,
    Menos,
    Reiniciar,
    /// Cambiar el paso (minimo 1).
    Paso {
        paso: i64,
    },
}

/// A quien le toco en la ruleta y donde se para la rueda.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GiroRuleta {
    pub elegido: u32,
    pub nombre: String,
    /// Grados que gira la rueda (con sus vueltas de adorno) para que la
    /// aguja, arriba y quieta, senale al elegido (`ruleta::angulo_final`).
    pub angulo: f32,
}

/// Una linea de gastos.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GastoMini {
    pub concepto: String,
    /// En unidades menores de la moneda del libro.
    pub centimos: i64,
    /// Ya escrito para ensenar («12,50 €»).
    pub importe: String,
}

/// Unos gastos enteros, leidos.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LibroMini {
    pub titulo: String,
    /// El codigo de tres letras (`EUR`).
    pub moneda: String,
    pub simbolo: String,
    pub decimales: u8,
    pub gastos: Vec<GastoMini>,
    pub total: i64,
    pub total_texto: String,
}

/// La moneda de un codigo, o el euro si no es un codigo (lo que hace el
/// movil cuando el sistema no sabe decir nada).
fn moneda_de(codigo: &str) -> gastos::Moneda {
    gastos::Moneda::de_codigo(codigo).unwrap_or_else(gastos::Moneda::euro)
}

fn hoy(desfase_ms: i64) -> mini::Fecha {
    mini::Fecha::de_ms_locales(crate::ahora_ms() + desfase_ms)
}

// ---- Lo comun a todas -------------------------------------------------------

/// Las palabras de las mini-apps, en el orden del movil (`mini::TODAS`).
#[uniffi::export]
pub fn mini_todas() -> Vec<String> {
    mini::TODAS.iter().map(|s| s.to_string()).collect()
}

/// El documento con el que nace una mini-app, o nada si no se conoce esa
/// palabra. `moneda` es el codigo del aparato (solo para los gastos).
#[uniffi::export]
pub fn mini_documento_nuevo(miniapp: String, titulo: String, moneda: String) -> Option<String> {
    mini::documento_nuevo(&miniapp, &titulo, &moneda_de(&moneda))
}

/// Lo que ensena la burbuja (o la fila del panel de pines).
#[uniffi::export]
pub fn mini_resumen(miniapp: String, documento: String, moneda: String) -> Option<ResumenMini> {
    mini::resumen(&miniapp, &documento, &moneda_de(&moneda)).map(|r| ResumenMini {
        texto: r.texto,
        hechas: r.hechas as u32,
        de: r.de as u32,
        avance: r.avance,
        vacia: r.vacia,
    })
}

/// El titulo del documento (vacio si no lleva).
#[uniffi::export]
pub fn mini_titulo(documento: String) -> String {
    mini::titulo(&documento)
}

/// El documento con otro titulo.
#[uniffi::export]
pub fn mini_con_titulo(
    miniapp: String,
    documento: String,
    titulo: String,
    moneda: String,
) -> Option<String> {
    mini::con_titulo(&miniapp, &documento, &titulo, &moneda_de(&moneda))
}

// ---- Tareas -----------------------------------------------------------------

/// Las tareas de un documento, con su fecha y los dias que llevan (sin
/// imagenes: resolverlas es cosa del chat, `Nucleo::tareas`).
#[uniffi::export]
pub fn mini_tareas_leer(documento: String, desfase_ms: i64) -> Vec<Tarea> {
    let hoy = hoy(desfase_ms);
    mini::leer_tareas(&documento)
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            let (visible, creada) = mini::partir(&t.texto);
            let (texto, _) = mini::imagenes_de(visible);
            Tarea {
                indice: i as u32,
                // La hora de recordarla (`⏰`) aparte, como en `Nucleo::tareas`.
                recuerda: crate::tareas::hora_de_tarea(&texto),
                texto: crate::tareas::sin_marca(&texto),
                hecha: t.hecha,
                creada: creada.map(|f| f.to_string()),
                dias: creada.map(|f| mini::dias_desde(f, hoy)),
                imagenes: Vec::new(),
                crudo: t.texto,
            }
        })
        .collect()
}

/// Marca o desmarca la tarea `indice` (la casilla del chat y del movil).
#[uniffi::export]
pub fn mini_tareas_alternar(documento: String, indice: u32) -> String {
    mini::alternar(&documento, indice as usize)
}

/// Anade una tarea al final con la fecha de hoy (`➕ AAAA-MM-DD`).
#[uniffi::export]
pub fn mini_tareas_anadir(documento: String, texto: String, desfase_ms: i64) -> String {
    mini::anadir_el(&documento, &texto, hoy(desfase_ms))
}

/// Quita la tarea `indice`.
#[uniffi::export]
pub fn mini_tareas_quitar(documento: String, indice: u32) -> String {
    mini::quitar(&documento, indice as usize)
}

/// Le cambia el texto a una tarea (conserva su fecha de creacion).
#[uniffi::export]
pub fn mini_tareas_renombrar(documento: String, indice: u32, texto: String) -> String {
    mini::renombrar(&documento, indice as usize, &texto)
}

// ---- Contador ---------------------------------------------------------------

#[uniffi::export]
pub fn mini_contador_leer(documento: String) -> CuentaMini {
    let c = contador::leer(&documento);
    CuentaMini {
        valor: c.valor,
        paso: c.paso,
    }
}

/// El documento tras un toque. Las claves que este aparato no entiende se
/// devuelven tal cual (`Cuenta::otros`).
#[uniffi::export]
pub fn mini_contador_cambiar(documento: String, accion: AccionContador) -> String {
    let c = contador::leer(&documento);
    let nueva = match accion {
        AccionContador::Mas => contador::mas(&c),
        AccionContador::Menos => contador::menos(&c),
        AccionContador::Reiniciar => contador::reiniciar(&c),
        AccionContador::Paso { paso } => contador::con_paso(&c, paso),
    };
    contador::escribir(&mini::titulo(&documento), &nueva)
}

// ---- Ruleta -----------------------------------------------------------------

#[uniffi::export]
pub fn mini_ruleta_leer(documento: String) -> Vec<String> {
    ruleta::leer(&documento)
}

#[uniffi::export]
pub fn mini_ruleta_anadir(documento: String, texto: String) -> String {
    let nombres = ruleta::anadir(&ruleta::leer(&documento), &texto);
    ruleta::escribir(&mini::titulo(&documento), &nombres)
}

#[uniffi::export]
pub fn mini_ruleta_quitar(documento: String, indice: u32) -> String {
    let nombres = ruleta::quitar(&ruleta::leer(&documento), indice as usize);
    ruleta::escribir(&mini::titulo(&documento), &nombres)
}

/// Gira: `azar` de 0 a 1 lo pone Swift. Nada si no hay a quien elegir.
#[uniffi::export]
pub fn mini_ruleta_girar(documento: String, azar: f64) -> Option<GiroRuleta> {
    let nombres = ruleta::leer(&documento);
    let elegido = ruleta::elegir(&nombres, azar)?;
    Some(GiroRuleta {
        elegido: elegido as u32,
        nombre: nombres[elegido].clone(),
        angulo: ruleta::angulo_final(elegido, nombres.len(), ruleta::VUELTAS_DE_ADORNO),
    })
}

// ---- Gastos -----------------------------------------------------------------

/// Los gastos de un documento. `moneda` solo vale si el documento no dice
/// la suya.
#[uniffi::export]
pub fn mini_gastos_leer(documento: String, moneda: String) -> LibroMini {
    let libro = gastos::leer(&documento, &moneda_de(&moneda));
    let m = &libro.moneda;
    LibroMini {
        titulo: libro.titulo.clone(),
        moneda: m.codigo().to_string(),
        simbolo: m.simbolo().to_string(),
        decimales: m.decimales(),
        gastos: libro
            .gastos
            .iter()
            .map(|g| GastoMini {
                concepto: g.concepto.clone(),
                centimos: g.centimos,
                importe: gastos::texto_de_importe(g.centimos, m),
            })
            .collect(),
        total: libro.total(),
        total_texto: gastos::texto_de_importe(libro.total(), m),
    }
}

/// Anade un gasto con el importe tal como se tecleo («12,5», «-3»). Un
/// importe que no se entiende es un error de argumento (no se apunta cero).
#[uniffi::export]
pub fn mini_gastos_anadir(
    documento: String,
    moneda: String,
    concepto: String,
    importe: String,
) -> Result<String, ErrorNucleo> {
    let libro = gastos::leer(&documento, &moneda_de(&moneda));
    let centimos = gastos::centimos_de(&importe, libro.moneda.decimales()).ok_or_else(|| {
        ErrorNucleo::Argumento {
            detalle: format!("importe no valido: {importe:?}"),
        }
    })?;
    Ok(gastos::escribir(&gastos::anadir(
        &libro, &concepto, centimos,
    )))
}

#[uniffi::export]
pub fn mini_gastos_borrar(documento: String, moneda: String, indice: u32) -> String {
    let libro = gastos::leer(&documento, &moneda_de(&moneda));
    gastos::escribir(&gastos::borrar(&libro, indice as usize))
}

// ---- En un mensaje del chat ---------------------------------------------------

fn es_miniapp(m: &Mensaje) -> bool {
    m.clase == Some(Clase::MiniApp) && m.miniapp.is_some()
}

#[uniffi::export]
impl Nucleo {
    /// **Crea una mini-app en un chat**: un mensaje `MINIAPP` con su
    /// documento de nacer, como el boton «+» del movil (`pedidos::nueva_lista`
    /// en Windows). Sube el chat en la lista.
    pub fn crear_miniapp(
        &self,
        chat: String,
        miniapp: String,
        titulo: String,
        moneda: String,
    ) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let documento =
            mini::documento_nuevo(&miniapp, &titulo, &moneda_de(&moneda)).ok_or_else(|| {
                ErrorNucleo::Argumento {
                    detalle: format!("mini-app desconocida: {miniapp:?}"),
                }
            })?;
        let raiz = self.ruta();
        let mut indice = Indice::leer_si_esta(raiz)?;
        if indice.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let aparato = self.identidad()?.yo.codigo();
        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero = carpeta.join("guardados.jsonl");
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let cuando = hora_unica();
        let m = Mensaje::miniapp(
            &miniapp,
            &titulo,
            &documento,
            &Sello {
                cuando,
                numero,
                aparato,
                proyecto: chat.clone(),
            },
        );
        cuaderno::anadir(&carpeta, &m)?;
        cache.quitar(&fichero);
        drop(cache);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = cuando;
            f.resumen = m.resumen();
        }
        indice.guardar(raiz)?;
        Ok(MensajeChat::from(m))
    }

    /// El documento de una mini-app tal como esta ahora en el chat (por si
    /// llego cambiado del movil).
    pub fn documento_miniapp(&self, chat: String, mensaje: String) -> Result<String, ErrorNucleo> {
        validar_chat(&chat)?;
        let fichero = almacen::carpeta(self.ruta(), &chat).join("guardados.jsonl");
        let texto = match std::fs::read_to_string(&fichero) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        texto
            .lines()
            .filter(|l| l.contains(mensaje.as_str()))
            .filter_map(|l| serde_json::from_str::<Mensaje>(l).ok())
            .find(|m| m.id == mensaje && es_miniapp(m))
            .map(|m| m.texto)
            .ok_or(ErrorNucleo::NoExiste { que: mensaje })
    }

    /// **Guarda el documento** de una mini-app en su mensaje. `anterior` es
    /// el documento sobre el que se hizo el cambio: si el del chat ya no es
    /// ese (llego otro del movil), no se toca nada y se devuelve `false`
    /// (hay que releer con [`Nucleo::documento_miniapp`]).
    pub fn guardar_miniapp(
        &self,
        chat: String,
        mensaje: String,
        anterior: String,
        documento: String,
    ) -> Result<bool, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        let mut era = true;
        let mut sigue = false;
        let r = cuaderno::cambiar(&carpeta, &mensaje, |m| {
            if !es_miniapp(m) {
                era = false;
                return false;
            }
            if m.texto != anterior {
                return false;
            }
            sigue = true;
            if m.texto == documento {
                return false;
            }
            m.texto = documento.clone();
            true
        });
        let r = match r {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            otro => otro?,
        };
        if r.is_none() {
            return Err(ErrorNucleo::NoExiste { que: mensaje });
        }
        if !era {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("no es una mini-app: {mensaje}"),
            });
        }
        self.cache_tomada().quitar(&carpeta.join("guardados.jsonl"));
        Ok(sigue)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn tareas_ida_y_vuelta_con_fecha() {
        let d = mini_documento_nuevo("tareas".into(), "Compra".into(), "EUR".into()).unwrap();
        let d = mini_tareas_anadir(d, "pan".into(), 0);
        let d = mini_tareas_anadir(d, "leche".into(), 0);
        let d = mini_tareas_alternar(d, 0);
        let t = mini_tareas_leer(d.clone(), 0);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].texto, "pan");
        assert!(t[0].hecha && !t[1].hecha);
        assert_eq!(t[0].dias, Some(0));
        assert!(t[0].crudo.contains('\u{2795}'));
        assert_eq!(mini_titulo(d.clone()), "Compra");
        let d = mini_tareas_quitar(d, 0);
        assert_eq!(mini_tareas_leer(d, 0).len(), 1);
    }

    #[test]
    fn contador_conserva_titulo_y_paso() {
        let d = mini_documento_nuevo("contador".into(), "Cajas".into(), "EUR".into()).unwrap();
        let d = mini_contador_cambiar(d, AccionContador::Paso { paso: 12 });
        let d = mini_contador_cambiar(d, AccionContador::Mas);
        let d = mini_contador_cambiar(d, AccionContador::Mas);
        assert_eq!(
            mini_contador_leer(d.clone()),
            CuentaMini {
                valor: 24,
                paso: 12
            }
        );
        assert_eq!(mini_titulo(d.clone()), "Cajas");
        let d = mini_contador_cambiar(d, AccionContador::Reiniciar);
        assert_eq!(mini_contador_leer(d), CuentaMini { valor: 0, paso: 12 });
    }

    #[test]
    fn ruleta_elige_y_senala() {
        let d = mini_documento_nuevo("ruleta".into(), "".into(), "EUR".into()).unwrap();
        assert!(mini_ruleta_girar(d.clone(), 0.5).is_none());
        let d = mini_ruleta_anadir(d, "ana".into());
        let d = mini_ruleta_anadir(d, "luis".into());
        assert_eq!(mini_ruleta_leer(d.clone()), vec!["ana", "luis"]);
        let g = mini_ruleta_girar(d.clone(), 0.75).unwrap();
        assert_eq!((g.elegido, g.nombre.as_str()), (1, "luis"));
        assert_eq!(g.angulo, 4.0 * 360.0 - 270.0);
        assert_eq!(mini_ruleta_leer(mini_ruleta_quitar(d, 0)), vec!["luis"]);
    }

    #[test]
    fn gastos_suman_y_un_importe_raro_no_entra() {
        let d = mini_documento_nuevo("gastos".into(), "Viaje".into(), "EUR".into()).unwrap();
        let d = mini_gastos_anadir(d, "EUR".into(), "Taxi".into(), "12,5".into()).unwrap();
        let d = mini_gastos_anadir(d, "EUR".into(), "Cafe".into(), "1.20".into()).unwrap();
        let l = mini_gastos_leer(d.clone(), "USD".into());
        assert_eq!(l.moneda, "EUR", "manda la del documento");
        assert_eq!(l.total, 1370);
        assert_eq!(l.gastos[0].centimos, 1250);
        assert!(mini_gastos_anadir(d.clone(), "EUR".into(), "x".into(), "abc".into()).is_err());
        let l = mini_gastos_leer(mini_gastos_borrar(d, "EUR".into(), 0), "EUR".into());
        assert_eq!(l.total, 120);
    }

    #[test]
    fn una_miniapp_desconocida_no_tiene_documento() {
        assert!(mini_documento_nuevo("otra".into(), "".into(), "EUR".into()).is_none());
        assert!(mini_resumen("otra".into(), "".into(), "EUR".into()).is_none());
        assert_eq!(mini_todas().len(), 7);
    }

    #[test]
    fn en_el_chat_se_crea_se_guarda_y_no_pisa_un_cambio_ajeno() {
        let dir = std::env::temp_dir().join(format!("pixpin-miniapps-{}", hora_unica()));
        let n = Nucleo::abrir(dir.display().to_string(), "Mac".into()).unwrap();
        let chat = n.chat_guardados().unwrap().id;
        let m = n
            .crear_miniapp(
                chat.clone(),
                "contador".into(),
                "Cajas".into(),
                "EUR".into(),
            )
            .unwrap();
        assert_eq!(m.clase, "MINIAPP");
        let d0 = n.documento_miniapp(chat.clone(), m.id.clone()).unwrap();
        let d1 = mini_contador_cambiar(d0.clone(), AccionContador::Mas);
        assert!(
            n.guardar_miniapp(chat.clone(), m.id.clone(), d0.clone(), d1.clone())
                .unwrap()
        );
        assert_eq!(n.documento_miniapp(chat.clone(), m.id.clone()).unwrap(), d1);
        // Sobre un documento viejo no se guarda nada.
        let d2 = mini_contador_cambiar(d0.clone(), AccionContador::Menos);
        assert!(
            !n.guardar_miniapp(chat.clone(), m.id.clone(), d0, d2)
                .unwrap()
        );
        assert_eq!(
            mini_contador_leer(n.documento_miniapp(chat.clone(), m.id).unwrap()).valor,
            1
        );
        assert!(
            n.crear_miniapp(chat, "otra".into(), "".into(), "EUR".into())
                .is_err()
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
