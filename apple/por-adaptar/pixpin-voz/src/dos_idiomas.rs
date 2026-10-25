//! **Dos idiomas en una misma nota**, como `MotorWhisper.reconocer` del movil
//! (`guardados/MotorWhisper.kt:177-235`).
//!
//! Whisper es multilingue: con un solo idioma puesto se le fuerza (asi no
//! adivina lenguas que nadie hablo); con un segundo idioma hay dos modos,
//! los mismos del movil y con los mismos nombres guardados en Ajustes
//! (`modoDeIdiomas`):
//!
//! - **«cada trozo como se dijo»** (`cada_uno`): en cada tanda Whisper dice
//!   que idioma oye; si es uno de los dos y el texto se sostiene
//!   ([`creible`](crate::credibilidad::creible)), vale; si no, se rehace
//!   forzando el primero. Solo se paga dos veces cuando se equivoca.
//! - **«todo en el primero»** (`todo_en_uno`): todas las tandas con el
//!   primero forzado, que es lo que hace que lo dicho en el otro salga
//!   traducido. La tarea `translate` de Whisper lleva **siempre** al ingles,
//!   asi que solo se usa si el primero es el ingles (`7264d3d`: con el
//!   primero en castellano escribia en ingles lo dicho en castellano).
//!
//! Aqui solo estan las decisiones, sin Windows ni modelo: son las que se
//! prueban. Quien las aplica es `whisper::Whisper::transcribir_pcm_con`.

/// Qué hace Whisper con dos idiomas mezclados. Las palabras son las del
/// movil (`MODO_CADA_IDIOMA = "cada_uno"`, `MODO_TODO_EN_UNO`), para que el
/// ajuste signifique lo mismo en los dos aparatos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoDeIdiomas {
    /// Cada trozo en el idioma en que se dijo.
    #[default]
    CadaUno,
    /// Todo en el primer idioma (lo del otro, traducido).
    TodoEnUno,
}

impl ModoDeIdiomas {
    pub fn palabra(self) -> &'static str {
        match self {
            ModoDeIdiomas::CadaUno => "cada_uno",
            ModoDeIdiomas::TodoEnUno => "todo_en_uno",
        }
    }

    /// Una palabra que no se conoce es el modo de fabrica, como en el movil
    /// (`prefs[...] ?: MODO_CADA_IDIOMA`).
    pub fn de(palabra: &str) -> ModoDeIdiomas {
        match palabra.trim() {
            "todo_en_uno" => ModoDeIdiomas::TodoEnUno,
            _ => ModoDeIdiomas::CadaUno,
        }
    }
}

/// Lo que se le pide a Whisper: el primero, el segundo (o ninguno) y el modo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idiomas {
    /// El idioma principal, ya como codigo de Whisper (`es`).
    pub principal: String,
    /// El segundo, solo si hay uno y es distinto del primero.
    pub otro: Option<String>,
    pub modo: ModoDeIdiomas,
}

/// `transcribe` o `translate`: las dos tareas que trae Whisper de fabrica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tarea {
    Transcribir,
    Traducir,
}

/// Menos que esto no se le deja adivinar el idioma
/// (`MINIMO_PARA_ADIVINAR_SEGUNDOS = 3`): un trozo corto adivina mal.
pub const MINIMO_PARA_ADIVINAR_MS: i64 = 3_000;

/// El codigo corto que entiende Whisper (`es-PE` → `es`), o `None` si
/// Whisper no lo sabe. El movil cae a «es» con uno raro; aqui quien llama
/// decide, porque un segundo idioma raro es «ninguno», no «castellano».
pub fn codigo(idioma: &str, sabe: impl Fn(&str) -> bool) -> Option<String> {
    let c = idioma.split('-').next().unwrap_or("").trim().to_lowercase();
    (!c.is_empty() && sabe(&c)).then_some(c)
}

impl Idiomas {
    /// Solo el primero: lo de siempre.
    pub fn uno(principal: &str) -> Idiomas {
        Idiomas {
            principal: principal.to_string(),
            otro: None,
            modo: ModoDeIdiomas::CadaUno,
        }
    }

    /// Con segundo: se descarta si esta vacio o es el mismo que el primero
    /// (`takeIf { it != principal }`).
    pub fn con(principal: &str, segundo: &str, modo: ModoDeIdiomas) -> Idiomas {
        let otro = Some(segundo.trim().to_lowercase()).filter(|s| !s.is_empty() && s != principal);
        Idiomas {
            principal: principal.to_string(),
            otro,
            modo,
        }
    }

    /// Si en cada tanda se deja adivinar el idioma: solo con dos idiomas y
    /// en «cada trozo como se dijo».
    pub fn adivinar(&self) -> bool {
        self.otro.is_some() && self.modo == ModoDeIdiomas::CadaUno
    }

    /// La tarea: traducir solo con «todo en uno», dos idiomas y el primero
    /// en ingles, que es el unico destino de `translate`.
    pub fn tarea(&self) -> Tarea {
        if self.modo == ModoDeIdiomas::TodoEnUno && self.otro.is_some() && self.principal == "en" {
            Tarea::Traducir
        } else {
            Tarea::Transcribir
        }
    }

    /// Si lo que Whisper dice haber oido vale: tiene que ser uno de los dos
    /// que se pusieron y el texto tiene que sostenerse en ese idioma.
    pub fn acepta(&self, oido: &str, texto: &str) -> bool {
        let es_de_los_dos = oido == self.principal || self.otro.as_deref() == Some(oido);
        es_de_los_dos && crate::credibilidad::creible(texto.trim(), oido)
    }

    /// El ultimo intento del movil (l. 226-234): un texto que no se sostiene
    /// y que no estaba en el primero se rehace una vez con el primero antes
    /// de tirarlo. Perder un trozo se ve como «no transcribe bien».
    pub fn reintentar_con_el_primero(&self, lengua: &str, texto: &str) -> bool {
        !texto.trim().is_empty()
            && lengua != self.principal
            && !crate::credibilidad::creible(texto.trim(), lengua)
    }
}

/// Whisper dice el idioma como `<|es|>`: se queda solo el codigo
/// (`r.lang.trim().lowercase().trim('<', '>', '|')`).
pub fn limpiar_codigo(oido: &str) -> String {
    oido.trim()
        .to_lowercase()
        .trim_matches(|c| c == '<' || c == '>' || c == '|')
        .to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_modo_se_guarda_con_las_palabras_del_movil_y_una_rara_es_el_de_fabrica() {
        assert_eq!(ModoDeIdiomas::CadaUno.palabra(), "cada_uno");
        assert_eq!(ModoDeIdiomas::TodoEnUno.palabra(), "todo_en_uno");
        assert_eq!(ModoDeIdiomas::de("todo_en_uno"), ModoDeIdiomas::TodoEnUno);
        assert_eq!(ModoDeIdiomas::de("cada_uno"), ModoDeIdiomas::CadaUno);
        assert_eq!(ModoDeIdiomas::de("otra cosa"), ModoDeIdiomas::CadaUno);
        assert_eq!(ModoDeIdiomas::de(""), ModoDeIdiomas::CadaUno);
    }

    #[test]
    fn un_segundo_idioma_vacio_o_igual_al_primero_es_ninguno() {
        assert_eq!(Idiomas::con("es", "", ModoDeIdiomas::CadaUno).otro, None);
        assert_eq!(Idiomas::con("es", "  ", ModoDeIdiomas::CadaUno).otro, None);
        assert_eq!(Idiomas::con("es", "ES", ModoDeIdiomas::CadaUno).otro, None);
        assert_eq!(
            Idiomas::con("es", "en", ModoDeIdiomas::CadaUno)
                .otro
                .as_deref(),
            Some("en")
        );
    }

    #[test]
    fn solo_se_adivina_con_dos_idiomas_y_cada_trozo_en_el_suyo() {
        assert!(Idiomas::con("es", "en", ModoDeIdiomas::CadaUno).adivinar());
        assert!(!Idiomas::con("es", "en", ModoDeIdiomas::TodoEnUno).adivinar());
        assert!(!Idiomas::uno("es").adivinar());
    }

    #[test]
    fn traducir_solo_cuando_el_destino_es_el_ingles() {
        assert_eq!(
            Idiomas::con("en", "es", ModoDeIdiomas::TodoEnUno).tarea(),
            Tarea::Traducir
        );
        // Con el primero en castellano, `translate` escribiria en ingles lo
        // dicho en castellano: se transcribe forzando el primero.
        assert_eq!(
            Idiomas::con("es", "en", ModoDeIdiomas::TodoEnUno).tarea(),
            Tarea::Transcribir
        );
        assert_eq!(
            Idiomas::con("en", "es", ModoDeIdiomas::CadaUno).tarea(),
            Tarea::Transcribir
        );
        assert_eq!(Idiomas::uno("en").tarea(), Tarea::Transcribir);
    }

    #[test]
    fn lo_adivinado_vale_si_es_de_los_dos_y_se_sostiene() {
        let d = Idiomas::con("es", "en", ModoDeIdiomas::CadaUno);
        assert!(d.acepta("en", "We will meet tomorrow at the office"));
        assert!(d.acepta("es", "Mañana nos vemos en la oficina"));
    }

    #[test]
    fn un_idioma_que_nadie_puso_no_vale_aunque_el_texto_sea_bueno() {
        let d = Idiomas::con("es", "en", ModoDeIdiomas::CadaUno);
        assert!(!d.acepta("pt", "Amanhã nos vemos no escritório"));
        assert!(!d.acepta("", "hola"));
    }

    #[test]
    fn lo_de_los_dos_que_no_se_sostiene_no_vale() {
        let d = Idiomas::con("es", "en", ModoDeIdiomas::CadaUno);
        // Basura con que Whisper rellena el silencio.
        assert!(!d.acepta("es", "Subtítulos realizados por la comunidad de Amara.org"));
        assert!(!d.acepta("en", ""));
    }

    #[test]
    fn se_reintenta_con_el_primero_solo_lo_que_no_estaba_en_el_primero_y_no_se_sostiene() {
        let d = Idiomas::con("es", "en", ModoDeIdiomas::CadaUno);
        assert!(d.reintentar_con_el_primero("en", "Thank you for watching"));
        assert!(!d.reintentar_con_el_primero("es", "Thank you for watching"));
        assert!(!d.reintentar_con_el_primero("en", "We will meet tomorrow"));
        assert!(!d.reintentar_con_el_primero("en", "  "));
    }

    #[test]
    fn el_idioma_de_whisper_se_queda_sin_sus_signos() {
        assert_eq!(limpiar_codigo("<|es|>"), "es");
        assert_eq!(limpiar_codigo(" <|EN|> "), "en");
        assert_eq!(limpiar_codigo("pt"), "pt");
        assert_eq!(limpiar_codigo(""), "");
    }

    #[test]
    fn el_codigo_corto_solo_si_whisper_lo_sabe() {
        let sabe = |c: &str| ["es", "en", "haw"].contains(&c);
        assert_eq!(codigo("es-PE", sabe).as_deref(), Some("es"));
        assert_eq!(codigo("EN", sabe).as_deref(), Some("en"));
        assert_eq!(codigo("xx", sabe), None);
        assert_eq!(codigo("", sabe), None);
    }
}
