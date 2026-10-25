//! El `.pixpin`: un proyecto entero en un fichero.
//!
//! Es el formato con el que el PixPin de Android pasa un proyecto de un
//! aparato a otro — su propia documentacion dice, con estas palabras, que es
//! «con el que un proyecto se pasa de un aparato a otro *o a la version de
//! escritorio* y se sigue editando alli». Este crate es el lado de aca de esa
//! frase.
//!
//! Dentro hay un ZIP normal, sin cifrar y sin nada binario propio:
//!
//! ```text
//! manifest.json               quien lo escribio y cuando
//! proyecto.json               las hojas y como se relacionan
//! lienzos/<id>.excalidraw     un dibujo por hoja, en JSON plano
//! imagenes/<id>               las fotos, tal cual
//! notas/<id>.md               las notas, en Markdown
//! croquis/<id>.json           los croquis del espacio
//! documento.pdf               el PDF del proyecto, sin anotar
//! ```
//!
//! # La regla que manda aqui
//!
//! **Lo que no se entiende, se conserva.** Windows todavia no sabe dibujar
//! croquis del espacio ni la mitad de las herramientas del movil. Un paquete
//! que entre tiene que poder salir sin haber perdido nada, o abrirlo en el
//! escritorio se convierte en una forma de romper el trabajo de meses.
//!
//! Por eso se guardan las entradas crudas del ZIP y no solo lo interpretado.

pub mod almacen;
pub mod anotado;
pub mod capas_del_pdf;
pub mod carpeta_de_paginas;
pub mod clase;
pub mod codigos;
pub mod comentarios_de_notas;
pub mod cuaderno;
pub mod formula;
pub mod identidad;
pub mod importar_hojas;
pub mod lienzo_de_la_foto;
pub mod mini;
pub mod para_el_movil;
pub mod portapapeles_tabla;
pub mod tabla;
pub mod tabla_web;
pub mod tabla_xlsx;
pub mod ubicacion;
pub mod vista;

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ErrorProyecto {
    #[error("no se pudo acceder a {1}: {0}")]
    Io(#[source] std::io::Error, String),
    #[error("el fichero no es un ZIP valido: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("el JSON de {1} tiene un error: {0}")]
    Json(#[source] serde_json::Error, String),
    #[error("no parece un .pixpin: no tiene proyecto.json")]
    SinProyecto,
}

/// Quien escribio el paquete y cuando.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifiesto {
    pub formato: String,
    pub version: u32,
    pub aplicacion: String,
    /// Milisegundos desde 1970.
    pub escrito: i64,
    pub proyecto: String,
    /// Lo que anada una version de Android al manifiesto, tal cual.
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl Default for Manifiesto {
    fn default() -> Self {
        Self {
            formato: "pixpin".into(),
            version: 1,
            aplicacion: "pixpin-max".into(),
            escrito: 0,
            proyecto: String::new(),
            resto: serde_json::Map::new(),
        }
    }
}

/// Una hoja del proyecto: un dibujo, una nota o un croquis.
///
/// Los tres campos son opcionales y excluyentes en la practica, pero el
/// formato no lo obliga y aqui tampoco: una hoja de una version futura
/// podria traer los tres, y rechazarla seria peor que ensenar lo que se
/// entienda.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Hoja {
    pub id: String,
    pub nombre: String,
    /// Id del lienzo → `lienzos/<dibujo>.excalidraw`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dibujo: Option<String>,
    /// El texto de la nota, tambien copiado en `notas/<id>.md`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nota: Option<String>,
    /// Id del croquis → `croquis/<croquis>.json`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub croquis: Option<String>,
    /// Pagina del `documento.pdf` sobre la que se dibujo, desde 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagina: Option<u32>,
    /// Nombre de una vista guardada del croquis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vista: Option<String>,
    /// Su codigo unico (Android v0.50): no cambia aunque cambie el `id` al
    /// recibirla. Ver `codigos`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    /// Todo lo que trae y aqui no se entiende, tal cual: abrir y volver a
    /// guardar no puede perder lo que anada una version de Android.
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

/// El proyecto: sus hojas y como se relacionan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Proyecto {
    pub id: String,
    pub nombre: String,
    pub hojas: Vec<Hoja>,
    pub archivado: bool,
    /// Milisegundos desde 1970 de la ultima vez que se toco.
    pub tocado: i64,
    pub croquis: Vec<String>,
    /// Rutas del aparato de origen. Al empaquetar van a `null` a proposito:
    /// una ruta de un movil no significa nada en un ordenador.
    #[serde(rename = "pdfOrigen", skip_serializing_if = "Option::is_none")]
    pub pdf_origen: Option<String>,
    #[serde(rename = "pdfLimpio", skip_serializing_if = "Option::is_none")]
    pub pdf_limpio: Option<String>,
    /// Los tres codigos del proyecto (Android v0.50): unico, cuando se creo
    /// (milisegundos desde 1970) y el aparato donde nacio (`K7Q2`). Solo se
    /// pone al dia un proyecto si coinciden los tres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(skip_serializing_if = "es_cero")]
    pub creado: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aparato: Option<String>,
    /// Lo que no se entiende, tal cual (ver `Hoja::resto`).
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

fn es_cero(v: &i64) -> bool {
    *v == 0
}

impl Hoja {
    /// Su codigo unico: el suyo, o el que sale de su id si es de antes.
    pub fn codigo_unico(&self) -> String {
        codigos::unico(self.uid.as_deref(), "h:", &self.id)
    }
}

impl Proyecto {
    pub fn codigo_unico(&self) -> String {
        codigos::unico(self.uid.as_deref(), "p:", &self.id)
    }

    /// Si dos proyectos son la misma cosa: codigo unico, fecha de creacion y
    /// aparato iguales (`Codigos.mismos` de Android). Si no, lo recibido se
    /// crea aparte en vez de pisar.
    pub fn mismo_que(&self, otro: &Proyecto) -> bool {
        self.codigo_unico() == otro.codigo_unico()
            && self.creado == otro.creado
            && self.aparato == otro.aparato
    }

    /// Le pone los codigos que le falten, sin tocar los que ya tiene: el
    /// unico sale de su id (el mismo que calcularia Android) y el aparato es
    /// el de este equipo. La fecha de creacion no se inventa.
    pub fn sellar(&mut self, aparato: Option<&str>) {
        if self.uid.is_none() {
            self.uid = Some(self.codigo_unico());
        }
        if self.aparato.is_none() {
            self.aparato = aparato.map(str::to_string);
        }
        for h in &mut self.hojas {
            if h.uid.is_none() {
                h.uid = Some(h.codigo_unico());
            }
        }
    }
}

/// Un `.pixpin` abierto.
pub struct Paquete {
    pub manifiesto: Manifiesto,
    pub proyecto: Proyecto,
    /// Cada entrada del ZIP con sus bytes, por su nombre dentro del paquete.
    ///
    /// Se guardan TODAS, tambien las que no sabemos interpretar: los croquis
    /// del espacio, las imagenes y lo que traiga una version futura. Es lo
    /// que hace que abrir y volver a guardar no pierda nada.
    entradas: BTreeMap<String, Vec<u8>>,
}

impl Paquete {
    /// Un paquete hecho aqui, sin mas que su proyecto y su manifiesto; lo
    /// demas se anade con `poner_entrada`.
    pub fn nuevo(manifiesto: Manifiesto, proyecto: Proyecto) -> Paquete {
        let mut entradas = BTreeMap::new();
        // Vacias a proposito: `a_bytes` escribe estas dos desde la
        // estructura, pero solo si estan en la lista.
        entradas.insert("manifest.json".to_string(), Vec::new());
        entradas.insert("proyecto.json".to_string(), Vec::new());
        Paquete {
            manifiesto,
            proyecto,
            entradas,
        }
    }

    /// Abre un `.pixpin`.
    pub fn abrir(ruta: &Path) -> Result<Paquete, ErrorProyecto> {
        let bytes =
            std::fs::read(ruta).map_err(|e| ErrorProyecto::Io(e, ruta.display().to_string()))?;
        Paquete::desde_bytes(&bytes)
    }

    /// Lo mismo, desde memoria. Aparte para poder probarlo sin disco.
    pub fn desde_bytes(bytes: &[u8]) -> Result<Paquete, ErrorProyecto> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
        let mut entradas = BTreeMap::new();
        for i in 0..zip.len() {
            let mut fichero = zip.by_index(i)?;
            // Las carpetas no llevan contenido; guardarlas como entrada
            // vacia crearia ficheros de cero bytes al volver a escribir.
            if fichero.is_dir() {
                continue;
            }
            // `enclosed_name` rechaza las rutas que se salen del paquete
            // («../../etc»). Un ZIP puede traerlas a proposito, y escribir
            // eso al disco es la vulnerabilidad clasica de los ZIP.
            let Some(nombre) = fichero.enclosed_name() else {
                continue;
            };
            let nombre = nombre.to_string_lossy().replace('\\', "/");
            let mut datos = Vec::new();
            fichero
                .read_to_end(&mut datos)
                .map_err(|e| ErrorProyecto::Io(e, nombre.clone()))?;
            entradas.insert(nombre, datos);
        }

        let crudo_proyecto = entradas
            .get("proyecto.json")
            .ok_or(ErrorProyecto::SinProyecto)?;
        let proyecto: Proyecto = serde_json::from_slice(crudo_proyecto)
            .map_err(|e| ErrorProyecto::Json(e, "proyecto.json".into()))?;
        // Un manifiesto que falte o este roto no impide abrir: lo que
        // importa del paquete son las hojas, y quedarse sin proyecto por una
        // fecha mal escrita seria desproporcionado.
        let manifiesto = entradas
            .get("manifest.json")
            .and_then(|c| serde_json::from_slice(c).ok())
            .unwrap_or_default();

        Ok(Paquete {
            manifiesto,
            proyecto,
            entradas,
        })
    }

    /// El lienzo de una hoja, ya traducido a elementos nuestros.
    ///
    /// `None` si la hoja no tiene dibujo o si su fichero no esta en el
    /// paquete — que pasa si el movil lo escribio a medias.
    pub fn lienzo_de(
        &self,
        hoja: &Hoja,
    ) -> Option<Result<pixpin_motor2d::excalidraw::Lienzo, ErrorProyecto>> {
        let id = hoja.dibujo.as_ref()?;
        let crudo = self.entradas.get(&format!("lienzos/{id}.excalidraw"))?;
        let texto = String::from_utf8_lossy(crudo);
        Some(
            pixpin_motor2d::excalidraw::leer(&texto)
                .map_err(|e| ErrorProyecto::Io(std::io::Error::other(e), format!("lienzos/{id}"))),
        )
    }

    /// La nota de una hoja. Se prefiere la que viene dentro del proyecto: es
    /// la que el movil da por buena, y el fichero de `notas/` es su copia.
    pub fn nota_de(&self, hoja: &Hoja) -> Option<String> {
        if let Some(t) = &hoja.nota {
            return Some(t.clone());
        }
        self.entradas
            .get(&format!("notas/{}.md", hoja.id))
            .map(|c| String::from_utf8_lossy(c).into_owned())
    }

    /// Los bytes de una entrada cualquiera, por su nombre en el paquete.
    pub fn entrada(&self, nombre: &str) -> Option<&[u8]> {
        self.entradas.get(nombre).map(|v| v.as_slice())
    }

    /// Todos los nombres que hay dentro.
    pub fn nombres(&self) -> impl Iterator<Item = &str> {
        self.entradas.keys().map(|s| s.as_str())
    }

    /// Cambia el contenido de una entrada, o la crea.
    pub fn poner_entrada(&mut self, nombre: &str, datos: Vec<u8>) {
        self.entradas.insert(nombre.to_string(), datos);
    }

    /// Escribe el paquete.
    ///
    /// El manifiesto y el proyecto se vuelven a serializar desde sus
    /// estructuras; todo lo demas sale tal cual entro.
    pub fn guardar(&self, ruta: &Path) -> Result<(), ErrorProyecto> {
        let bytes = self.a_bytes()?;
        std::fs::write(ruta, bytes).map_err(|e| ErrorProyecto::Io(e, ruta.display().to_string()))
    }

    /// Lo mismo, a memoria. Aparte para poder probarlo sin disco.
    pub fn a_bytes(&self) -> Result<Vec<u8>, ErrorProyecto> {
        let mut salida = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut salida));
            let opciones: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            let manifiesto = serde_json::to_vec_pretty(&self.manifiesto)
                .map_err(|e| ErrorProyecto::Json(e, "manifest.json".into()))?;
            let proyecto = serde_json::to_vec_pretty(&self.proyecto)
                .map_err(|e| ErrorProyecto::Json(e, "proyecto.json".into()))?;

            for (nombre, datos) in &self.entradas {
                // Estos dos se escriben desde la estructura, no desde lo que
                // entro: son los unicos que este lado edita.
                let contenido: &[u8] = match nombre.as_str() {
                    "manifest.json" => &manifiesto,
                    "proyecto.json" => &proyecto,
                    _ => datos,
                };
                zip.start_file(nombre, opciones)
                    .map_err(ErrorProyecto::Zip)?;
                zip.write_all(contenido)
                    .map_err(|e| ErrorProyecto::Io(e, nombre.clone()))?;
            }
            // Si el paquete no traia manifiesto, se le pone uno: un `.pixpin`
            // sin el es legible pero no dice de donde viene.
            if !self.entradas.contains_key("manifest.json") {
                zip.start_file("manifest.json", opciones)
                    .map_err(ErrorProyecto::Zip)?;
                zip.write_all(&manifiesto)
                    .map_err(|e| ErrorProyecto::Io(e, "manifest.json".into()))?;
            }
            zip.finish().map_err(ErrorProyecto::Zip)?;
        }
        Ok(salida)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_proyecto_de_android_con_sus_tres_codigos_sale_igual_al_volver_a_escribirlo() {
        let original = r#"{"id":"p1","nombre":"Casa","hojas":[{"id":"h1","nombre":"Planta","dibujo":"d1","uid":"2C8C39HXZ9","padre":"h0","borrada":123}],"archivado":false,"tocado":5,"croquis":[],"uid":"VVT587BFCA","creado":1757939357123,"aparato":"K7Q2","borradas":{"h9":456},"vieneDe":{"aparato":"9FMQ"}}"#;
        let p: Proyecto = serde_json::from_str(original).unwrap();
        assert_eq!(p.uid.as_deref(), Some("VVT587BFCA"));
        assert_eq!(
            (p.creado, p.aparato.as_deref()),
            (1757939357123, Some("K7Q2"))
        );
        let vuelta = serde_json::to_value(&p).unwrap();
        let esperado: serde_json::Value = serde_json::from_str(original).unwrap();
        assert_eq!(vuelta, esperado, "se perdio algo al guardar");
    }

    #[test]
    fn sellar_pone_los_codigos_que_saldrian_en_android_y_no_toca_los_que_hay() {
        let mut p = Proyecto {
            id: "proyecto-1".into(),
            hojas: vec![Hoja {
                id: "hoja-7".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        p.sellar(Some("ABCD"));
        assert_eq!(p.uid.as_deref(), Some("VVT587BFCA"));
        assert_eq!(p.hojas[0].uid.as_deref(), Some("2C8C39HXZ9"));
        assert_eq!(p.aparato.as_deref(), Some("ABCD"));
        // Caso negativo: lo que ya tenia codigos no cambia al sellar otra vez.
        let mut q = p.clone();
        q.sellar(Some("ZZZZ"));
        assert_eq!(q.aparato.as_deref(), Some("ABCD"));
        assert!(p.mismo_que(&q));
        // Mismo unico pero otra fecha: no es la misma cosa, se crea aparte.
        q.creado = 1;
        assert!(!p.mismo_que(&q));
    }

    /// Un paquete de mentira con lo justo: proyecto, un lienzo, una nota y
    /// un croquis que aqui no sabemos leer.
    fn paquete_de_prueba() -> Vec<u8> {
        let mut salida = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut salida));
            let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            let mut poner = |nombre: &str, texto: &str| {
                zip.start_file(nombre, o).unwrap();
                zip.write_all(texto.as_bytes()).unwrap();
            };
            poner(
                "manifest.json",
                r#"{"formato":"pixpin","version":1,"aplicacion":"pixpin-android","escrito":1725500000000,"proyecto":"Casa Rosales"}"#,
            );
            poner(
                "proyecto.json",
                r##"{"id":"pr-1","nombre":"Casa Rosales","hojas":[
                    {"id":"h1","nombre":"Planta","dibujo":"dib-1","pagina":0},
                    {"id":"h2","nombre":"Notas","nota":"# Obra\nlo que sea"},
                    {"id":"h3","nombre":"Maqueta","croquis":"cr-1"}
                 ],"archivado":false,"tocado":1725500000000,"croquis":["cr-1"]}"##,
            );
            poner(
                "lienzos/dib-1.excalidraw",
                r#"{"type":"excalidraw","elements":[
                    {"id":"a","type":"rectangle","x":1,"y":2,"width":3,"height":4}
                 ]}"#,
            );
            poner("notas/h2.md", "# Obra\nlo que sea");
            poner("croquis/cr-1.json", r#"{"trazos":[],"vistas":["frente"]}"#);
            zip.finish().unwrap();
        }
        salida
    }

    #[test]
    fn se_lee_el_proyecto_y_sus_hojas() {
        let p = Paquete::desde_bytes(&paquete_de_prueba()).unwrap();
        assert_eq!(p.proyecto.nombre, "Casa Rosales");
        assert_eq!(p.proyecto.hojas.len(), 3);
        assert_eq!(p.proyecto.hojas[0].dibujo.as_deref(), Some("dib-1"));
        assert_eq!(p.proyecto.hojas[0].pagina, Some(0));
        assert_eq!(p.manifiesto.aplicacion, "pixpin-android");
    }

    #[test]
    fn el_lienzo_de_una_hoja_llega_traducido() {
        let p = Paquete::desde_bytes(&paquete_de_prueba()).unwrap();
        let lienzo = p.lienzo_de(&p.proyecto.hojas[0]).unwrap().unwrap();
        let e = lienzo.elementos();
        assert_eq!(e.len(), 1);
        assert_eq!((e[0].x, e[0].y), (1.0, 2.0));
    }

    #[test]
    fn una_hoja_sin_dibujo_no_da_lienzo() {
        // Caso negativo: la hoja de notas no tiene lienzo, y pedirselo no
        // puede inventar uno vacio que luego se guarde encima del bueno.
        let p = Paquete::desde_bytes(&paquete_de_prueba()).unwrap();
        assert!(p.lienzo_de(&p.proyecto.hojas[1]).is_none());
    }

    #[test]
    fn la_nota_se_lee_del_proyecto_o_del_fichero() {
        let p = Paquete::desde_bytes(&paquete_de_prueba()).unwrap();
        assert!(p.nota_de(&p.proyecto.hojas[1]).unwrap().contains("Obra"));
        // Y una hoja que no es de notas no da nota.
        assert!(p.nota_de(&p.proyecto.hojas[0]).is_none());
    }

    #[test]
    fn lo_que_no_entendemos_sobrevive_a_la_ida_y_vuelta() {
        // Es la razon de ser de este crate. Windows no sabe leer un croquis
        // del espacio; si abrir y guardar lo borrara, abrir un proyecto en
        // el escritorio seria una forma de romper el trabajo de meses.
        let p = Paquete::desde_bytes(&paquete_de_prueba()).unwrap();
        assert!(p.entrada("croquis/cr-1.json").is_some());
        let vuelta = Paquete::desde_bytes(&p.a_bytes().unwrap()).unwrap();
        assert_eq!(
            vuelta.entrada("croquis/cr-1.json"),
            p.entrada("croquis/cr-1.json"),
            "el croquis se perdio o cambio"
        );
        // Y las hojas que lo referencian siguen ahi.
        assert_eq!(vuelta.proyecto.hojas.len(), 3);
        assert_eq!(vuelta.proyecto.croquis, vec!["cr-1".to_string()]);
    }

    #[test]
    fn el_lienzo_tambien_sobrevive_intacto() {
        let p = Paquete::desde_bytes(&paquete_de_prueba()).unwrap();
        let vuelta = Paquete::desde_bytes(&p.a_bytes().unwrap()).unwrap();
        assert_eq!(
            vuelta.entrada("lienzos/dib-1.excalidraw"),
            p.entrada("lienzos/dib-1.excalidraw")
        );
    }

    #[test]
    fn un_zip_sin_proyecto_no_es_un_pixpin() {
        // Caso negativo: sin esto, arrastrar un ZIP cualquiera daria un
        // proyecto vacio y pareceria que se abrio bien.
        let mut salida = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut salida));
            let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            zip.start_file("hola.txt", o).unwrap();
            zip.write_all(b"nada").unwrap();
            zip.finish().unwrap();
        }
        assert!(matches!(
            Paquete::desde_bytes(&salida),
            Err(ErrorProyecto::SinProyecto)
        ));
    }

    #[test]
    fn lo_que_no_es_un_zip_se_rechaza() {
        assert!(matches!(
            Paquete::desde_bytes(b"esto no es un zip"),
            Err(ErrorProyecto::Zip(_))
        ));
    }

    #[test]
    fn un_manifiesto_roto_no_impide_abrir() {
        // Lo que importa del paquete son las hojas. Quedarse sin proyecto
        // por una fecha mal escrita en el manifiesto seria desproporcionado.
        let mut salida = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut salida));
            let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            zip.start_file("manifest.json", o).unwrap();
            zip.write_all(b"{ roto").unwrap();
            zip.start_file("proyecto.json", o).unwrap();
            zip.write_all(br#"{"nombre":"Sigue valiendo"}"#).unwrap();
            zip.finish().unwrap();
        }
        let p = Paquete::desde_bytes(&salida).unwrap();
        assert_eq!(p.proyecto.nombre, "Sigue valiendo");
        assert_eq!(p.manifiesto.formato, "pixpin", "se uso el de por defecto");
    }

    #[test]
    fn un_proyecto_de_una_version_futura_abre_igual() {
        // La misma regla que los ajustes y el indice: las claves que no se
        // conocen se ignoran, y las que faltan toman su valor por defecto.
        let mut salida = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut salida));
            let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            zip.start_file("proyecto.json", o).unwrap();
            zip.write_all(
                br#"{"nombre":"Del futuro","hojas":[{"id":"h9","inventado":true}],
                     "algoQueNoConocemos":{"a":1}}"#,
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let p = Paquete::desde_bytes(&salida).unwrap();
        assert_eq!(p.proyecto.nombre, "Del futuro");
        assert_eq!(p.proyecto.hojas.len(), 1);
        assert!(!p.proyecto.archivado, "lo que falta toma su valor");
    }
}
