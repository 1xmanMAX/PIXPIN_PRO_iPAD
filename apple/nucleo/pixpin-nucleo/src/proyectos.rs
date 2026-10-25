//! **Lo que se hace con un proyecto desde su fila** (Windows,
//! `ventana_chat/proyectos.rs`): cambiarle el nombre, archivarlo y abrir un
//! `.pixpin` que llega de fuera (Archivos, otra app, AirDrop).
//!
//! Nada nuevo en el formato: el nombre va al indice (`nombre`, `tocado`) y
//! al `proyecto.json` si lo tiene, que es lo que viaja; el archivado va al
//! `proyecto.json` si lo tiene o, si no, a la marca
//! [`pixpin_proyecto::vista::MARCA_ARCHIVADO`] de su ficha. Subir `tocado`
//! es lo que hace que la siguiente vuelta de sincronizar lo lleve, como en
//! Windows.

use std::path::Path;

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::{Paquete, Proyecto, vista};
use pixpin_sincro::envio::{self, Elemento};

use crate::chats::{FichaChat, validar_chat};
use crate::{ErrorNucleo, Nucleo};

/// Lo que queda de un nombre escrito: su primera linea recortada, o nada si
/// esta en blanco (`nombre_limpio` de Windows).
pub fn nombre_limpio(escrito: &str) -> Option<String> {
    let limpio = escrito.lines().next().unwrap_or("").trim();
    (!limpio.is_empty()).then(|| limpio.to_string())
}

fn leer_proyecto_json(carpeta: &Path) -> Proyecto {
    std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Lo escribe entero, primero al lado y luego de un tiron (el de Windows).
fn guardar_proyecto_json(carpeta: &Path, p: &Proyecto) -> std::io::Result<()> {
    let texto = serde_json::to_string(p).map_err(std::io::Error::other)?;
    std::fs::create_dir_all(carpeta)?;
    let temporal = carpeta.join("proyecto.json.tmp");
    std::fs::write(&temporal, texto)?;
    std::fs::rename(&temporal, carpeta.join("proyecto.json"))
}

fn no_existe(id: &str) -> ErrorNucleo {
    ErrorNucleo::NoExiste {
        que: format!("proyecto {id}"),
    }
}

/// **Cambia el nombre** (`renombrar` de Windows): recortado, en blanco no
/// vale, y cuenta como tocarlo (sube arriba). «Mensajes guardados» no se
/// renombra.
pub fn renombrar(
    raiz: &Path,
    id: &str,
    nombre: &str,
    ahora: i64,
) -> Result<FichaChat, ErrorNucleo> {
    validar_chat(id)?;
    let limpio = nombre_limpio(nombre).ok_or_else(|| ErrorNucleo::Argumento {
        detalle: "el nombre del proyecto no puede quedar vacio".into(),
    })?;
    let mut indice = Indice::leer_si_esta(raiz)?;
    let f = indice
        .proyectos
        .iter_mut()
        .find(|f| f.id == id)
        .ok_or_else(|| no_existe(id))?;
    if f.es_guardados() {
        return Err(ErrorNucleo::Argumento {
            detalle: "«Mensajes guardados» no se renombra".into(),
        });
    }
    f.nombre = limpio.clone();
    f.tocado = ahora;
    let ficha = FichaChat::from(&*f);
    indice.guardar(raiz)?;
    let carpeta = almacen::carpeta(raiz, id);
    if carpeta.join("proyecto.json").is_file() {
        let mut p = leer_proyecto_json(&carpeta);
        p.nombre = limpio;
        p.tocado = ahora;
        guardar_proyecto_json(&carpeta, &p)?;
    }
    Ok(ficha)
}

/// **Archiva o desarchiva** (`archivar` de Windows, que tambien lo da por
/// tocado): en su `proyecto.json` si lo tiene (viaja al movil, que lo lee
/// igual), y si no con la marca en su ficha del indice.
pub fn archivar(
    raiz: &Path,
    id: &str,
    archivar: bool,
    ahora: i64,
) -> Result<FichaChat, ErrorNucleo> {
    validar_chat(id)?;
    let mut indice = Indice::leer_si_esta(raiz)?;
    let f = indice
        .proyectos
        .iter_mut()
        .find(|f| f.id == id)
        .ok_or_else(|| no_existe(id))?;
    if f.es_guardados() || f.es_asistente() {
        return Err(ErrorNucleo::Argumento {
            detalle: "ese chat no se archiva".into(),
        });
    }
    let carpeta = almacen::carpeta(raiz, id);
    let con_json = carpeta.join("proyecto.json").is_file();
    if con_json {
        let mut p = leer_proyecto_json(&carpeta);
        p.archivado = archivar;
        p.tocado = ahora;
        guardar_proyecto_json(&carpeta, &p)?;
    }
    f.tocado = ahora;
    if archivar && !con_json {
        f.resto
            .insert(vista::MARCA_ARCHIVADO.into(), serde_json::Value::Bool(true));
    } else {
        f.resto.remove(vista::MARCA_ARCHIVADO);
    }
    let mut ficha = FichaChat::from(&*f);
    // Con `proyecto.json` la verdad es el, no la marca de la ficha.
    ficha.archivado = archivar;
    indice.guardar(raiz)?;
    Ok(ficha)
}

/// Lo que diria el envio por codigo de este paquete (sus tres codigos):
/// lo mismo que monta `envio.rs` al mandar un proyecto.
fn elemento_de_paquete(p: &Paquete, bytes: i64) -> Elemento {
    let pr = &p.proyecto;
    let origen = pr
        .resto
        .get("origen")
        .and_then(|v| v.as_str())
        .unwrap_or(&pr.id);
    Elemento {
        tipo: envio::PROYECTO.into(),
        nombre: pr.nombre.clone(),
        bytes,
        mime: Some("application/zip".into()),
        identidad: format!("proyecto:{origen}"),
        proyecto: None,
        proyecto_nombre: None,
        creado: pr.creado,
        uid: pr.uid.clone(),
        codigo_de_chat: None,
        aparato: pr.aparato.clone(),
    }
}

fn abrir_paquete(ruta: &Path) -> Result<(Paquete, Elemento), ErrorNucleo> {
    let bytes = std::fs::metadata(ruta).map(|m| m.len() as i64).unwrap_or(0);
    let p = Paquete::abrir(ruta).map_err(|e| ErrorNucleo::Argumento {
        detalle: format!("no es un proyecto de PixPin: {e}"),
    })?;
    let e = elemento_de_paquete(&p, bytes);
    Ok((p, e))
}

/// El nombre del proyecto de aqui que ES el de ese `.pixpin` (los tres
/// codigos), si lo hay: entonces se pregunta «Actualizar el que tengo» o
/// «Crear como nuevo», como en el envio por codigo.
pub fn ya_esta(raiz: &Path, ruta: &Path) -> Result<Option<String>, ErrorNucleo> {
    let (_, e) = abrir_paquete(ruta)?;
    Ok(Indice::leer(raiz)
        .misma(&crate::recepcion::ficha_que_llega(&e))
        .map(|f| f.nombre.clone()))
}

/// **Abre un `.pixpin` de fuera en la lista de proyectos**
/// ([`crate::recepcion::guardar_un_proyecto`]): encima del que ya hay si es
/// el mismo (con su copia de antes), o aparte si `como_nuevo`.
pub fn importar(
    raiz: &Path,
    ruta: &Path,
    aparato: &str,
    como_nuevo: bool,
    ahora: i64,
) -> Result<FichaChat, ErrorNucleo> {
    let (p, e) = abrir_paquete(ruta)?;
    let (ficha, _) =
        crate::recepcion::guardar_un_proyecto(raiz, &e, p, aparato, como_nuevo, ahora)?;
    Ok(FichaChat::from(&ficha))
}

#[uniffi::export]
impl Nucleo {
    /// **Cambia el nombre de un proyecto** (Windows, «Cambiar el nombre…»
    /// de su fila). En blanco o «Mensajes guardados» no vale.
    pub fn renombrar_proyecto(
        &self,
        ficha: String,
        nombre: String,
    ) -> Result<FichaChat, ErrorNucleo> {
        renombrar(self.ruta(), &ficha, &nombre, crate::ahora_ms())
    }

    /// **Archiva (o desarchiva) un proyecto** (Windows, «Archivar» de su
    /// fila). «Mensajes guardados» y el asistente no se archivan.
    pub fn archivar_proyecto(
        &self,
        ficha: String,
        archivar: bool,
    ) -> Result<FichaChat, ErrorNucleo> {
        crate::proyectos::archivar(self.ruta(), &ficha, archivar, crate::ahora_ms())
    }

    /// El nombre del proyecto de aqui que ya es ese `.pixpin`, si lo hay
    /// (para preguntar «Actualizar el que tengo» / «Crear como nuevo»).
    pub fn proyecto_que_ya_esta(&self, ruta: String) -> Result<Option<String>, ErrorNucleo> {
        ya_esta(self.ruta(), Path::new(&ruta))
    }

    /// **Abre un `.pixpin`** que llega de fuera en la lista de proyectos:
    /// pone al dia el que ya hay (con copia de antes) o, con `como_nuevo`,
    /// entra aparte con codigos nuevos.
    pub fn importar_proyecto(
        &self,
        ruta: String,
        como_nuevo: bool,
    ) -> Result<FichaChat, ErrorNucleo> {
        let aparato = self.identidad()?.yo.codigo();
        importar(
            self.ruta(),
            Path::new(&ruta),
            &aparato,
            como_nuevo,
            crate::ahora_ms(),
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    fn carpeta(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-proyectos-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn con_uno(raiz: &Path, id: &str) {
        let mut i = Indice::default();
        i.proyectos.push(almacen::Ficha {
            id: id.into(),
            nombre: "Tesis".into(),
            tocado: 1,
            ..Default::default()
        });
        i.guardar(raiz).unwrap();
    }

    fn ficha(raiz: &Path, id: &str) -> almacen::Ficha {
        Indice::leer(raiz)
            .proyectos
            .into_iter()
            .find(|f| f.id == id)
            .unwrap()
    }

    #[test]
    fn el_nombre_limpio_es_la_primera_linea_recortada() {
        assert_eq!(nombre_limpio("  Obra  \nmas").as_deref(), Some("Obra"));
        assert_eq!(nombre_limpio("   \n"), None);
        assert_eq!(nombre_limpio(""), None);
    }

    #[test]
    fn renombrar_va_al_indice_y_al_proyecto_json_y_sube_arriba() {
        let raiz = carpeta("renombrar");
        con_uno(&raiz, "p1");
        let c = almacen::carpeta(&raiz, "p1");
        guardar_proyecto_json(
            &c,
            &Proyecto {
                id: "p1".into(),
                nombre: "Tesis".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let f = renombrar(&raiz, "p1", "  Obra nueva \n", 500).unwrap();
        assert_eq!(f.nombre, "Obra nueva");
        assert_eq!(f.tocado, 500);
        assert_eq!(ficha(&raiz, "p1").nombre, "Obra nueva");
        let p = leer_proyecto_json(&c);
        assert_eq!((p.nombre.as_str(), p.tocado), ("Obra nueva", 500));
        // Casos negativos: en blanco, uno que no esta, un id con barras.
        assert!(renombrar(&raiz, "p1", "   ", 600).is_err());
        assert_eq!(ficha(&raiz, "p1").tocado, 500, "en blanco no toca nada");
        assert!(matches!(
            renombrar(&raiz, "nadie", "X", 600),
            Err(ErrorNucleo::NoExiste { .. })
        ));
        assert!(renombrar(&raiz, "../x", "X", 600).is_err());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn sin_proyecto_json_renombrar_no_lo_crea() {
        let raiz = carpeta("renombrar-sin-json");
        con_uno(&raiz, "p1");
        renombrar(&raiz, "p1", "Otra", 9).unwrap();
        assert!(!almacen::carpeta(&raiz, "p1").join("proyecto.json").exists());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn archivar_sin_proyecto_json_pone_la_marca_y_desarchivar_la_quita() {
        let raiz = carpeta("archivar-marca");
        con_uno(&raiz, "p1");
        let f = archivar(&raiz, "p1", true, 70).unwrap();
        assert!(f.archivado);
        let g = ficha(&raiz, "p1");
        assert_eq!(
            g.resto.get(vista::MARCA_ARCHIVADO),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(g.tocado, 70);
        let f = archivar(&raiz, "p1", false, 80).unwrap();
        assert!(!f.archivado);
        assert!(
            !ficha(&raiz, "p1")
                .resto
                .contains_key(vista::MARCA_ARCHIVADO)
        );
        // Caso negativo: uno que no esta.
        assert!(archivar(&raiz, "nadie", true, 90).is_err());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn archivar_con_proyecto_json_va_al_json_y_no_a_la_marca() {
        let raiz = carpeta("archivar-json");
        con_uno(&raiz, "p1");
        let c = almacen::carpeta(&raiz, "p1");
        guardar_proyecto_json(
            &c,
            &Proyecto {
                id: "p1".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let f = archivar(&raiz, "p1", true, 70).unwrap();
        assert!(f.archivado);
        assert!(leer_proyecto_json(&c).archivado);
        assert!(
            !ficha(&raiz, "p1")
                .resto
                .contains_key(vista::MARCA_ARCHIVADO)
        );
        archivar(&raiz, "p1", false, 71).unwrap();
        assert!(!leer_proyecto_json(&c).archivado);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn mensajes_guardados_ni_se_renombra_ni_se_archiva() {
        let raiz = carpeta("guardados");
        let g = almacen::asegurar_guardados(&raiz, 1, "ZZZZ").unwrap();
        assert!(renombrar(&raiz, &g.id, "Otro", 2).is_err());
        assert!(archivar(&raiz, &g.id, true, 2).is_err());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    fn un_pixpin(raiz: &Path, nombre: &str) -> PathBuf {
        let proyecto = Proyecto {
            id: "p-movil".into(),
            nombre: nombre.into(),
            hojas: vec![pixpin_proyecto::Hoja {
                id: "h1".into(),
                nombre: "Plano".into(),
                dibujo: Some("d1".into()),
                uid: Some("AAAAAAAAAA".into()),
                ..Default::default()
            }],
            tocado: 1_757_939_357_123,
            uid: Some("VVT587BFCA".into()),
            creado: 1_757_939_357_123,
            aparato: Some("K7Q2".into()),
            ..Default::default()
        };
        let mut p = Paquete::nuevo(Default::default(), proyecto);
        p.poner_entrada("lienzos/d1.excalidraw", b"{}".to_vec());
        let ruta = raiz.join(format!("{nombre}.pixpin"));
        std::fs::write(&ruta, p.a_bytes().unwrap()).unwrap();
        ruta
    }

    #[test]
    fn importar_un_pixpin_lo_mete_y_otra_vez_pregunta_y_actualiza_o_duplica() {
        let raiz = carpeta("importar");
        let fuera = carpeta("importar-fuera");
        let ruta = un_pixpin(&fuera, "Tesis");
        assert_eq!(ya_esta(&raiz, &ruta).unwrap(), None);
        let f = importar(&raiz, &ruta, "ZZZZ", false, 1_757_939_400_000).unwrap();
        assert_eq!(f.nombre, "Tesis");
        assert_eq!(Indice::leer(&raiz).proyectos.len(), 1);
        // Ya esta: se ofrece actualizar o crear como nuevo.
        assert_eq!(ya_esta(&raiz, &ruta).unwrap().as_deref(), Some("Tesis"));
        let g = importar(&raiz, &ruta, "ZZZZ", false, 1_757_939_500_000).unwrap();
        assert_eq!(g.id, f.id, "actualizar el que tengo conserva su id");
        assert_eq!(Indice::leer(&raiz).proyectos.len(), 1);
        let h = importar(&raiz, &ruta, "ZZZZ", true, 1_757_939_600_000).unwrap();
        assert_ne!(h.id, f.id);
        assert_eq!(Indice::leer(&raiz).proyectos.len(), 2);
        // Caso negativo: algo que no es un `.pixpin`.
        let roto = fuera.join("roto.pixpin");
        std::fs::write(&roto, b"no soy un zip").unwrap();
        assert!(importar(&raiz, &roto, "ZZZZ", false, 1).is_err());
        assert!(ya_esta(&raiz, &roto).is_err());
        assert_eq!(Indice::leer(&raiz).proyectos.len(), 2);
        let _ = std::fs::remove_dir_all(&raiz);
        let _ = std::fs::remove_dir_all(&fuera);
    }
}
