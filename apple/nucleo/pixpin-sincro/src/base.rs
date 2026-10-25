//! Lo acordado con otro aparato, con su sello exacto.
//!
//! Puerto de `Base` de `sincro/Disco.kt`. El detalle que obliga a tener esto
//! aparte en vez de un `BTreeMap`: Android calcula el sello como
//! `sha256(Canonico.json.encodeToString(Base))`, es decir, sobre el texto de
//! kotlinx con `encodeDefaults = true` (sale `"proyecto":null` escrito) y con
//! los mapas **en el orden en que los tiene en memoria**. Ese orden es el de
//! llegada: kotlinx decodifica un mapa en un `LinkedHashMap`, y quien dirige
//! lo manda en el orden de su `HashMap`. Los dos lados guardan lo mismo en el
//! mismo orden y por eso sus sellos cuadran; ordenado de otra manera aqui, el
//! sello no cuadraria NUNCA y cada vuelta se haria como la primera.

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::canonico::{Json, Objeto, sha256_hex};

/// Un mapa de texto a texto que recuerda el orden de llegada y no repite
/// claves, como el `LinkedHashMap` de Kotlin: poner una clave que ya esta la
/// cambia en su sitio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mapa {
    pares: Vec<(String, String)>,
}

impl Mapa {
    pub fn nuevo() -> Mapa {
        Mapa::default()
    }

    pub fn obtener(&self, clave: &str) -> Option<&str> {
        self.pares
            .iter()
            .find(|(k, _)| k == clave)
            .map(|(_, v)| v.as_str())
    }

    pub fn poner(&mut self, clave: impl Into<String>, valor: impl Into<String>) {
        let (clave, valor) = (clave.into(), valor.into());
        match self.pares.iter_mut().find(|(k, _)| *k == clave) {
            Some(p) => p.1 = valor,
            None => self.pares.push((clave, valor)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pares.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn claves(&self) -> impl Iterator<Item = &str> {
        self.pares.iter().map(|(k, _)| k.as_str())
    }

    pub fn valores(&self) -> impl Iterator<Item = &str> {
        self.pares.iter().map(|(_, v)| v.as_str())
    }

    pub fn len(&self) -> usize {
        self.pares.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pares.is_empty()
    }

    pub fn contiene(&self, clave: &str) -> bool {
        self.obtener(clave).is_some()
    }

    /// Para quien necesita buscar deprisa y no le importa el orden (el plan).
    pub fn a_btree(&self) -> std::collections::BTreeMap<String, String> {
        self.pares.iter().cloned().collect()
    }

    fn a_objeto(&self) -> Objeto {
        Objeto::de(
            self.pares
                .iter()
                .map(|(k, v)| (k.clone(), Json::cadena(v.clone())))
                .collect(),
        )
    }
}

impl FromIterator<(String, String)> for Mapa {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(i: I) -> Mapa {
        let mut m = Mapa::nuevo();
        for (k, v) in i {
            m.poner(k, v);
        }
        m
    }
}

impl Serialize for Mapa {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.pares.len()))?;
        for (k, v) in &self.pares {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for Mapa {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Mapa, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Mapa;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("un mapa de texto a texto")
            }
            // Se lee par a par en el orden del texto: es el orden que da el
            // sello, y un `HashMap` intermedio lo perderia.
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Mapa, A::Error> {
                let mut m = Mapa::nuevo();
                while let Some((k, v)) = a.next_entry::<String, String>()? {
                    m.poner(k, v);
                }
                Ok(m)
            }
        }
        d.deserialize_map(V)
    }
}

/// Lo acordado con un aparato la ultima vez que se sincronizo un chat.
///
/// En el cable (`Protocolo.json`, `encodeDefaults = false`) lo vacio no se
/// escribe; en el sello, si (ver [Base::sello]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Base {
    #[serde(skip_serializing_if = "Mapa::is_empty")]
    pub mensajes: Mapa,
    #[serde(skip_serializing_if = "Mapa::is_empty")]
    pub archivos: Mapa,
    /// El proyecto tal como quedo, en JSON portatil.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proyecto: Option<String>,
}

impl Base {
    /// El texto de `Canonico.json.encodeToString(Base)`: las tres claves en
    /// el orden de la clase, los mapas en su orden y el proyecto `null` si
    /// no hay. Es lo que se guarda en disco tambien (`Disco.JSON` escribe lo
    /// mismo), asi que un fichero escrito aqui lo lee igual Android.
    pub fn texto_kotlin(&self) -> String {
        let proyecto = match &self.proyecto {
            Some(p) => Json::cadena(p.clone()),
            None => Json::Nulo,
        };
        Json::Objeto(Objeto::de(vec![
            ("mensajes".into(), Json::Objeto(self.mensajes.a_objeto())),
            ("archivos".into(), Json::Objeto(self.archivos.a_objeto())),
            ("proyecto".into(), proyecto),
        ]))
        .a_texto()
    }

    /// `Base.sello` de Android. Si los dos aparatos no dan el mismo, lo
    /// acordado no vale y la vuelta se hace como la primera: junta todo y no
    /// pisa nada.
    pub fn sello(&self) -> String {
        sha256_hex(self.texto_kotlin().as_bytes())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_orden_de_llegada_se_conserva_al_leer_y_al_escribir() {
        let texto = r#"{"mensajes":{"Z":"1","A":"2","M":"3"},"archivos":{"b/x":"9","a/y":"8"}}"#;
        let b: Base = serde_json::from_str(texto).unwrap();
        let claves: Vec<&str> = b.mensajes.claves().collect();
        assert_eq!(claves, ["Z", "A", "M"], "ni ordenado ni al azar");
        assert_eq!(serde_json::to_string(&b).unwrap(), texto);
    }

    #[test]
    fn el_sello_es_el_del_texto_de_kotlin_con_el_proyecto_nulo_escrito() {
        let mut b = Base::default();
        b.mensajes.poner("K2", "abc");
        b.mensajes.poner("A1", "def");
        b.archivos.poner("pins/draw/d.excalidraw.gz", "123");
        // Derivado a mano de `Canonico.json` (encodeDefaults = true): las
        // tres claves en el orden de la clase y el nulo explicito.
        let esperado = r#"{"mensajes":{"K2":"abc","A1":"def"},"archivos":{"pins/draw/d.excalidraw.gz":"123"},"proyecto":null}"#;
        assert_eq!(b.texto_kotlin(), esperado);
        assert_eq!(b.sello(), sha256_hex(esperado.as_bytes()));
        // Vacia: los mapas vacios tambien se escriben.
        assert_eq!(
            Base::default().texto_kotlin(),
            r#"{"mensajes":{},"archivos":{},"proyecto":null}"#
        );
        // SHA-256 de ese texto, calculado aparte: fija el valor, no solo la
        // forma.
        assert_eq!(
            Base::default().sello(),
            sha256_hex(br#"{"mensajes":{},"archivos":{},"proyecto":null}"#)
        );
    }

    #[test]
    fn el_mismo_contenido_en_otro_orden_da_otro_sello() {
        // Es la razon de ser de `Mapa`: con un BTreeMap estos dos darian lo
        // mismo y el de Android no.
        let a: Base = serde_json::from_str(r#"{"mensajes":{"a":"1","b":"2"}}"#).unwrap();
        let b: Base = serde_json::from_str(r#"{"mensajes":{"b":"2","a":"1"}}"#).unwrap();
        assert_ne!(a.sello(), b.sello());
    }

    #[test]
    fn el_proyecto_con_comillas_se_escapa_como_kotlinx() {
        let b = Base {
            proyecto: Some("{\"id\":\"p\"}\n".into()),
            ..Default::default()
        };
        assert_eq!(
            b.texto_kotlin(),
            r#"{"mensajes":{},"archivos":{},"proyecto":"{\"id\":\"p\"}\n"}"#
        );
    }

    #[test]
    fn en_el_cable_lo_vacio_no_se_escribe() {
        assert_eq!(serde_json::to_string(&Base::default()).unwrap(), "{}");
    }
}
