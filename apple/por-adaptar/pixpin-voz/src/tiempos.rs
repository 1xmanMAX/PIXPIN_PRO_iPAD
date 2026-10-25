//! El texto con sus minutos: `[1:23] lo que se dijo`.
//!
//! Es el formato exacto que el movil guarda en `Mensaje.transcripcion`
//! (`Transcriptor.conTiempos`, `Transcriptor.kt:66-87`) y el que lee de
//! vuelta para saltar al minuto tocando una linea (`tiempoDe` :91-97). Se
//! copia al caracter porque el campo **viaja por la sincronizacion**: un PC
//! que escriba `[01:23]` con cero delante deja al movil sin poder saltar.
//!
//! Todo lo de aqui es aritmetica y cadenas: se prueba entero sin sonido.

/// Un trozo de texto, en que milisegundo del audio empieza y en cual acaba.
///
/// El trozo puede ser una palabra (Vosk), una frase (el reconocedor de
/// Windows), una tanda (Whisper, Qwen3-ASR) o una linea ya juntada por
/// [`agrupar`]. `hasta_ms` igual a `desde_ms` quiere decir que no se sabe
/// cuando acaba: se le trata como un instante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segmento {
    pub desde_ms: i64,
    pub hasta_ms: i64,
    pub texto: String,
}

impl Segmento {
    /// Un trozo del que solo se sabe cuando empieza.
    pub fn nuevo(desde_ms: i64, texto: impl Into<String>) -> Segmento {
        Segmento::entre(desde_ms, desde_ms, texto)
    }

    /// Un trozo con principio y fin. Un fin antes del principio (un tiempo
    /// tocado a mano) se queda en el principio.
    pub fn entre(desde_ms: i64, hasta_ms: i64, texto: impl Into<String>) -> Segmento {
        Segmento {
            desde_ms,
            hasta_ms: hasta_ms.max(desde_ms),
            texto: texto.into(),
        }
    }

    /// El mismo trozo corrido `ms` (un turno que empieza a mitad del audio).
    pub fn corrido(self, ms: i64) -> Segmento {
        Segmento {
            desde_ms: self.desde_ms + ms,
            hasta_ms: self.hasta_ms + ms,
            texto: self.texto,
        }
    }
}

/// Cuanto abarca una linea del texto con tiempos, como mucho.
///
/// **Actualizado el 2026-10-25, por peticion del usuario**: antes eran los
/// 20 s del movil (`PARRAFO_MS`, `Transcriptor.kt:100`) y una clase de 27 s
/// salio en un solo bloque `[0:00]`. Ahora hay una marca cada 15 s como
/// mucho: ni frase a frase —una lista de renglones sueltos no se lee— ni de
/// corrido. El formato `[m:ss]` es el mismo, asi que el movil lo lee igual.
pub const PARRAFO_MS: i64 = 15_000;

/// Lo que dura como poco una linea antes de partirla por un punto: por
/// debajo de 5 s, una frase corta se queda con la siguiente.
pub const PARRAFO_MINIMO_MS: i64 = 5_000;

/// Un silencio entre dos trozos de mas de esto es buen sitio para partir.
const PAUSA_MS: i64 = 700;

/// **Junta los trozos en lineas de hasta `max_ms`**, partiendo donde mejor
/// se lea.
///
/// Se van sumando trozos a la linea; cuando el siguiente la haria pasar de
/// `max_ms` (de donde empieza la linea a donde acaba el trozo), se parte.
/// No se parte a ciegas delante de ese trozo: se busca, entre los sitios de
/// la linea que dejan al menos `min_ms` detras, **el mejor**: tras un punto
/// (`.`, `?`, `!`, `…`), si no tras un silencio largo, si no tras una coma;
/// a igualdad, el mas tardio. Solo si no hay ninguno se parte delante del
/// trozo nuevo. Un trozo que solo ya pasa de `max_ms` va en su linea.
///
/// Los trozos en blanco no cuentan y ninguna linea sale vacia. Pura: es lo
/// que comparten todos los motores.
pub fn agrupar(trozos: &[Segmento], max_ms: i64, min_ms: i64) -> Vec<Segmento> {
    let mut lineas: Vec<Segmento> = Vec::new();
    let mut linea: Vec<Segmento> = Vec::new();
    for t in trozos {
        let texto = t.texto.trim();
        if texto.is_empty() {
            continue;
        }
        let t = Segmento::entre(t.desde_ms, t.hasta_ms, texto);
        while let Some(primero) = linea.first() {
            if t.hasta_ms - primero.desde_ms <= max_ms {
                break;
            }
            let corte = mejor_corte(&linea, min_ms).unwrap_or(linea.len());
            let resto = linea.split_off(corte);
            lineas.push(juntar(&linea));
            linea = resto;
        }
        linea.push(t);
    }
    if !linea.is_empty() {
        lineas.push(juntar(&linea));
    }
    lineas
}

/// Donde partir `linea` (el indice del primer trozo de la siguiente), o
/// `None` si no hay ningun sitio bueno que deje `min_ms` detras.
fn mejor_corte(linea: &[Segmento], min_ms: i64) -> Option<usize> {
    let desde = linea.first()?.desde_ms;
    let mut mejor: Option<(u8, usize)> = None;
    for k in 1..linea.len() {
        if linea[k].desde_ms - desde < min_ms {
            continue;
        }
        let antes = &linea[k - 1];
        let nota = if acaba_frase(&antes.texto) {
            3
        } else if linea[k].desde_ms - antes.hasta_ms >= PAUSA_MS {
            2
        } else if antes.texto.ends_with([',', ';', ':']) {
            1
        } else {
            0
        };
        if nota > 0 && mejor.is_none_or(|(n, _)| nota >= n) {
            mejor = Some((nota, k));
        }
    }
    mejor.map(|(_, k)| k)
}

/// Si un texto acaba una frase: punto, interrogacion, exclamacion o
/// puntos suspensivos, aunque detras vaya una comilla o un parentesis.
fn acaba_frase(texto: &str) -> bool {
    texto
        .trim_end_matches(['"', '\'', ')', '»', '”', '’'])
        .ends_with(['.', '?', '!', '…', '。', '？', '！'])
}

/// Una linea de varios trozos: empieza con el primero, acaba con el que
/// mas tarde acabe, y el texto va separado por un blanco.
fn juntar(linea: &[Segmento]) -> Segmento {
    let desde = linea.first().map_or(0, |s| s.desde_ms);
    let hasta = linea.iter().map(|s| s.hasta_ms).max().unwrap_or(desde);
    let texto = linea
        .iter()
        .map(|s| s.texto.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Segmento::entre(desde, hasta, texto)
}

/// **Las palabras de una frase de la que solo se sabe principio y fin**,
/// con su tiempo repartido segun lo largas que son.
///
/// Es para el reconocedor de Windows, que da cada frase con su inicio y lo
/// que dura pero no el tiempo de cada palabra: una frase de 40 s sin pausa
/// tiene que poder partirse en lineas de 15 s. El reparto por letras es una
/// estimacion (se habla mas o menos a ritmo constante); el `[m:ss]` de cada
/// linea cae a uno o dos segundos de lo dicho, que para saltar al minuto
/// basta.
pub fn repartir(desde_ms: i64, hasta_ms: i64, texto: &str) -> Vec<Segmento> {
    let palabras: Vec<&str> = texto.split_whitespace().collect();
    let letras: usize = palabras.iter().map(|p| p.chars().count()).sum();
    if letras == 0 {
        return Vec::new();
    }
    let dura = (hasta_ms - desde_ms).max(0);
    let mut hechas = 0usize;
    palabras
        .iter()
        .map(|p| {
            let a = desde_ms + dura * hechas as i64 / letras as i64;
            hechas += p.chars().count();
            let b = desde_ms + dura * hechas as i64 / letras as i64;
            Segmento::entre(a, b, *p)
        })
        .collect()
}

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

/// **Lo que aporta una frase** de la que se sabe principio y fin: ella
/// entera si cabe en una linea ([`PARRAFO_MS`]), con su minuto exacto; si
/// no, sus palabras con el tiempo repartido ([`repartir`]), para que
/// [`agrupar`] la parta en lineas de 15 s.
pub fn trozos_de_frase(desde_ms: i64, hasta_ms: i64, texto: &str) -> Vec<Segmento> {
    if hasta_ms - desde_ms <= PARRAFO_MS {
        let texto = texto.trim();
        if texto.is_empty() {
            return Vec::new();
        }
        return vec![Segmento::entre(desde_ms, hasta_ms, texto)];
    }
    repartir(desde_ms, hasta_ms, texto)
}

/// Los trozos juntados en lineas de hasta [`PARRAFO_MS`] (ver [`agrupar`]):
/// lo que guardan los motores en [`Transcripcion::segmentos`], uno por
/// linea del texto.
pub fn lineas(trozos: &[Segmento]) -> Vec<Segmento> {
    agrupar(trozos, PARRAFO_MS, PARRAFO_MINIMO_MS)
}

/// El texto con sus tiempos, una linea por parrafo, separadas por un
/// renglon en blanco.
///
/// Los segmentos se juntan en lineas de hasta [`PARRAFO_MS`] con
/// [`agrupar`]. El `prefijo` es lo que la conversacion por turnos mete
/// delante (`**Nombre:** `); en una nota normal va vacio.
pub fn con_tiempos(segmentos: &[Segmento], prefijo: &str) -> String {
    lineas(segmentos)
        .iter()
        .map(|l| format!("[{}] {prefijo}{}", marca_de_tiempo(l.desde_ms), l.texto))
        .collect::<Vec<_>>()
        .join("\n\n")
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

/// El resultado de pasar una nota a texto: justo lo que hay que escribir en
/// el mensaje del cuaderno.
#[derive(Debug, Clone, PartialEq)]
pub struct Transcripcion {
    /// El cuerpo con marcas, `[1:23] lo que se dijo`, separado por renglon
    /// en blanco. Va a `Mensaje.transcripcion`.
    pub texto: String,
    /// Los trozos sueltos, por si quien llama quiere pintarlos.
    pub segmentos: Vec<Segmento>,
    /// Va a `resto["estadoDelTexto"]`.
    pub estado: EstadoDelTexto,
    pub duracion_ms: i64,
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
    fn a_los_quince_segundos_empieza_un_parrafo_nuevo() {
        let segmentos = [
            Segmento::nuevo(0, "uno"),
            Segmento::nuevo(PARRAFO_MS + 1, "dos"),
        ];
        assert_eq!(con_tiempos(&segmentos, ""), "[0:00] uno\n\n[0:15] dos");
    }

    /// Un monologo de `n` palabras de 400 ms seguidas, una frase cada
    /// `cada` palabras (la ultima de cada frase lleva punto).
    fn monologo(n: usize, cada: usize) -> Vec<Segmento> {
        (0..n)
            .map(|i| {
                let a = i as i64 * 400;
                let p = if (i + 1) % cada == 0 {
                    "fin."
                } else {
                    "palabra"
                };
                Segmento::entre(a, a + 400, p)
            })
            .collect()
    }

    #[test]
    fn un_monologo_largo_sale_con_una_marca_cada_quince_segundos_como_mucho() {
        // Cinco minutos sin un solo punto ni pausa.
        let palabras: Vec<Segmento> = (0..750)
            .map(|i| Segmento::entre(i * 400, i * 400 + 400, "palabra"))
            .collect();
        let l = lineas(&palabras);
        assert!(l.len() >= 20, "{}", l.len());
        assert!(l.iter().all(|s| s.hasta_ms - s.desde_ms <= PARRAFO_MS));
        for par in l.windows(2) {
            assert!(par[1].desde_ms - par[0].desde_ms <= PARRAFO_MS);
        }
        // No se pierde ni una palabra.
        let total: usize = l.iter().map(|s| s.texto.split(' ').count()).sum();
        assert_eq!(total, 750);
    }

    #[test]
    fn la_clase_de_veintisiete_segundos_ya_no_sale_en_un_solo_bloque() {
        // Dos tandas de Whisper seguidas: 0-14 s y 14-27 s.
        let tandas = [
            Segmento::entre(0, 14_000, "Hoy vemos derivadas."),
            Segmento::entre(14_000, 27_000, "Empezamos por la definicion."),
        ];
        assert_eq!(
            con_tiempos(&tandas, ""),
            "[0:00] Hoy vemos derivadas.\n\n[0:14] Empezamos por la definicion."
        );
    }

    #[test]
    fn un_audio_corto_de_menos_de_quince_segundos_queda_en_una_linea() {
        // 12 s, con un punto cada 2 s.
        let l = lineas(&monologo(30, 5));
        assert_eq!(l.len(), 1, "{l:?}");
        assert_eq!((l[0].desde_ms, l[0].hasta_ms), (0, 12_000));
    }

    #[test]
    fn se_parte_tras_un_punto_antes_que_a_mitad_de_frase() {
        // Frases de 4 s (10 palabras): el corte cae tras el punto de los
        // 12 s, no en la palabra que pasa de los 15.
        let l = lineas(&monologo(100, 10));
        assert!(l.len() > 1);
        for s in &l[..l.len() - 1] {
            assert!(s.texto.ends_with("fin."), "{:?}", s.texto);
            assert!(s.hasta_ms - s.desde_ms >= PARRAFO_MINIMO_MS);
        }
        assert_eq!(l[1].desde_ms, 12_000);
    }

    #[test]
    fn sin_puntos_se_prefiere_un_silencio_y_luego_una_coma() {
        // Palabras de 0,5 s; una coma tras la que acaba a los 5,5 s y un
        // silencio de 1 s tras la que acaba a los 7,5 s.
        let mut v: Vec<Segmento> = (0..32)
            .map(|i| Segmento::entre(i * 500, i * 500 + 500, "a"))
            .collect();
        v[10].texto = "coma,".into();
        for s in &mut v[15..] {
            s.desde_ms += 1_000;
            s.hasta_ms += 1_000;
        }
        let l = lineas(&v);
        assert_eq!(l[1].desde_ms, 8_500, "tras el silencio: {l:?}");
        // Sin el silencio, tras la coma.
        let seguido: Vec<Segmento> = (0..32)
            .map(|i| {
                let t = if i == 15 { "coma," } else { "a" };
                Segmento::entre(i * 500, i * 500 + 500, t)
            })
            .collect();
        let l = lineas(&seguido);
        assert_eq!(l[1].desde_ms, 8_000, "tras la coma: {l:?}");
    }

    #[test]
    fn ninguna_linea_sale_vacia_y_un_trozo_largo_va_solo() {
        let v = [
            Segmento::entre(0, 1_000, "  "),
            Segmento::entre(1_000, 30_000, "una tanda larguisima"),
            Segmento::entre(30_000, 31_000, ""),
            Segmento::entre(31_000, 32_000, "y otra"),
        ];
        assert_eq!(
            lineas(&v),
            [
                Segmento::entre(1_000, 30_000, "una tanda larguisima"),
                Segmento::entre(31_000, 32_000, "y otra"),
            ]
        );
        assert!(lineas(&[]).is_empty());
    }

    #[test]
    fn juntar_lo_ya_juntado_no_cambia_nada() {
        let l = lineas(&monologo(200, 7));
        assert_eq!(lineas(&l), l);
    }

    #[test]
    fn repartir_da_a_cada_palabra_su_trozo_de_la_frase() {
        let p = repartir(10_000, 20_000, "uno dos tres cuatro");
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], Segmento::entre(10_000, 11_875, "uno"));
        assert_eq!(p[3].hasta_ms, 20_000);
        assert!(p.windows(2).all(|w| w[0].hasta_ms == w[1].desde_ms));
        assert!(repartir(0, 100, "   ").is_empty());
        // Una frase de 40 s sin pausa sale en tres lineas.
        let larga = vec!["palabra"; 100].join(" ");
        assert_eq!(lineas(&repartir(0, 40_000, &larga)).len(), 3);
    }

    #[test]
    fn una_frase_que_cabe_en_una_linea_va_entera_y_una_larga_por_palabras() {
        assert_eq!(
            trozos_de_frase(2_000, 9_000, " hola que tal "),
            [Segmento::entre(2_000, 9_000, "hola que tal")]
        );
        assert!(trozos_de_frase(0, 1_000, "  ").is_empty());
        let larga = vec!["palabra"; 60].join(" ");
        let t = trozos_de_frase(0, 30_000, &larga);
        assert_eq!(t.len(), 60);
        assert_eq!(lineas(&t).len(), 2);
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
