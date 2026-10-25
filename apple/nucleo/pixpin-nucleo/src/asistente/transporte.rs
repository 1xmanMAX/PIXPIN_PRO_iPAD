//! **El HTTPS del asistente en el Mac**: URLSession desde Swift.
//!
//! `pixpin_ia::nube::http` habla con Claude y DeepSeek por un
//! [`pixpin_ia::nube::http::Transporte`] que pone la app (en Windows lo hace
//! WinHTTP). Aqui ese transporte es un **interfaz que implementa Swift**
//! ([`TransporteApp`]): manda el POST con URLSession (TLS, proxy y
//! certificados del sistema) y va pasando la respuesta a un
//! [`ReceptorHttp`] mientras llega. Asi Claude, DeepSeek, los reintentos y
//! «probar» son el mismo codigo que Windows, sin un TLS propio en el
//! ejecutable.
//!
//! El receptor lo llama Swift desde la cola de su `URLSession` (otro hilo):
//! cada trozo cruza por un canal al hilo del turno, que es quien lee el SSE.
//! Por eso la llamada a Swift va en un hilo aparte y el del turno espera en
//! el canal (y puede pedir parar entre trozo y trozo).
//!
//! La clave va en las cabeceras: Swift la pone en la peticion y nunca la
//! escribe en un registro.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pixpin_ia::motor::Seguir;
use pixpin_ia::nube::http::{self, EventoHttp, Pedido, Transporte};

use crate::ErrorNucleo;

/// Una cabecera de la peticion.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CabeceraHttp {
    pub nombre: String,
    pub valor: String,
}

/// **El HTTPS de la app**, implementado en Swift (URLSession).
#[uniffi::export(foreign)]
pub trait TransporteApp: Send + Sync {
    /// Manda un POST a `url` con `cabeceras` y `cuerpo` y **espera** a que
    /// acabe, contando la respuesta a `receptor`: primero
    /// [`ReceptorHttp::estado`], luego cada [`ReceptorHttp::datos`]. Si
    /// `datos` devuelve `false`, hay que cancelar la peticion y volver.
    /// Devuelve el error de red (sin conexion, TLS, tiempo), o `None`.
    fn enviar(
        &self,
        url: String,
        cabeceras: Vec<CabeceraHttp>,
        cuerpo: Vec<u8>,
        receptor: Arc<ReceptorHttp>,
    ) -> Option<String>;

    /// **Una peticion cualquiera** (GET, POST, PUT...) con solo las
    /// `cabeceras` dadas: las conexiones del asistente (Google Drive y su
    /// OAuth). Como `enviar`, pero con su metodo, sin `Content-Type` ni
    /// `Accept` puestos, y contando la cabecera `Location` al receptor
    /// ([`ReceptorHttp::ubicacion`]) antes de los datos.
    fn pedir(
        &self,
        metodo: String,
        url: String,
        cabeceras: Vec<CabeceraHttp>,
        cuerpo: Vec<u8>,
        receptor: Arc<ReceptorHttp>,
    ) -> Option<String>;
}

/// Lo que va llegando, de Swift al hilo del turno.
enum Llegada {
    Estado { codigo: u32, esperar: Option<Duration> },
    Datos(Vec<u8>),
    Ubicacion(String),
    Fin(Option<String>),
}

/// **Donde Swift deja la respuesta** mientras llega.
#[derive(uniffi::Object)]
pub struct ReceptorHttp {
    tx: Mutex<Option<Sender<Llegada>>>,
    parar: AtomicBool,
}

#[uniffi::export]
impl ReceptorHttp {
    /// La cabecera: el codigo HTTP y `retry-after` en segundos (o negativo
    /// si no lo dijo).
    pub fn estado(&self, codigo: u32, reintentar_en_s: f64) {
        let esperar = (reintentar_en_s.is_finite() && reintentar_en_s >= 0.0)
            .then(|| Duration::from_secs_f64(reintentar_en_s.min(120.0)));
        self.mandar(Llegada::Estado { codigo, esperar });
    }

    /// Un trozo del cuerpo. `false`: hay que cancelar (el usuario paro o ya
    /// no hay quien lea).
    /// La cabecera `Location` de la respuesta (si la hay), tras `estado`.
    pub fn ubicacion(&self, valor: String) {
        self.mandar(Llegada::Ubicacion(valor));
    }

    pub fn datos(&self, trozo: Vec<u8>) -> bool {
        if self.parar.load(Ordering::Relaxed) {
            return false;
        }
        self.mandar(Llegada::Datos(trozo)) && !self.parar.load(Ordering::Relaxed)
    }
}

impl ReceptorHttp {
    fn nuevo() -> (Arc<ReceptorHttp>, Receiver<Llegada>) {
        let (tx, rx) = channel();
        (
            Arc::new(ReceptorHttp {
                tx: Mutex::new(Some(tx)),
                parar: AtomicBool::new(false),
            }),
            rx,
        )
    }

    fn mandar(&self, l: Llegada) -> bool {
        let g = self.tx.lock().unwrap_or_else(|e| e.into_inner());
        g.as_ref().is_some_and(|tx| tx.send(l).is_ok())
    }

    fn cerrar(&self, error: Option<String>) {
        let mut g = self.tx.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(tx) = g.take() {
            let _ = tx.send(Llegada::Fin(error));
        }
    }
}

/// El adaptador al trait de `pixpin-ia`.
struct Adaptador {
    app: Arc<dyn TransporteApp>,
}

impl Transporte for Adaptador {
    fn enviar(&self, p: &Pedido, al_evento: &mut dyn FnMut(EventoHttp) -> Seguir) -> Result<(), String> {
        let (receptor, rx) = ReceptorHttp::nuevo();
        let cabeceras = p
            .cabeceras
            .iter()
            .map(|(k, v)| CabeceraHttp {
                nombre: (*k).to_string(),
                valor: v.clone(),
            })
            .collect();
        let (app, url, cuerpo, r) = (self.app.clone(), p.url.to_string(), p.cuerpo.to_vec(), receptor.clone());
        let lanzado = std::thread::Builder::new()
            .name("asistente-https".into())
            .spawn(move || {
                let error = app.enviar(url, cabeceras, cuerpo, r.clone());
                r.cerrar(error);
            });
        if let Err(e) = lanzado {
            return Err(format!("no se pudo lanzar la peticion: {e}"));
        }
        leer(&receptor, &rx, al_evento)
    }

    fn pedir(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        al_evento: &mut dyn FnMut(EventoHttp) -> Seguir,
    ) -> Result<(), String> {
        let (receptor, rx) = ReceptorHttp::nuevo();
        let cabeceras = cabeceras
            .iter()
            .map(|(k, v)| CabeceraHttp {
                nombre: (*k).to_string(),
                valor: v.clone(),
            })
            .collect();
        let (app, metodo, url, cuerpo, r) =
            (self.app.clone(), metodo.to_string(), url.to_string(), cuerpo.to_vec(), receptor.clone());
        let lanzado = std::thread::Builder::new()
            .name("conexion-https".into())
            .spawn(move || {
                let error = app.pedir(metodo, url, cabeceras, cuerpo, r.clone());
                r.cerrar(error);
            });
        if let Err(e) = lanzado {
            return Err(format!("no se pudo lanzar la peticion: {e}"));
        }
        leer(&receptor, &rx, al_evento)
    }
}

/// Lee lo que llega hasta el final (o hasta que `al_evento` pide parar: se
/// le dice a Swift y se sigue leyendo hasta que suelte, para no dejarle
/// escribiendo en un canal sin nadie).
fn leer(
    receptor: &ReceptorHttp,
    rx: &Receiver<Llegada>,
    al_evento: &mut dyn FnMut(EventoHttp) -> Seguir,
) -> Result<(), String> {
    let mut parado = false;
    loop {
        match rx.recv() {
            Ok(Llegada::Estado { codigo, esperar }) if !parado => {
                if al_evento(EventoHttp::Estado { codigo, esperar }) == Seguir::Parar {
                    parado = true;
                    receptor.parar.store(true, Ordering::Relaxed);
                }
            }
            Ok(Llegada::Ubicacion(u)) if !parado => {
                let _ = al_evento(EventoHttp::Ubicacion(&u));
            }
            Ok(Llegada::Datos(b)) if !parado => {
                if al_evento(EventoHttp::Datos(&b)) == Seguir::Parar {
                    parado = true;
                    receptor.parar.store(true, Ordering::Relaxed);
                }
            }
            Ok(Llegada::Fin(error)) => return if parado { Ok(()) } else { error.map_or(Ok(()), Err) },
            Ok(_) => {}
            // Swift solto el receptor sin cerrar (no deberia): se acabo.
            Err(_) => return Ok(()),
        }
    }
}

/// **Instala el HTTPS de la app** (una vez por proceso). Instalarlo otra vez
/// da error de argumento y se queda el primero.
#[uniffi::export]
pub fn instalar_transporte(transporte: Arc<dyn TransporteApp>) -> Result<(), ErrorNucleo> {
    http::instalar_transporte(Box::new(Adaptador { app: transporte })).map_err(|detalle| ErrorNucleo::Argumento { detalle })
}

/// Si ya hay HTTPS para la nube.
#[uniffi::export]
pub fn hay_transporte() -> bool {
    http::hay_transporte()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un URLSession de mentira: contesta lo guionado desde otro hilo.
    struct Falso {
        codigo: u32,
        trozos: Vec<&'static str>,
        error: Option<&'static str>,
        cancelado: Arc<AtomicBool>,
    }

    impl TransporteApp for Falso {
        fn enviar(&self, url: String, cabeceras: Vec<CabeceraHttp>, cuerpo: Vec<u8>, r: Arc<ReceptorHttp>) -> Option<String> {
            assert!(url.starts_with("https://"));
            assert!(cabeceras.iter().any(|c| c.nombre == "x-api-key"));
            assert_eq!(cuerpo, b"{}");
            if let Some(e) = self.error {
                return Some(e.into());
            }
            r.estado(self.codigo, if self.codigo == 429 { 3.0 } else { -1.0 });
            for t in &self.trozos {
                // Como la red: los trozos no llegan todos de golpe.
                std::thread::sleep(Duration::from_millis(2));
                if !r.datos(t.as_bytes().to_vec()) {
                    self.cancelado.store(true, Ordering::Relaxed);
                    return None;
                }
            }
            None
        }

        fn pedir(&self, metodo: String, url: String, cabeceras: Vec<CabeceraHttp>, cuerpo: Vec<u8>, r: Arc<ReceptorHttp>) -> Option<String> {
            assert_eq!(metodo, "PUT");
            assert!(url.starts_with("https://"));
            assert!(cabeceras.iter().any(|c| c.nombre == "Authorization"));
            r.estado(self.codigo, -1.0);
            r.ubicacion("https://subida/1".into());
            r.datos(cuerpo);
            None
        }
    }

    #[test]
    fn pedir_lleva_su_metodo_y_trae_la_ubicacion() {
        let (t, _) = falso(200, vec![], None);
        let mut oido = Vec::new();
        let r = Transporte::pedir(
            &t,
            "PUT",
            "https://www.googleapis.com/upload/drive/v3/files",
            &[("Authorization", "Bearer x".into())],
            b"hola",
            &mut |e| {
                oido.push(match e {
                    EventoHttp::Estado { codigo, .. } => codigo.to_string(),
                    EventoHttp::Ubicacion(u) => u.to_string(),
                    EventoHttp::Datos(b) => String::from_utf8_lossy(b).into_owned(),
                });
                Seguir::Si
            },
        );
        assert_eq!(r, Ok(()));
        assert_eq!(oido, ["200", "https://subida/1", "hola"]);
    }

    fn falso(codigo: u32, trozos: Vec<&'static str>, error: Option<&'static str>) -> (Adaptador, Arc<AtomicBool>) {
        let cancelado = Arc::new(AtomicBool::new(false));
        let app = Falso {
            codigo,
            trozos,
            error,
            cancelado: cancelado.clone(),
        };
        (Adaptador { app: Arc::new(app) }, cancelado)
    }

    fn pedido() -> Pedido<'static> {
        Pedido {
            url: "https://api.anthropic.com/v1/messages",
            cabeceras: vec![("x-api-key", "sk-ant-x".into())],
            cuerpo: b"{}",
        }
    }

    #[test]
    fn los_trozos_cruzan_en_orden_desde_otro_hilo() {
        let (t, cancelado) = falso(200, vec!["uno ", "dos ", "tres"], None);
        let mut oido = Vec::new();
        let r = t.enviar(&pedido(), &mut |e| {
            oido.push(match e {
                EventoHttp::Estado { codigo, esperar } => format!("{codigo}:{esperar:?}"),
                EventoHttp::Datos(b) => String::from_utf8_lossy(b).into_owned(),
                EventoHttp::Ubicacion(u) => u.to_string(),
            });
            Seguir::Si
        });
        assert_eq!(r, Ok(()));
        assert_eq!(oido, ["200:None", "uno ", "dos ", "tres"]);
        assert!(!cancelado.load(Ordering::Relaxed));
    }

    #[test]
    fn parar_le_dice_a_swift_que_cancele() {
        let (t, cancelado) = falso(200, vec!["a"; 50], None);
        let mut n = 0;
        let r = t.enviar(&pedido(), &mut |e| {
            if matches!(e, EventoHttp::Datos(_)) {
                n += 1;
                return Seguir::Parar;
            }
            Seguir::Si
        });
        assert_eq!(r, Ok(()));
        assert_eq!(n, 1, "tras parar no llega nada mas");
        // Swift lo vio al siguiente trozo (o al primero, si fue rapido).
        assert!(cancelado.load(Ordering::Relaxed));
    }

    #[test]
    fn el_error_de_red_y_el_retry_after_llegan() {
        let (t, _) = falso(0, vec![], Some("sin conexion"));
        assert_eq!(t.enviar(&pedido(), &mut |_| Seguir::Si), Err("sin conexion".into()));
        let (t, _) = falso(429, vec!["{}"], None);
        let mut espera = None;
        let r = t.enviar(&pedido(), &mut |e| {
            if let EventoHttp::Estado { esperar, .. } = e {
                espera = esperar;
            }
            Seguir::Si
        });
        assert_eq!(r, Ok(()));
        assert_eq!(espera, Some(Duration::from_secs(3)));
    }
}
