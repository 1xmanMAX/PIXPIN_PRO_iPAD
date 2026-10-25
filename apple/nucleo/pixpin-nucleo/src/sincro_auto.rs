//! **Sincronizar solo, con la aplicacion abierta**: la decision de
//! `sincronizar/automatica.rs` de Windows, pura y sin red, para que el
//! `Sincronizador` de Swift haga lo mismo que el PC.
//!
//! - **Con quien**: los aparatos del grupo que contestan a la sonda. Con los
//!   demas no se intenta: cada intento con uno apagado cuesta agotar la
//!   conexion.
//! - **Cuando**: al aparecer uno en la red; al cambiar algo en los chats de
//!   aqui, tras [`QUIETO`] sin mas cambios; y cada [`PERIODO`] mientras haya
//!   alguno, que es como llega lo hecho en el movil (el movil no avisa).
//! - **Las tareas, al instante**: lo que toca tareas lo dice
//!   ([`RelojSincro::avisan_tareas`]) y, tras [`CALMA_TAREAS`] sin mas
//!   toques (o [`TOPE_TAREAS`] desde el primero), sale una vuelta **solo con
//!   esos chats** (`Nucleo::sincronizar_tareas`). Nunca a la par que otra.
//! - **Sin ruido**: se avisa del fallo solo al [`FALLOS_PARA_AVISAR`]º
//!   seguido, una vez.
//!
//! La unica diferencia con Windows es **como se ve un cambio aqui**: alli se
//! recorre la carpeta de los chats cada 15 s ([`firma_de`]); en el Mac, quien
//! escribe avisa (`pixpinCambioLocal`) y Swift pasa la cuenta de avisos como
//! huella ([`RelojSincro::ver_cambios`]). Asi nada sondea: Swift duerme lo
//! que diga [`RelojSincro::proximo_ms`] y despierta con cualquier aviso.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;

/// Lo que se espera sin mas toques en las tareas antes de mandarlas: lo
/// justo para que tres casillas seguidas viajen en una vuelta.
pub const CALMA_TAREAS: i64 = 600;
/// Y lo mas que se espera desde el primer toque, aunque no paren.
pub const TOPE_TAREAS: i64 = 1_500;
/// Lo que se espera sin cambios en los chats antes de mandarlos.
pub const QUIETO: i64 = 30_000;
/// Cada cuanto se sincroniza aunque aqui no cambie nada.
pub const PERIODO: i64 = 10 * 60_000;
/// A los cuantos fallos seguidos se avisa.
pub const FALLOS_PARA_AVISAR: u32 = 3;
/// Con tareas que ya tocan y otra vuelta en marcha, cada cuanto se vuelve a
/// mirar (`REINTENTO` de Windows).
pub const REINTENTO: i64 = 250;

/// Por que se sincroniza ahora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MotivoAuto {
    Aparecio,
    Cambio,
    Periodo,
}

/// Lo que toca hacer ahora ([`Reloj::siguiente`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum PasoAuto {
    Nada,
    /// La vuelta rapida, con las fichas de los chats cuyas tareas cambiaron.
    Tareas {
        fichas: Vec<String>,
    },
    /// La vuelta entera de siempre.
    Entera {
        motivo: MotivoAuto,
    },
    /// Habia tareas esperando y no contesta nadie: se olvidan, que la vuelta
    /// entera de cuando aparezca alguno ya las lleva.
    Olvidar,
}

/// **La huella de los chats**: cuantos ficheros, cuanto pesan y la suma de
/// cuando se tocaron. Cambia con cualquier cosa que se escriba.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Firma {
    pub ficheros: u64,
    pub bytes: u64,
    pub tocados: u128,
}

/// La huella de una carpeta, como Windows (aqui solo la usan las pruebas y
/// quien no tenga avisos de cambio).
pub fn firma_de(carpeta: &Path) -> Firma {
    let mut f = Firma::default();
    let mut pendientes = vec![carpeta.to_path_buf()];
    while let Some(d) = pendientes.pop() {
        let Ok(leer) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in leer.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                pendientes.push(e.path());
            } else {
                f.ficheros += 1;
                f.bytes += meta.len();
                f.tocados += meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis());
            }
        }
    }
    f
}

/// La huella hecha de la cuenta de avisos de cambio de Swift.
fn firma_de_cambios(n: u64) -> Firma {
    Firma {
        ficheros: n,
        ..Default::default()
    }
}

/// Lo que recuerda la automatica entre latido y latido (`Reloj` de Windows,
/// igual campo a campo).
#[derive(Debug, Default)]
pub struct Reloj {
    /// Quien contesto la ultima vez.
    pub responden: BTreeMap<String, bool>,
    /// Los que pasaron a contestar desde la ultima vuelta.
    pub aparecidos: BTreeSet<String>,
    /// La huella tras la ultima vuelta (o al empezar).
    pub base: Option<Firma>,
    /// La ultima huella vista y desde cuando esta asi.
    pub vista: Option<(Firma, i64)>,
    /// Cuando empezo la ultima vuelta automatica.
    pub ultima: Option<i64>,
    pub fallos_seguidos: u32,
    /// Las fichas con tareas cambiadas que aun no han salido.
    pub tareas: BTreeSet<String>,
    /// Cuando llego el primer aviso de tareas sin mandar, y el ultimo.
    pub tareas_desde: i64,
    pub tareas_ultimo: i64,
}

impl Reloj {
    /// Lo que dijo la sonda de un aparato.
    pub fn responde(&mut self, id: &str, si: bool) {
        let antes = self.responden.insert(id.to_string(), si).unwrap_or(false);
        if si && !antes {
            self.aparecidos.insert(id.to_string());
        }
        if !si {
            self.aparecidos.remove(id);
        }
    }

    /// Una huella nueva de los chats, vista `ahora`.
    pub fn ver(&mut self, firma: Firma, ahora: i64) {
        if self.base.is_none() {
            self.base = Some(firma);
        }
        if self.vista.is_none_or(|(f, _)| f != firma) {
            self.vista = Some((firma, ahora));
        }
    }

    /// Con quien se puede sincronizar ahora.
    pub fn con_quien(&self) -> Vec<String> {
        self.responden
            .iter()
            .filter(|(_, si)| **si)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// **Pura**: si toca sincronizar `ahora`, y por que.
    pub fn toca(&self, ahora: i64) -> Option<MotivoAuto> {
        if self.con_quien().is_empty() {
            return None;
        }
        if !self.aparecidos.is_empty() {
            return Some(MotivoAuto::Aparecio);
        }
        if let (Some(base), Some((vista, desde))) = (self.base, self.vista)
            && vista != base
            && ahora - desde >= QUIETO
        {
            return Some(MotivoAuto::Cambio);
        }
        if self.ultima.is_none_or(|u| ahora - u >= PERIODO) {
            return Some(MotivoAuto::Periodo);
        }
        None
    }

    /// Cambiaron las tareas del chat de `ficha`, `ahora`.
    pub fn avisan_tareas(&mut self, ficha: &str, ahora: i64) {
        if self.tareas.is_empty() {
            self.tareas_desde = ahora;
        }
        self.tareas.insert(ficha.to_string());
        self.tareas_ultimo = ahora;
    }

    /// Cuanto falta para mandar las tareas que esperan (0: ya), o `None` si
    /// no espera ninguna.
    pub fn espera_tareas(&self, ahora: i64) -> Option<i64> {
        if self.tareas.is_empty() {
            return None;
        }
        let calma = self.tareas_ultimo + CALMA_TAREAS;
        let tope = self.tareas_desde + TOPE_TAREAS;
        Some((calma.min(tope) - ahora).max(0))
    }

    /// **Pura**: lo que toca `ahora`. Con `ocupado` (una vuelta en marcha,
    /// nuestra o de otro que llamo) nada: lo que espera se queda y sale
    /// detras. Las tareas van antes que la vuelta entera, que tarda mas.
    pub fn siguiente(&self, ahora: i64, ocupado: bool) -> PasoAuto {
        if ocupado {
            return PasoAuto::Nada;
        }
        if self.espera_tareas(ahora) == Some(0) {
            return if self.con_quien().is_empty() {
                PasoAuto::Olvidar
            } else {
                PasoAuto::Tareas {
                    fichas: self.tareas.iter().cloned().collect(),
                }
            };
        }
        match self.toca(ahora) {
            Some(motivo) => PasoAuto::Entera { motivo },
            None => PasoAuto::Nada,
        }
    }

    /// Salieron (o se olvidan) las tareas que esperaban.
    pub fn tomar_tareas(&mut self) -> BTreeSet<String> {
        std::mem::take(&mut self.tareas)
    }

    /// Empezo una vuelta entera: lo que esperaba de tareas va dentro.
    pub fn empezo(&mut self, ahora: i64) {
        self.tareas.clear();
        self.ultima = Some(ahora);
        self.aparecidos.clear();
    }

    /// Acabo una vuelta, con la huella de despues (lo que escribio la propia
    /// vuelta no cuenta como cambio). Devuelve si hay que avisar del fallo.
    pub fn acabo(&mut self, bien: bool, firma: Firma, ahora: i64) -> bool {
        self.base = Some(firma);
        self.vista = Some((firma, ahora));
        if bien {
            self.fallos_seguidos = 0;
            false
        } else {
            self.fallos_seguidos += 1;
            self.fallos_seguidos == FALLOS_PARA_AVISAR
        }
    }

    /// **Cuanto dormir** sin mas avisos hasta que algo pueda tocar (lo que
    /// en Windows es el latido de 5 s, hecho cuenta para no sondear), o
    /// `None` si nada va a tocar solo. `corriendo`: una vuelta suya en
    /// marcha, que despierta al acabar.
    pub fn proximo(&self, ahora: i64, corriendo: bool) -> Option<i64> {
        if corriendo {
            return None;
        }
        let mut v: Vec<i64> = Vec::new();
        match self.espera_tareas(ahora) {
            // Toca ya y no se pudo (otra vuelta en marcha): pronto.
            Some(0) => v.push(REINTENTO),
            Some(ms) => v.push(ms),
            None => {}
        }
        if !self.con_quien().is_empty() {
            if !self.aparecidos.is_empty() {
                v.push(0);
            }
            if let (Some(base), Some((vista, desde))) = (self.base, self.vista)
                && vista != base
            {
                v.push((desde + QUIETO - ahora).max(0));
            }
            v.push(self.ultima.map_or(0, |u| (u + PERIODO - ahora).max(0)));
        }
        v.into_iter().min()
    }
}

/// **Cuantas rondas lleva «con todos»**: con dos aparatos o mas al otro
/// lado (tres o mas en el grupo), dos. En la primera este queda con todo,
/// pero los primeros se quedaron con lo de antes de hablar con los ultimos;
/// la segunda lo reparte, y cuesta poco porque solo viajan los cambios. Las
/// tareas al instante van en una: lo que corre es que lo de aqui llegue.
#[uniffi::export]
pub fn rondas_con_todos(aparatos: u32, solo_tareas: bool) -> u32 {
    if aparatos >= 2 && !solo_tareas { 2 } else { 1 }
}

/// Las constantes de la automatica, para Swift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TiemposAuto {
    pub quieto_ms: i64,
    pub periodo_ms: i64,
    pub calma_tareas_ms: i64,
    pub tope_tareas_ms: i64,
    pub fallos_para_avisar: u32,
}

#[uniffi::export]
pub fn tiempos_auto() -> TiemposAuto {
    TiemposAuto {
        quieto_ms: QUIETO,
        periodo_ms: PERIODO,
        calma_tareas_ms: CALMA_TAREAS,
        tope_tareas_ms: TOPE_TAREAS,
        fallos_para_avisar: FALLOS_PARA_AVISAR,
    }
}

/// **El [`Reloj`] para Swift**: lo guarda el `Sincronizador` y le pregunta
/// que toca. Todo es puro y rapido: se puede llamar en el hilo principal.
#[derive(uniffi::Object, Default)]
pub struct RelojSincro {
    r: Mutex<Reloj>,
}

impl RelojSincro {
    fn con<T>(&self, f: impl FnOnce(&mut Reloj) -> T) -> T {
        let mut g = self.r.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut g)
    }
}

#[uniffi::export]
impl RelojSincro {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::default())
    }

    /// Lo que dijo la sonda de un aparato del grupo.
    pub fn responde(&self, id: String, si: bool) {
        self.con(|r| r.responde(&id, si));
    }

    /// Ya no esta en la red (se fue de Bonjour): como si no contestara.
    pub fn olvidar(&self, id: String) {
        self.con(|r| {
            r.responden.remove(&id);
            r.aparecidos.remove(&id);
        });
    }

    /// La cuenta de avisos de cambio de aqui (`pixpinCambioLocal`), vista
    /// `ahora`: cuando deja de crecer [`QUIETO`] ms, toca la vuelta.
    pub fn ver_cambios(&self, cambios: u64, ahora: i64) {
        self.con(|r| r.ver(firma_de_cambios(cambios), ahora));
    }

    pub fn con_quien(&self) -> Vec<String> {
        self.con(|r| r.con_quien())
    }

    pub fn avisan_tareas(&self, ficha: String, ahora: i64) {
        self.con(|r| r.avisan_tareas(&ficha, ahora));
    }

    pub fn siguiente(&self, ahora: i64, ocupado: bool) -> PasoAuto {
        self.con(|r| r.siguiente(ahora, ocupado))
    }

    pub fn tomar_tareas(&self) -> Vec<String> {
        self.con(|r| r.tomar_tareas().into_iter().collect())
    }

    pub fn empezo(&self, ahora: i64) {
        self.con(|r| r.empezo(ahora));
    }

    /// Acabo una vuelta entera; `cambios` es la cuenta de avisos de ahora.
    /// Devuelve si hay que avisar del fallo (el tercero seguido, una vez).
    pub fn acabo(&self, bien: bool, cambios: u64, ahora: i64) -> bool {
        self.con(|r| r.acabo(bien, firma_de_cambios(cambios), ahora))
    }

    pub fn fallos_seguidos(&self) -> u32 {
        self.con(|r| r.fallos_seguidos)
    }

    /// Ver [`Reloj::proximo`].
    pub fn proximo_ms(&self, ahora: i64, corriendo: bool) -> Option<i64> {
        self.con(|r| r.proximo(ahora, corriendo))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn firma(n: u64) -> Firma {
        Firma {
            ficheros: n,
            bytes: n,
            tocados: n as u128,
        }
    }

    #[test]
    fn sin_nadie_que_conteste_no_se_sincroniza_nunca() {
        let mut r = Reloj::default();
        r.ver(firma(1), 0);
        r.ver(firma(2), 0);
        assert_eq!(r.toca(PERIODO * 5), None);
        r.responde("tel", false);
        assert_eq!(r.toca(PERIODO * 5), None, "caso negativo: contesto que no");
        assert_eq!(r.proximo(0, false), None, "y no se duerme esperando nada");
    }

    #[test]
    fn al_aparecer_un_aparato_se_sincroniza_enseguida_y_solo_una_vez() {
        let mut r = Reloj::default();
        r.ver(firma(1), 0);
        r.empezo(0);
        r.responde("tel", true);
        assert_eq!(r.toca(1_000), Some(MotivoAuto::Aparecio));
        r.empezo(1_000);
        r.acabo(true, firma(1), 2_000);
        r.responde("tel", true);
        assert_eq!(r.toca(3_000), None);
        r.responde("tel", false);
        r.responde("tel", true);
        assert_eq!(r.toca(4_000), Some(MotivoAuto::Aparecio));
    }

    #[test]
    fn un_cambio_aqui_espera_treinta_segundos_quieto() {
        let mut r = Reloj::default();
        r.responde("tel", true);
        r.ver(firma(1), 0);
        r.empezo(0);
        r.acabo(true, firma(1), 0);
        r.ver(firma(2), 1_000);
        assert_eq!(r.toca(1_000 + QUIETO - 1), None, "aun escribiendo");
        r.ver(firma(3), 20_000);
        assert_eq!(r.toca(1_000 + QUIETO + 1), None);
        assert_eq!(r.toca(20_000 + QUIETO), Some(MotivoAuto::Cambio));
        r.empezo(50_000);
        r.acabo(true, firma(9), 60_000);
        assert_eq!(r.toca(60_000 + QUIETO * 2), None);
    }

    #[test]
    fn cada_diez_minutos_aunque_no_cambie_nada() {
        let mut r = Reloj::default();
        r.responde("tel", true);
        r.ver(firma(1), 0);
        assert_eq!(r.toca(0), Some(MotivoAuto::Aparecio));
        r.empezo(0);
        r.acabo(true, firma(1), 0);
        assert_eq!(r.toca(PERIODO - 1), None);
        assert_eq!(r.toca(PERIODO), Some(MotivoAuto::Periodo));
    }

    #[test]
    fn se_avisa_del_fallo_una_vez_al_tercero_seguido_y_un_acierto_lo_borra() {
        let mut r = Reloj::default();
        assert!(!r.acabo(false, firma(1), 0));
        assert!(!r.acabo(false, firma(1), 0));
        assert!(r.acabo(false, firma(1), 0), "el tercero avisa");
        assert!(
            !r.acabo(false, firma(1), 0),
            "caso negativo: el cuarto ya no"
        );
        assert!(!r.acabo(true, firma(1), 0));
        assert_eq!(r.fallos_seguidos, 0);
    }

    #[test]
    fn la_firma_cambia_al_escribir_un_fichero() {
        let d = std::env::temp_dir().join(format!("pp-mac-firma-{}", std::process::id()));
        std::fs::create_dir_all(d.join("chat")).unwrap();
        let a = firma_de(&d);
        std::fs::write(d.join("chat").join("x.txt"), b"hola").unwrap();
        let b = firma_de(&d);
        assert_ne!(a, b);
        assert_eq!(b.ficheros, a.ficheros + 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    fn con_tel() -> Reloj {
        let mut r = Reloj::default();
        r.responde("tel", true);
        r.ver(firma(1), 0);
        r.empezo(0);
        r.acabo(true, firma(1), 0);
        r
    }

    fn fichas(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn una_tarea_tachada_sale_en_cuanto_se_calma_y_no_antes() {
        let mut r = con_tel();
        assert_eq!(r.siguiente(1_000, false), PasoAuto::Nada);
        r.avisan_tareas("obra", 1_000);
        assert_eq!(r.espera_tareas(1_000), Some(CALMA_TAREAS));
        assert_eq!(
            r.siguiente(1_000 + CALMA_TAREAS - 1, false),
            PasoAuto::Nada,
            "aun tachando"
        );
        assert_eq!(
            r.siguiente(1_000 + CALMA_TAREAS, false),
            PasoAuto::Tareas {
                fichas: fichas(&["obra"])
            }
        );
        assert_eq!(
            r.tomar_tareas().into_iter().collect::<Vec<_>>(),
            fichas(&["obra"])
        );
        assert_eq!(
            r.siguiente(1_000 + CALMA_TAREAS, false),
            PasoAuto::Nada,
            "caso negativo: ya salio"
        );
        assert_eq!(r.espera_tareas(5_000), None);
    }

    #[test]
    fn varias_casillas_seguidas_van_en_una_vuelta_y_el_tope_no_deja_esperar_de_mas() {
        let mut r = con_tel();
        for (i, t) in [0, 400, 800, 1_200].into_iter().enumerate() {
            r.avisan_tareas(if i % 2 == 0 { "obra" } else { "guardados" }, 10_000 + t);
            if 10_000 + t < 10_000 + TOPE_TAREAS {
                assert_eq!(r.siguiente(10_000 + t, false), PasoAuto::Nada);
            }
        }
        assert_eq!(r.siguiente(10_000 + TOPE_TAREAS - 1, false), PasoAuto::Nada);
        assert_eq!(
            r.siguiente(10_000 + TOPE_TAREAS, false),
            PasoAuto::Tareas {
                fichas: fichas(&["guardados", "obra"])
            },
            "dos chats en la misma vuelta"
        );
    }

    #[test]
    fn con_otra_vuelta_en_marcha_las_tareas_esperan_y_salen_detras() {
        let mut r = con_tel();
        r.avisan_tareas("obra", 0);
        assert_eq!(r.siguiente(5_000, true), PasoAuto::Nada, "nunca a la par");
        r.avisan_tareas("guardados", 5_100);
        assert_eq!(r.siguiente(5_200, true), PasoAuto::Nada);
        assert_eq!(
            r.siguiente(PERIODO + 1, false),
            PasoAuto::Tareas {
                fichas: fichas(&["guardados", "obra"])
            }
        );
        r.tomar_tareas();
        assert_eq!(
            r.siguiente(PERIODO + 1, false),
            PasoAuto::Entera {
                motivo: MotivoAuto::Periodo
            }
        );
    }

    #[test]
    fn caso_negativo_sin_nadie_que_conteste_las_tareas_se_olvidan_sin_salir() {
        let mut r = Reloj::default();
        r.avisan_tareas("obra", 0);
        assert_eq!(r.siguiente(CALMA_TAREAS, false), PasoAuto::Olvidar);
        r.responde("tel", false);
        assert_eq!(r.siguiente(CALMA_TAREAS, false), PasoAuto::Olvidar);
        r.tomar_tareas();
        assert_eq!(r.siguiente(CALMA_TAREAS, false), PasoAuto::Nada);
    }

    #[test]
    fn la_vuelta_entera_se_lleva_lo_que_esperaba_de_tareas() {
        let mut r = con_tel();
        r.responde("tel", false);
        r.responde("tel", true);
        r.avisan_tareas("obra", 100);
        assert_eq!(
            r.siguiente(200, false),
            PasoAuto::Entera {
                motivo: MotivoAuto::Aparecio
            }
        );
        r.empezo(200);
        assert_eq!(r.espera_tareas(10_000), None);
    }

    #[test]
    fn se_duerme_lo_justo_para_lo_primero_que_toque() {
        // `cuanto_dormir` de Windows, sin latido: la cuenta exacta.
        let mut r = con_tel();
        assert_eq!(r.proximo(0, false), Some(PERIODO), "sin nada: el periodo");
        r.avisan_tareas("obra", 0);
        assert_eq!(r.proximo(100, false), Some(CALMA_TAREAS - 100));
        assert_eq!(
            r.proximo(CALMA_TAREAS, false),
            Some(REINTENTO),
            "ocupado: reintentar pronto"
        );
        assert_eq!(
            r.proximo(0, true),
            None,
            "con una vuelta suya en marcha, la despierta su final"
        );
        r.tomar_tareas();
        r.ver(firma(5), 1_000);
        assert_eq!(r.proximo(1_000, false), Some(QUIETO), "un cambio: la calma");
    }

    #[test]
    fn dos_rondas_con_tres_aparatos_o_mas_y_una_con_las_tareas() {
        assert_eq!(rondas_con_todos(1, false), 1);
        assert_eq!(rondas_con_todos(2, false), 2);
        assert_eq!(rondas_con_todos(5, false), 2);
        assert_eq!(rondas_con_todos(3, true), 1, "las tareas, una ronda");
        assert_eq!(rondas_con_todos(0, false), 1);
    }

    #[test]
    fn el_reloj_de_swift_cuenta_los_avisos_como_huella() {
        let r = RelojSincro::new();
        r.responde("tel".into(), true);
        r.ver_cambios(0, 0);
        r.empezo(0);
        assert!(!r.acabo(true, 0, 0));
        r.ver_cambios(1, 1_000);
        assert_eq!(r.siguiente(1_000 + QUIETO - 1, false), PasoAuto::Nada);
        assert_eq!(
            r.siguiente(1_000 + QUIETO, false),
            PasoAuto::Entera {
                motivo: MotivoAuto::Cambio
            }
        );
        // Se fue de la red: ya no hay con quien.
        r.olvidar("tel".into());
        assert!(r.con_quien().is_empty());
        assert_eq!(r.siguiente(1_000 + QUIETO, false), PasoAuto::Nada);
    }
}
