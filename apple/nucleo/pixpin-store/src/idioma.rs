//! Idiomas de la interfaz, con Fluent.
//!
//! Se elige Fluent y no un mapa de cadenas porque maneja bien plurales y
//! genero, que en español hacen falta en cuanto aparece un contador.
//!
//! `resolver_idioma` recibe el locale como texto en vez de consultarlo al
//! sistema: este crate tiene `#![forbid(unsafe_code)]` y no puede llamar a
//! Win32. El ejecutable se lo pasa desde `pixpin_sistema::entorno`.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

use crate::ajustes::PreferenciaIdioma;

const FTL_ES: &str = include_str!("../i18n/es-ES/main.ftl");
const FTL_EN: &str = include_str!("../i18n/en-US/main.ftl");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idioma {
    Espanol,
    Ingles,
}

impl Idioma {
    fn etiqueta(self) -> &'static str {
        match self {
            Idioma::Espanol => "es-ES",
            Idioma::Ingles => "en-US",
        }
    }

    fn fuente(self) -> &'static str {
        match self {
            Idioma::Espanol => FTL_ES,
            Idioma::Ingles => FTL_EN,
        }
    }
}

/// Decide el idioma final. La preferencia explicita del usuario siempre gana.
pub fn resolver_idioma(locale_sistema: &str, preferencia: PreferenciaIdioma) -> Idioma {
    match preferencia {
        PreferenciaIdioma::Espanol => Idioma::Espanol,
        PreferenciaIdioma::Ingles => Idioma::Ingles,
        // Solo miramos la parte de idioma: es-MX y es-ES comparten catalogo.
        PreferenciaIdioma::Sistema => {
            if locale_sistema.split(['-', '_']).next().unwrap_or("") == "es" {
                Idioma::Espanol
            } else {
                Idioma::Ingles
            }
        }
    }
}

/// Catalogo de textos ya cargado.
pub struct Catalogo {
    bundle: FluentBundle<FluentResource>,
}

impl Catalogo {
    pub fn nuevo(idioma: Idioma) -> Self {
        let lang: LanguageIdentifier = idioma
            .etiqueta()
            .parse()
            .expect("las etiquetas de idioma son constantes del propio codigo");

        let recurso = FluentResource::try_new(idioma.fuente().to_string())
            .expect("los catalogos se validan por test en tiempo de compilacion");

        let mut bundle = FluentBundle::new_concurrent(vec![lang]);
        // Fluent inserta marcas de direccion invisibles alrededor de los
        // argumentos. Utiles en arabe o hebreo; aqui solo ensucian el texto y
        // rompen las comparaciones exactas de los tests.
        bundle.set_use_isolating(false);
        bundle
            .add_resource(recurso)
            .expect("no puede haber claves duplicadas en un unico recurso");

        Self { bundle }
    }

    /// Texto sin argumentos. Si la clave falta, devuelve la propia clave.
    pub fn t(&self, clave: &str) -> String {
        self.con_argumentos(clave, None)
    }

    /// Texto con argumentos, por ejemplo `{ $atajo }`.
    pub fn t_args(&self, clave: &str, args: &FluentArgs) -> String {
        self.con_argumentos(clave, Some(args))
    }

    fn con_argumentos(&self, clave: &str, args: Option<&FluentArgs>) -> String {
        let Some(mensaje) = self.bundle.get_message(clave) else {
            return clave.to_string();
        };
        let Some(patron) = mensaje.value() else {
            return clave.to_string();
        };
        let mut errores = Vec::new();
        let texto = self.bundle.format_pattern(patron, args, &mut errores);
        if !errores.is_empty() {
            tracing::warn!(clave, ?errores, "error al formatear un texto");
        }
        texto.into_owned()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_textos_de_captura_estan_en_los_dos_idiomas() {
        // El test de paridad ya comprueba que las claves coinciden, pero no
        // que existan. Este falla si alguien las olvida en ambos catalogos.
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);
        for clave in ["captura-guardada", "captura-fallo"] {
            assert_ne!(es.t(clave), clave, "falta {clave} en es-ES");
            assert_ne!(en.t(clave), clave, "falta {clave} en en-US");
        }
    }

    #[test]
    fn los_textos_de_la_pila_estan_en_los_dos_idiomas() {
        // La paridad comprueba que las claves coinciden; esto, que existen.
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);
        for clave in [
            "pila-titulo-una",
            "pila-titulo-varias",
            "pila-copiar-elegidas",
            "pila-copiar-todas",
            "pila-quitar",
            "pila-copiadas",
        ] {
            assert_ne!(es.t(clave), clave, "falta {clave} en es-ES");
            assert_ne!(en.t(clave), clave, "falta {clave} en en-US");
        }
    }

    #[test]
    fn el_titulo_de_la_pila_interpola_cuantas_son() {
        let es = Catalogo::nuevo(Idioma::Espanol);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("cuantas", 5);
        let texto = es.t_args("pila-titulo-varias", &args);
        assert!(texto.contains('5'), "deberia decir cuantas son: {texto:?}");
        assert!(
            !texto.contains('\u{2068}'),
            "sin marcas de aislamiento: {texto:?}"
        );
    }

    #[test]
    fn los_textos_de_la_barra_estan_en_los_dos_idiomas() {
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);
        for clave in [
            "barra-copiar",
            "barra-guardar",
            "barra-guardar-como",
            "barra-descartar",
        ] {
            assert_ne!(es.t(clave), clave, "falta {clave} en es-ES");
            assert_ne!(en.t(clave), clave, "falta {clave} en en-US");
        }
    }

    #[test]
    fn la_preferencia_explicita_gana_al_sistema() {
        assert_eq!(
            resolver_idioma("es-ES", PreferenciaIdioma::Ingles),
            Idioma::Ingles
        );
        assert_eq!(
            resolver_idioma("en-US", PreferenciaIdioma::Espanol),
            Idioma::Espanol
        );
    }

    #[test]
    fn en_modo_sistema_se_mira_el_locale() {
        assert_eq!(
            resolver_idioma("es-ES", PreferenciaIdioma::Sistema),
            Idioma::Espanol
        );
        assert_eq!(
            resolver_idioma("es-MX", PreferenciaIdioma::Sistema),
            Idioma::Espanol
        );
        assert_eq!(
            resolver_idioma("en-GB", PreferenciaIdioma::Sistema),
            Idioma::Ingles
        );
    }

    #[test]
    fn un_idioma_no_soportado_cae_a_ingles() {
        // Ingles y no español: es el reparto habitual para un idioma que no
        // conocemos, y quien tenga el sistema en aleman entendera antes ingles.
        assert_eq!(
            resolver_idioma("de-DE", PreferenciaIdioma::Sistema),
            Idioma::Ingles
        );
        assert_eq!(
            resolver_idioma("", PreferenciaIdioma::Sistema),
            Idioma::Ingles
        );
        assert_eq!(
            resolver_idioma("basura", PreferenciaIdioma::Sistema),
            Idioma::Ingles
        );
    }

    #[test]
    fn los_dos_catalogos_devuelven_texto_traducido() {
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);

        assert_eq!(es.t("comando-salir"), "Salir");
        assert_eq!(en.t("comando-salir"), "Quit");
        // El nombre del producto no se traduce.
        assert_eq!(es.t("app-nombre"), "PixPin Max");
        assert_eq!(en.t("app-nombre"), "PixPin Max");
    }

    #[test]
    fn una_clave_que_falta_devuelve_la_propia_clave() {
        // Devolver la clave en vez de entrar en panico: un texto que falta es
        // feo, pero una aplicacion que se cierra por eso es inaceptable.
        let es = Catalogo::nuevo(Idioma::Espanol);
        assert_eq!(es.t("clave-que-no-existe"), "clave-que-no-existe");
    }

    #[test]
    fn t_args_sustituye_el_argumento_y_no_deja_marcas_de_aislamiento() {
        // set_use_isolating(false) existe para esto: sin ella, Fluent envuelve
        // cada argumento interpolado entre U+2068 y U+2069 (marcas de
        // aislamiento direccional, utiles en arabe o hebreo). Son invisibles
        // en pantalla, asi que dos cadenas que "se ven iguales" pueden diferir
        // en bytes. Este test compara la cadena exacta para que quien quite
        // esa linea lo vea aqui, no en un bug reportado por un usuario.
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);

        let mut args = FluentArgs::new();
        args.set("atajo", "Ctrl+Alt+X");

        let texto_es = es.t_args("error-atajo-ocupado", &args);
        let texto_en = en.t_args("error-atajo-ocupado", &args);

        assert_eq!(
            texto_es,
            "No se pudo registrar Ctrl+Alt+X: otra aplicación lo está usando."
        );
        assert_eq!(
            texto_en,
            "Could not register Ctrl+Alt+X: another application is using it."
        );

        for (idioma, texto) in [("es-ES", &texto_es), ("en-US", &texto_en)] {
            assert!(
                !texto.contains('\u{2068}') && !texto.contains('\u{2069}'),
                "el texto de {idioma} lleva marcas de aislamiento direccional \
                 (U+2068/U+2069) alrededor del argumento interpolado; falta \
                 `bundle.set_use_isolating(false)` en Catalogo::nuevo. Texto: {texto:?}"
            );
        }
    }

    #[test]
    fn los_dos_catalogos_tienen_exactamente_las_mismas_claves() {
        // Este test es la red que impide que una traduccion se quede atras.
        let claves_es = claves_de(include_str!("../i18n/es-ES/main.ftl"));
        let claves_en = claves_de(include_str!("../i18n/en-US/main.ftl"));

        let solo_es: Vec<_> = claves_es.difference(&claves_en).collect();
        let solo_en: Vec<_> = claves_en.difference(&claves_es).collect();

        assert!(
            solo_es.is_empty() && solo_en.is_empty(),
            "catalogos desincronizados. Solo en es-ES: {solo_es:?}. Solo en en-US: {solo_en:?}"
        );
    }

    /// Extrae los identificadores de un fichero .ftl sin usar el parser de
    /// Fluent: basta con las lineas que empiezan por identificador y `=`.
    ///
    /// Limitacion conocida: no reconoce valores multilinea ni atributos
    /// (`.attr = ...`). Ningun catalogo actual los usa; si se añaden, este
    /// helper habra que revisarlo.
    fn claves_de(ftl: &str) -> std::collections::BTreeSet<String> {
        ftl.lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.starts_with(char::is_whitespace))
            .filter_map(|l| l.split_once('='))
            .map(|(clave, _)| clave.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect()
    }
}
