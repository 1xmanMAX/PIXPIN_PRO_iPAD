//! Las mini-apps que miden tiempo: el cronometro, el temporizador y la
//! alarma. Puerto de `mini/Tiempos.kt` de Android.
//!
//! ## Por que las tres juntas
//!
//! Porque son la misma cuenta mirada desde tres sitios. Un cronometro es
//! tiempo que sube desde un instante; un temporizador, tiempo que baja hacia
//! otro; y una alarma, un instante al que se llega. Las tres se guardan igual
//! —lineas de `- clave: valor` bajo el titulo— y las tres se leen igual.
//!
//! ## Lo que se guarda es el instante, no el numero que se ve
//!
//! Esta es **la** decision del fichero, y se copia tal cual. Un cronometro
//! corriendo no guarda «llevo 42 segundos»: guarda cuando se puso en marcha.
//! Guardar el numero obligaria a estar escribiendolo en disco todo el rato
//! para que sobreviviera a cerrar la aplicacion, y aun asi se quedaria parado
//! en cuanto el sistema matara el proceso. Con el instante, el numero se
//! calcula al mirarlo y **es correcto aunque nadie estuviera mirando**.
//!
//! Por eso aqui no hay reloj: el instante entra por parametro (`ahora`, en
//! milisegundos desde el epoch, como el `System.currentTimeMillis()` del
//! movil) y todo esto se comprueba sin reloj y sin ventana.

use super::{Resumen, documento_de_claves, otras_claves, valor, valores};

/// Un cronometro: lo que ya llevaba acumulado y desde cuando corre, si corre.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cronometro {
    /// Lo acumulado en las vueltas anteriores, en milisegundos.
    pub llevado: i64,
    /// Cuando se puso en marcha esta vez. `None` si esta parado.
    pub desde: Option<i64>,
    /// Las vueltas marcadas, en milisegundos desde el arranque.
    pub vueltas: Vec<i64>,
    /// Las claves que este PC no entiende, para devolverlas al guardar.
    pub otros: Vec<(String, String)>,
}

impl Cronometro {
    pub fn corriendo(&self) -> bool {
        self.desde.is_some()
    }

    /// Cuanto lleva ahora mismo, mirandolo en el instante `ahora`.
    pub fn transcurrido(&self, ahora: i64) -> i64 {
        self.llevado + self.desde.map_or(0, |d| (ahora - d).max(0))
    }
}

/// Un temporizador: cuanto se pidio y cuando termina, si esta en marcha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Temporizador {
    /// Lo que dura, en milisegundos. Es lo que se repite al volver a lanzarlo.
    pub duracion: i64,
    /// El instante en que suena. `None` si esta parado.
    pub fin_en: Option<i64>,
    /// Las claves que este PC no entiende, para devolverlas al guardar.
    pub otros: Vec<(String, String)>,
}

/// Cinco minutos, que es lo que dura casi todo lo que se pone a cocer
/// (`Tiempos.kt:48`).
pub const DURACION_POR_DEFECTO: i64 = 5 * 60 * 1000;

/// Mas de un dia no es un temporizador, es una cita: para eso esta la alarma.
pub const TOPE_DE_DURACION: i64 = 24 * 60 * 60 * 1000;

impl Default for Temporizador {
    fn default() -> Self {
        Self {
            duracion: DURACION_POR_DEFECTO,
            fin_en: None,
            otros: Vec::new(),
        }
    }
}

impl Temporizador {
    pub fn corriendo(&self) -> bool {
        self.fin_en.is_some()
    }

    /// Lo que falta. Cero cuando ya paso su hora.
    ///
    /// No se deja bajar de cero a proposito: un temporizador vencido tiene que
    /// decir «se acabo», no llevar la cuenta de lo tarde que vas.
    pub fn restante(&self, ahora: i64) -> i64 {
        match self.fin_en {
            Some(f) => (f - ahora).max(0),
            None => self.duracion,
        }
    }

    pub fn vencido(&self, ahora: i64) -> bool {
        self.fin_en.is_some_and(|f| ahora >= f)
    }
}

/// Una alarma: a que hora, y si esta puesta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alarma {
    pub hora: u8,
    pub minuto: u8,
    pub activa: bool,
    /// Las claves que este PC no entiende, para devolverlas al guardar.
    pub otros: Vec<(String, String)>,
}

impl Default for Alarma {
    fn default() -> Self {
        // Las ocho de la manana y apagada (`Tiempos.kt:68-70`): una alarma que
        // naciera sonando despertaria a quien solo queria mirarla.
        Self {
            hora: 8,
            minuto: 0,
            activa: false,
            otros: Vec::new(),
        }
    }
}

// ---- Cronometro ----------------------------------------------------------

/// Las claves del cronometro, para saber cual no lo es.
const CLAVES_CRONOMETRO: [&str; 3] = ["llevado", "desde", "vueltas"];

pub fn leer_cronometro(documento: &str) -> Cronometro {
    let v = valores(documento);
    Cronometro {
        llevado: valor(&v, "llevado")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
            .max(0),
        desde: valor(&v, "desde").and_then(|s| s.parse::<i64>().ok()),
        // Separadas por espacios, y lo que no sea un numero se cae: una
        // vuelta ilegible no puede llevarse las demas por delante.
        vueltas: valor(&v, "vueltas")
            .unwrap_or("")
            .split(' ')
            .filter_map(|t| t.parse::<i64>().ok())
            .collect(),
        otros: otras_claves(&v, &CLAVES_CRONOMETRO),
    }
}

pub fn escribir_cronometro(titulo: &str, c: &Cronometro) -> String {
    let mut pares = vec![("llevado".to_string(), c.llevado.to_string())];
    if let Some(d) = c.desde {
        pares.push(("desde".to_string(), d.to_string()));
    }
    // `vueltas` solo si las hay: una lista vacia dejaria una linea que no
    // dice nada, y el movil escribe el documento sin ella.
    if !c.vueltas.is_empty() {
        let lista = c
            .vueltas
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        pares.push(("vueltas".to_string(), lista));
    }
    pares.extend(c.otros.iter().cloned());
    documento_de_claves(titulo, &pares)
}

/// Arranca, si no estaba ya arrancado. Volver a arrancar no reinicia nada.
pub fn arrancar(c: &Cronometro, ahora: i64) -> Cronometro {
    if c.corriendo() {
        return c.clone();
    }
    Cronometro {
        desde: Some(ahora),
        ..c.clone()
    }
}

/// Para y **guarda lo llevado**: si no, parar seria reiniciar sin avisar.
pub fn parar(c: &Cronometro, ahora: i64) -> Cronometro {
    if !c.corriendo() {
        return c.clone();
    }
    Cronometro {
        llevado: c.transcurrido(ahora),
        desde: None,
        vueltas: c.vueltas.clone(),
        otros: c.otros.clone(),
    }
}

/// A cero. Se queda con las claves ajenas: lo que se reinicia es el
/// cronometro, no el documento de quien lo escribio.
pub fn reiniciar(c: &Cronometro) -> Cronometro {
    Cronometro {
        otros: c.otros.clone(),
        ..Cronometro::default()
    }
}

/// Marca una vuelta. Parado no marca nada: no hay nada que marcar.
pub fn vuelta(c: &Cronometro, ahora: i64) -> Cronometro {
    if !c.corriendo() {
        return c.clone();
    }
    let mut nuevo = c.clone();
    nuevo.vueltas.push(c.transcurrido(ahora));
    nuevo
}

/// Lo que ensena la burbuja: lo que llevaba **la ultima vez que se toco**.
///
/// No el tiempo vivo, y esta razonado en `MiniApps.kt:120-127`: la lista de la
/// conversacion no es un reloj y no puede repintarse sesenta veces por
/// segundo. Quien quiera verlo correr, lo abre.
pub fn resumen_cronometro(documento: &str) -> Resumen {
    let c = leer_cronometro(documento);
    Resumen {
        texto: como_se_lee_corto(c.llevado),
        de: c.vueltas.len(),
        vacia: c.llevado == 0 && !c.corriendo(),
        ..Resumen::default()
    }
}

// ---- Temporizador --------------------------------------------------------

/// Ojo con `finen`: se escribe `finEn` y se lee `finen`, porque la clave se
/// baja a minusculas al leer (`Tiempos.kt:84` frente a `:131`). Escribir
/// `finen` tambien lo leeria el movil, pero entonces los dos documentos no
/// serian el mismo texto y el resumen de sincronizacion cambiaria sin que
/// nadie tocara nada.
const CLAVES_TEMPORIZADOR: [&str; 2] = ["duracion", "finen"];

pub fn leer_temporizador(documento: &str) -> Temporizador {
    let v = valores(documento);
    Temporizador {
        duracion: valor(&v, "duracion")
            .and_then(|s| s.parse::<i64>().ok())
            .map(|d| d.max(0))
            .unwrap_or(DURACION_POR_DEFECTO),
        fin_en: valor(&v, "finen").and_then(|s| s.parse::<i64>().ok()),
        otros: otras_claves(&v, &CLAVES_TEMPORIZADOR),
    }
}

pub fn escribir_temporizador(titulo: &str, t: &Temporizador) -> String {
    let mut pares = vec![("duracion".to_string(), t.duracion.to_string())];
    if let Some(f) = t.fin_en {
        // Con F mayuscula al escribir, como el movil.
        pares.push(("finEn".to_string(), f.to_string()));
    }
    pares.extend(t.otros.iter().cloned());
    documento_de_claves(titulo, &pares)
}

/// Lo lanza desde `ahora`. Con duracion cero no se lanza: sonaria en el acto.
pub fn lanzar(t: &Temporizador, ahora: i64) -> Temporizador {
    if t.duracion <= 0 {
        return t.clone();
    }
    Temporizador {
        fin_en: Some(ahora + t.duracion),
        ..t.clone()
    }
}

pub fn detener(t: &Temporizador) -> Temporizador {
    Temporizador {
        fin_en: None,
        ..t.clone()
    }
}

/// Cambia lo que dura. Estando en marcha, ademas lo relanza con la nueva.
pub fn con_duracion(t: &Temporizador, duracion: i64, ahora: i64) -> Temporizador {
    let cambiado = Temporizador {
        duracion: duracion.clamp(0, TOPE_DE_DURACION),
        ..t.clone()
    };
    if t.corriendo() {
        lanzar(&cambiado, ahora)
    } else {
        cambiado
    }
}

pub fn resumen_temporizador(documento: &str) -> Resumen {
    let t = leer_temporizador(documento);
    Resumen {
        texto: como_se_lee_corto(t.duracion),
        vacia: !t.corriendo(),
        ..Resumen::default()
    }
}

// ---- Alarma --------------------------------------------------------------

const CLAVES_ALARMA: [&str; 2] = ["hora", "activa"];

/// Como se escribe que una alarma esta puesta: `sí`, **con tilde**.
///
/// Y al leer es una comparacion exacta contra esta misma palabra
/// (`Tiempos.kt:165`), asi que un PC que escribiera `si` sin tilde apagaria la
/// alarma en el movil sin decirlo. Por eso esta aqui como constante y no
/// escrita a mano en dos sitios.
pub const SI: &str = "sí";

/// Y como se escribe que no lo esta.
pub const NO: &str = "no";

pub fn leer_alarma(documento: &str) -> Alarma {
    let v = valores(documento);
    let hhmm = valor(&v, "hora").unwrap_or("");
    let mut trozos = hhmm.split(':');
    let hora = trozos
        .next()
        .and_then(|t| t.trim().parse::<i64>().ok())
        .map(|h| h.clamp(0, 23) as u8)
        .unwrap_or(8);
    let minuto = trozos
        .next()
        .and_then(|t| t.trim().parse::<i64>().ok())
        .map(|m| m.clamp(0, 59) as u8)
        .unwrap_or(0);
    Alarma {
        hora,
        minuto,
        activa: valor(&v, "activa") == Some(SI),
        otros: otras_claves(&v, &CLAVES_ALARMA),
    }
}

pub fn escribir_alarma(titulo: &str, a: &Alarma) -> String {
    let mut pares = vec![
        // La hora **sin** rellenar a dos cifras y el minuto **con**: asi lo
        // escribe el movil (`Tiempos.kt:172`), y `07:05` seria otro texto.
        ("hora".to_string(), format!("{}:{:02}", a.hora, a.minuto)),
        (
            "activa".to_string(),
            if a.activa { SI } else { NO }.to_string(),
        ),
    ];
    pares.extend(a.otros.iter().cloned());
    documento_de_claves(titulo, &pares)
}

pub fn resumen_alarma(documento: &str) -> Resumen {
    let a = leer_alarma(documento);
    Resumen {
        texto: format!("{}:{:02}", a.hora, a.minuto),
        vacia: !a.activa,
        ..Resumen::default()
    }
}

// ---- Como se leen los tiempos --------------------------------------------

/// `1:05,3` mientras es corto y `12:04:05` cuando hay horas.
///
/// Las decimas solo por debajo de una hora: en un cronometro de cocina son lo
/// que se mira, y en uno de tres horas son un digito que baila y no dice nada.
pub fn como_se_lee(ms: i64) -> String {
    let total = ms.max(0);
    let (horas, minutos, seg) = en_partes(total);
    if horas > 0 {
        format!("{horas}:{minutos:02}:{seg:02}")
    } else {
        format!("{minutos}:{seg:02},{}", (total % 1000) / 100)
    }
}

/// Sin decimas, para lo que no corre: una duracion elegida, una alarma.
pub fn como_se_lee_corto(ms: i64) -> String {
    let (horas, minutos, seg) = en_partes(ms.max(0));
    if horas > 0 {
        format!("{horas}:{minutos:02}:{seg:02}")
    } else {
        format!("{minutos}:{seg:02}")
    }
}

fn en_partes(ms: i64) -> (i64, i64, i64) {
    let segundos = ms / 1000;
    (segundos / 3600, (segundos % 3600) / 60, segundos % 60)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_cronometro_parado_guarda_lo_llevado_y_no_el_instante() {
        let c = arrancar(&Cronometro::default(), 1_000);
        assert_eq!(c.transcurrido(43_350), 42_350);
        let c = parar(&c, 43_350);
        assert!(!c.corriendo(), "parar suelta el instante");
        assert_eq!(c.llevado, 42_350);
        // Volver a arrancar suma, no reinicia.
        let c = arrancar(&c, 100_000);
        assert_eq!(c.transcurrido(101_000), 43_350);
    }

    #[test]
    fn el_documento_del_cronometro_se_escribe_como_en_el_movil() {
        let c = Cronometro {
            llevado: 42_350,
            desde: Some(1_758_351_600_000),
            vueltas: vec![10_250, 20_400, 33_900],
            otros: Vec::new(),
        };
        let d = escribir_cronometro("Tanda de series", &c);
        assert_eq!(
            d,
            "# Tanda de series\n\n- llevado: 42350\n- desde: 1758351600000\n- vueltas: 10250 20400 33900"
        );
        assert_eq!(leer_cronometro(&d), c, "ida y vuelta");
    }

    #[test]
    fn un_cronometro_parado_y_sin_vueltas_no_escribe_esas_lineas() {
        let d = escribir_cronometro("", &Cronometro::default());
        assert_eq!(d, "- llevado: 0", "sin titulo no hay cabecera");
        let c = leer_cronometro(&d);
        assert!(!c.corriendo() && c.vueltas.is_empty());
    }

    #[test]
    fn arrancar_dos_veces_no_pierde_el_arranque_y_parado_no_marca_vueltas() {
        let c = arrancar(&Cronometro::default(), 1_000);
        assert_eq!(arrancar(&c, 9_000).desde, Some(1_000));
        let parado = Cronometro::default();
        assert_eq!(vuelta(&parado, 5_000), parado, "parado no hay que marcar");
        assert_eq!(parar(&parado, 5_000), parado);
    }

    #[test]
    fn un_cronometro_roto_no_pierde_la_lista_ni_lo_que_no_se_entiende() {
        let d =
            "# T\n\n- llevado: -5\n- desde: hola\n- vueltas: 10 ochenta 30\n- ritmo: 3\nchachara";
        let c = leer_cronometro(d);
        assert_eq!(c.llevado, 0, "lo negativo se acota, no se cae");
        assert_eq!(c.desde, None, "un instante ilegible es estar parado");
        assert_eq!(
            c.vueltas,
            vec![10, 30],
            "la vuelta rota no se lleva las otras"
        );
        assert_eq!(c.otros, vec![("ritmo".to_string(), "3".to_string())]);
        // Y al guardar, la clave de la version futura sigue ahi.
        assert!(escribir_cronometro("T", &c).ends_with("- ritmo: 3"));
    }

    #[test]
    fn el_temporizador_escribe_finen_con_efe_mayuscula_y_lo_lee_en_minuscula() {
        let t = lanzar(
            &Temporizador {
                duracion: 420_000,
                ..Temporizador::default()
            },
            1_758_351_600_000,
        );
        let d = escribir_temporizador("Pasta", &t);
        assert_eq!(d, "# Pasta\n\n- duracion: 420000\n- finEn: 1758352020000");
        assert_eq!(leer_temporizador(&d), t, "escrito finEn, leido finen");
        // Y si llegara en minusculas, tambien se entiende.
        let otro = leer_temporizador("# Pasta\n\n- duracion: 420000\n- finen: 7");
        assert_eq!(otro.fin_en, Some(7));
    }

    #[test]
    fn el_temporizador_no_baja_de_cero_ni_se_lanza_vacio() {
        let t = leer_temporizador("- duracion: 1000\n- finEn: 5000");
        assert_eq!(t.restante(4_000), 1_000);
        assert_eq!(t.restante(9_999), 0, "vencido dice cero, no negativo");
        assert!(t.vencido(5_000));
        let vacio = Temporizador {
            duracion: 0,
            ..Temporizador::default()
        };
        assert!(!lanzar(&vacio, 1).corriendo(), "cero sonaria en el acto");
        // Cambiar la duracion estando en marcha relanza desde ahora.
        let cambiado = con_duracion(&t, 2_000, 10_000);
        assert_eq!(cambiado.fin_en, Some(12_000));
        assert_eq!(
            con_duracion(&t, i64::MAX, 0).duracion,
            TOPE_DE_DURACION,
            "mas de un dia es una cita, no un temporizador"
        );
    }

    #[test]
    fn un_temporizador_sin_documento_dura_cinco_minutos() {
        let t = leer_temporizador("");
        assert_eq!(t.duracion, DURACION_POR_DEFECTO);
        assert!(!t.corriendo());
        assert_eq!(t.restante(0), DURACION_POR_DEFECTO);
    }

    #[test]
    fn la_alarma_se_escribe_con_la_tilde_y_sin_rellenar_la_hora() {
        let a = Alarma {
            hora: 7,
            minuto: 5,
            activa: true,
            otros: Vec::new(),
        };
        let d = escribir_alarma("Despertar", &a);
        assert_eq!(d, "# Despertar\n\n- hora: 7:05\n- activa: sí");
        assert_eq!(leer_alarma(&d), a);
        assert!(
            escribir_alarma("D", &Alarma::default()).contains("- hora: 8:00\n- activa: no"),
            "asi nace"
        );
    }

    #[test]
    fn si_sin_tilde_no_enciende_la_alarma() {
        // Es lo que hace el movil, comparacion exacta: cualquier otra cosa es
        // «no». La prueba fija la trampa para que nadie la «arregle».
        assert!(!leer_alarma("- hora: 7:05\n- activa: si").activa);
        assert!(!leer_alarma("- hora: 7:05\n- activa: SÍ").activa);
        assert!(leer_alarma("- hora: 7:05\n- activa: sí").activa);
    }

    #[test]
    fn una_hora_imposible_se_acota_en_vez_de_caerse() {
        let a = leer_alarma("- hora: 99:88\n- activa: sí");
        assert_eq!((a.hora, a.minuto), (23, 59));
        let sin = leer_alarma("# Solo el titulo\n\nchachara");
        assert_eq!((sin.hora, sin.minuto, sin.activa), (8, 0, false));
        let rara = leer_alarma("- hora: pues no\n- activa: sí");
        assert_eq!((rara.hora, rara.minuto), (8, 0));
    }

    #[test]
    fn el_titulo_no_se_lee_como_un_dato() {
        // `# valor: 3` es un titulo, no una clave: si se leyera el cuerpo
        // entero, un contador con ese nombre arrancaria en tres.
        let a = leer_alarma("# hora: 9:30\n\n- hora: 7:05\n- activa: sí");
        assert_eq!((a.hora, a.minuto), (7, 5));
    }

    #[test]
    fn los_tiempos_se_leen_con_decimas_solo_cuando_son_cortos() {
        assert_eq!(como_se_lee(65_300), "1:05,3");
        assert_eq!(como_se_lee(43_445_000), "12:04:05");
        assert_eq!(como_se_lee(-5), "0:00,0", "lo negativo es cero");
        assert_eq!(como_se_lee_corto(65_300), "1:05");
        assert_eq!(como_se_lee_corto(DURACION_POR_DEFECTO), "5:00");
    }

    #[test]
    fn los_resumenes_dicen_lo_mismo_que_la_burbuja_del_movil() {
        let d = escribir_cronometro("T", &Cronometro::default());
        let r = super::resumen_cronometro(&d);
        assert_eq!(r.texto, "0:00");
        assert!(r.vacia);
        let corriendo = escribir_cronometro("T", &arrancar(&Cronometro::default(), 1));
        assert!(!super::resumen_cronometro(&corriendo).vacia);
        assert_eq!(
            resumen_temporizador(&escribir_temporizador("T", &Temporizador::default())).texto,
            "5:00"
        );
        let a = resumen_alarma("- hora: 7:05\n- activa: sí");
        assert_eq!(a.texto, "7:05");
        assert!(!a.vacia, "puesta no esta vacia");
    }
}
