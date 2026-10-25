//! El texto con sus minutos: `[1:23] lo que se dijo`.
//!
//! Es el formato exacto que el movil guarda en `Mensaje.transcripcion`
//! (`Transcriptor.conTiempos`, `Transcriptor.kt:66-87`) y el que lee de
//! vuelta para saltar al minuto tocando una linea (`tiempoDe` :91-97). Se
//! copia al caracter porque el campo **viaja por la sincronizacion**: un PC
//! que escriba `[01:23]` con cero delante deja al movil sin poder saltar.
//!
//! Todo lo de aqui es aritmetica y cadenas: se prueba entero sin sonido.

/// Un trozo de texto y en que milisegundo del audio empieza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segmento {
    pub desde_ms: i64,
    pub texto: String,
}

impl Segmento {
    pub fn nuevo(desde_ms: i64, texto: impl Into<String>) -> Segmento {
        Segmento {
            desde_ms,
            texto: texto.into(),
        }
    }
}

/// Cuanto abarca un parrafo del texto con tiempos, como mucho.
///
/// `PARRAFO_MS` (`Transcriptor.kt:100`). Ni frase a frase —una lista de
/// renglones sueltos no se lee— ni de corrido: veinte segundos es un sitio
/// al que saltar.
pub const PARRAFO_MS: i64 = 20_000;

/// Como quedo el texto de una nota de voz.
///
/// Es `Mensaje.estadoDelTexto` (`Mensajes.kt:590-594`), que **no** esta
/// declarado en el `Mensaje` de Rust y por eso viaja en `resto`. Se guarda
/// la palabra tal cual la escribe el movil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDelTexto {
    /// Salio entero y sin huecos.
    Bien,
    /// Hay texto, pero algun trozo no se entendio: tiene agujeros.
    Aviso,
    /// No salio nada aprovechable.
    Mal,
    /// El valor especial: esto no es una nota, es musica, y lo escrito es
    /// su letra. Lo pone el usuario a mano, nunca el reconocedor.
    Letra,
}

impl EstadoDelTexto {
    /// La palabra que se escribe en el cuaderno. Constantes de
    /// `Mensajes.kt:590-594`.
    pub fn palabra(self) -> &'static str {
        match self {
            EstadoDelTexto::Bien => "bien",
            EstadoDelTexto::Aviso => "aviso",
            EstadoDelTexto::Mal => "mal",
            EstadoDelTexto::Letra => "letra",
        }
    }

    /// Lo contrario: leer lo que venga del movil. Una palabra desconocida
    /// da `None` a proposito, igual que `MiniApp.de`: una version futura de
    /// Android puede inventarse un estado y eso no puede tumbar nada.
    pub fn de(palabra: &str) -> Option<EstadoDelTexto> {
        Some(match palabra {
            "bien" => EstadoDelTexto::Bien,
            "aviso" => EstadoDelTexto::Aviso,
            "mal" => EstadoDelTexto::Mal,
            "letra" => EstadoDelTexto::Letra,
            _ => return None,
        })
    }
}

/// En que estado queda un texto con `trozos` frases aprovechadas y `avisos`
/// trozos que no se entendieron.
///
/// Sin nada aprovechado es `Mal` aunque no hubiera ni un aviso: una nota de
/// puro silencio no «salio bien».
pub fn estado_de(trozos: usize, avisos: usize) -> EstadoDelTexto {
    if trozos == 0 {
        EstadoDelTexto::Mal
    } else if avisos == 0 {
        EstadoDelTexto::Bien
    } else {
        EstadoDelTexto::Aviso
    }
}

/// `1:23`, `12:05`, `1:02:09`.
///
/// Sin cero delante en la primera cifra y con dos cifras en las demas,
/// exactamente como `marcaDeTiempo` (`Transcriptor.kt:89-93`). Un tiempo
/// negativo —que no deberia llegar, pero llega de un JSON tocado a mano— se
/// trata como cero en vez de escribir `-1:-5`.
pub fn marca_de_tiempo(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let seg = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{seg:02}")
    } else {
        format!("{m}:{seg:02}")
    }
}

/// El texto con sus tiempos, una linea por parrafo, separadas por un
/// renglon en blanco.
///
/// Los segmentos se juntan en parrafos de hasta [`PARRAFO_MS`]. El
/// `prefijo` es lo que la conversacion por turnos mete delante
/// (`**Nombre:** `); en una nota normal va vacio.
pub fn con_tiempos(segmentos: &[Segmento], prefijo: &str) -> String {
    let mut lineas: Vec<String> = Vec::new();
    let mut desde: i64 = -1;
    let mut trozo = String::new();

    for sg in segmentos {
        if desde >= 0 && sg.desde_ms - desde >= PARRAFO_MS {
            lineas.push(format!(
                "[{}] {prefijo}{}",
                marca_de_tiempo(desde),
                trozo.trim()
            ));
            trozo.clear();
            desde = -1;
        }
        if desde < 0 {
            desde = sg.desde_ms;
        }
        trozo.push_str(&sg.texto);
        trozo.push(' ');
    }
    if !trozo.trim().is_empty() {
        lineas.push(format!(
            "[{}] {prefijo}{}",
            marca_de_tiempo(desde.max(0)),
            trozo.trim()
        ));
    }
    lineas.join("\n\n")
}

/// El milisegundo con el que empieza una linea, y la linea sin la marca.
///
/// `None` si no lleva marca: es lo que distingue una transcripcion con
/// tiempos de un texto cualquiera. Es la vuelta de [`con_tiempos`] y lo que
/// permite saltar al minuto pulsando un renglon (`tiempoDe` :91-97).
pub fn tiempo_de(linea: &str) -> Option<(i64, &str)> {
    let recortada = linea.trim_start();
    let resto = recortada.strip_prefix('[')?;
    let (dentro, despues) = resto.split_once(']')?;

    // Las piezas son `h:mm:ss` o `m:ss`. Se parten a mano y no con una
    // expresion regular para no arrastrar `regex` a un crate de cimientos
    // por dos puntos y dos numeros.
    let mut partes = dentro.split(':');
    let a = partes.next()?;
    let b = partes.next()?;
    let c = partes.next();
    if partes.next().is_some() {
        return None;
    }

    let (h, m, s) = match c {
        Some(c) => (numero(a)?, dos_cifras(b)?, dos_cifras(c)?),
        None => (0, numero(a)?, dos_cifras(b)?),
    };
    if m > 59 || s > 59 {
        return None;
    }
    Some((
        (h * 3600 + m * 60 + s) * 1000,
        despues.strip_prefix(' ').unwrap_or(despues),
    ))
}

/// Un numero de una o mas cifras, sin signo ni blancos.
fn numero(t: &str) -> Option<i64> {
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// Exactamente dos cifras: `[1:5]` no es una marca valida, y admitirla
/// haria que un texto que empieza por `[1:5]` (una cita, una nota) se
/// confundiera con una transcripcion.
fn dos_cifras(t: &str) -> Option<i64> {
    if t.len() != 2 {
        return None;
    }
    numero(t)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_marca_no_rellena_la_primera_cifra_y_si_las_demas() {
        assert_eq!(marca_de_tiempo(0), "0:00");
        assert_eq!(marca_de_tiempo(83_000), "1:23");
        assert_eq!(marca_de_tiempo(725_000), "12:05");
        assert_eq!(marca_de_tiempo(3_729_000), "1:02:09");
    }

    #[test]
    fn un_tiempo_negativo_no_escribe_una_marca_al_reves() {
        assert_eq!(marca_de_tiempo(-5_000), "0:00");
    }

    #[test]
    fn las_frases_seguidas_se_juntan_en_un_solo_parrafo() {
        let segmentos = [
            Segmento::nuevo(1_000, "hola"),
            Segmento::nuevo(2_500, "que tal"),
        ];
        assert_eq!(con_tiempos(&segmentos, ""), "[0:01] hola que tal");
    }

    #[test]
    fn a_los_veinte_segundos_empieza_un_parrafo_nuevo() {
        let segmentos = [
            Segmento::nuevo(0, "uno"),
            Segmento::nuevo(PARRAFO_MS, "dos"),
        ];
        assert_eq!(con_tiempos(&segmentos, ""), "[0:00] uno\n\n[0:20] dos");
    }

    #[test]
    fn el_prefijo_de_los_turnos_va_detras_de_la_marca() {
        let segmentos = [Segmento::nuevo(0, "buenos dias")];
        assert_eq!(
            con_tiempos(&segmentos, "**Ana:** "),
            "[0:00] **Ana:** buenos dias"
        );
    }

    #[test]
    fn sin_segmentos_o_con_segmentos_en_blanco_no_se_escribe_ninguna_linea() {
        assert_eq!(con_tiempos(&[], ""), "");
        let vacios = [Segmento::nuevo(0, "   "), Segmento::nuevo(100, "")];
        assert_eq!(con_tiempos(&vacios, ""), "");
    }

    #[test]
    fn leer_la_marca_devuelve_el_milisegundo_y_la_linea_pelada() {
        assert_eq!(
            tiempo_de("[1:23] lo que se dijo"),
            Some((83_000, "lo que se dijo"))
        );
        assert_eq!(tiempo_de("[1:02:09] tarde"), Some((3_729_000, "tarde")));
    }

    #[test]
    fn una_linea_sin_marca_o_con_una_marca_falsa_no_se_confunde_con_un_tiempo() {
        assert_eq!(tiempo_de("lo que se dijo"), None);
        // Una cita numerada al principio de un texto cualquiera.
        assert_eq!(tiempo_de("[1:5] versiculo"), None);
        assert_eq!(tiempo_de("[a:bc] nada"), None);
        assert_eq!(tiempo_de("[1:99] minuto imposible"), None);
        assert_eq!(tiempo_de("[1:2:3:4] demasiado"), None);
        assert_eq!(tiempo_de("[] vacio"), None);
    }

    #[test]
    fn escribir_y_volver_a_leer_da_el_mismo_tiempo() {
        let segmentos = [Segmento::nuevo(0, "uno"), Segmento::nuevo(3_729_000, "dos")];
        let cuerpo = con_tiempos(&segmentos, "");
        let leidos: Vec<i64> = cuerpo
            .split("\n\n")
            .filter_map(|l| tiempo_de(l).map(|(ms, _)| ms))
            .collect();
        assert_eq!(leidos, [0, 3_729_000]);
    }

    #[test]
    fn el_estado_del_texto_distingue_lo_entero_lo_agujereado_y_lo_perdido() {
        assert_eq!(estado_de(3, 0), EstadoDelTexto::Bien);
        assert_eq!(estado_de(3, 1), EstadoDelTexto::Aviso);
        assert_eq!(estado_de(0, 0), EstadoDelTexto::Mal);
        assert_eq!(estado_de(0, 4), EstadoDelTexto::Mal);
    }

    #[test]
    fn el_estado_se_escribe_y_se_lee_con_las_palabras_del_movil() {
        assert_eq!(EstadoDelTexto::Bien.palabra(), "bien");
        assert_eq!(EstadoDelTexto::Letra.palabra(), "letra");
        assert_eq!(EstadoDelTexto::de("aviso"), Some(EstadoDelTexto::Aviso));
        // Una palabra de una version futura de Android no tumba nada.
        assert_eq!(EstadoDelTexto::de("regular"), None);
        assert_eq!(EstadoDelTexto::de(""), None);
    }
}
