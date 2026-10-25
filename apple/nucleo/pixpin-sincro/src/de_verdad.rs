//! **Dos aparatos de mentira, sincronizandose de verdad.**
//!
//! Puerto de `SincronizarDeVerdadTest.kt`: cada «aparato» es una carpeta
//! `files` como la del movil ([DiscoAndroid]) y se hablan por un socket en
//! esta maquina con el protocolo y el cifrado de verdad. Las dos carpetas y
//! sus prefijos son distintos a proposito: asi se comprueba que lo que viaja
//! no lleva rutas de un aparato dentro. Si esto pasa aqui igual que en
//! Kotlin, el `Respondedor` y la `Sesion` de Rust hacen lo mismo que los del
//! movil; las pruebas del PC contra este «movil» estan en `pixpin-proyecto`.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::PathBuf;

use serde_json::json;

use crate::canonico::{self, Json};
use crate::disco::{Disco, GENERAL, Marca};
use crate::disco_android::DiscoAndroid;
use crate::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar, sincronizar};
use crate::kotlin;
use crate::mezcla::Mando;
use crate::protocolo::{self, ErrorSincro, Hecho};

struct Par {
    dir: PathBuf,
    tel: DiscoAndroid,
    tab: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Par {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Par {
    let dir = std::env::temp_dir().join(format!(
        "pixpin-de-verdad-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let tel = DiscoAndroid::nuevo(dir.join("telefono/files"), "tel");
    let tab = DiscoAndroid::nuevo(dir.join("tableta-de-otro-usuario/data/files"), "tab");
    presentar(&tel, "id-tel", "Teléfono");
    presentar(&tab, "id-tab", "Tableta");
    Par {
        dir,
        tel,
        tab,
        reloj: Reloj::nuevo(),
    }
}

impl Par {
    fn emparejar(&self) {
        crear_grupo(&self.tel, "ABCDE23456", self.reloj.ahora());
        conectado(
            &self.tab,
            &self.tel,
            true,
            Some("ABCDE23456"),
            &self.reloj,
            |_| {},
        )
        .unwrap();
    }

    fn sincronizar(&self, chats: &[&str]) -> Hecho {
        sincronizar(&self.tel, &self.tab, chats, &self.reloj, &|| {}).0
    }

    fn general(&self) -> Hecho {
        self.sincronizar(&[GENERAL])
    }

    /// Una vuelta entera desde el telefono, como la de la pantalla. Con
    /// `responder`, contesta a «¿Que sincronizar?»; sin el, es «con todos».
    fn vuelta_del_tel(
        &self,
        responder: Option<
            &mut dyn FnMut(crate::vuelta::Pregunta) -> Option<crate::vuelta::Eleccion>,
        >,
    ) -> crate::vuelta::Vuelta {
        conectado(&self.tel, &self.tab, false, None, &self.reloj, |s| {
            let mut hecho = Hecho::default();
            match responder {
                Some(r) => {
                    let mut elegir = |_: &crate::mensajes::Aparato, pr| r(pr);
                    crate::vuelta::una(
                        s,
                        Some(&mut elegir),
                        &mut hecho,
                        "",
                        &|| self.reloj.ahora(),
                        &mut |_| {},
                    )
                }
                None => {
                    crate::vuelta::una(s, None, &mut hecho, "", &|| self.reloj.ahora(), &mut |_| {})
                }
            }
            .unwrap()
        })
        .unwrap()
    }

    /// Lo mismo, pero con el telefono al mando: ver
    /// [crate::protocolo::Sesion::quien_manda].
    fn mandando(&self, chats: &[&str]) -> Hecho {
        conectado(&self.tel, &self.tab, false, None, &self.reloj, |s| {
            s.quien_manda(Mando::LoMioManda);
            let mut hecho = Hecho::default();
            for chat in chats {
                let prep = s.preparar(chat).unwrap();
                s.aplicar(&prep, &mut hecho).unwrap();
                let arch = s.preparar_archivos(&prep).unwrap();
                s.aplicar_archivos(&arch, &mut hecho, &mut |_| {}).unwrap();
                s.cerrar(&prep, self.reloj.ahora()).unwrap();
            }
            hecho
        })
        .unwrap()
    }
}

struct Nuevo<'a> {
    id: &'a str,
    texto: &'a str,
    clase: &'a str,
    proyecto: Option<&'a str>,
    ruta: Option<String>,
    referencia: Option<&'a str>,
}

fn nota<'a>(id: &'a str, texto: &'a str) -> Nuevo<'a> {
    Nuevo {
        id,
        texto,
        clase: "NOTA",
        proyecto: None,
        ruta: None,
        referencia: None,
    }
}

/// Como guarda el chat: el siguiente numero de su conversacion y la letra.
fn mensaje(d: &DiscoAndroid, reloj: &Reloj, n: Nuevo<'_>) -> Json {
    let suyos: Vec<Json> = d
        .leer_mensajes()
        .into_iter()
        .filter(|m| kotlin::cadena(m, "proyecto") == n.proyecto)
        .collect();
    let numero = suyos
        .iter()
        .filter_map(|m| kotlin::numero(m, "numero"))
        .max()
        .unwrap_or(0)
        + 1;
    let letra = d.identidad().unwrap().yo.letra;
    let m = Json::de_valor(&json!({
        "id": n.id, "cuando": reloj.tic(), "clase": n.clase, "texto": n.texto,
        "proyecto": n.proyecto, "ruta": n.ruta, "referencia": n.referencia,
        "numero": numero, "letra": letra,
    }));
    d.anadir_mensaje(&m).unwrap();
    kotlin::normalizar_mensaje(&m).unwrap()
}

/// Como hace el chat: reescribe sin el mensaje y deja la marca.
fn borrar(d: &DiscoAndroid, reloj: &Reloj, id: &str) {
    let antes = d.leer_mensajes();
    let despues: Vec<Json> = antes
        .iter()
        .filter(|m| kotlin::cadena(m, "id") != Some(id))
        .cloned()
        .collect();
    d.escribir_mensajes(&despues).unwrap();
    let letra = d.identidad().unwrap().yo.letra;
    let cuando = reloj.tic();
    let marcas: Vec<Marca> = antes
        .iter()
        .filter(|m| kotlin::cadena(m, "id") == Some(id))
        .map(|m| Marca {
            chat: kotlin::chat_de(m),
            sena: kotlin::de_chat(m)
                .or_else(|| {
                    letra
                        .as_ref()
                        .map(|l| format!("{}{l}", kotlin::numero(m, "numero").unwrap_or(0)))
                })
                .unwrap_or_default(),
            cuando,
            uid: Some(kotlin::unico(m)),
        })
        .collect();
    d.anotar_borrados(&marcas).unwrap();
}

fn editar(d: &DiscoAndroid, id: &str, cambio: impl Fn(&mut Json)) {
    let lista: Vec<Json> = d
        .leer_mensajes()
        .into_iter()
        .map(|mut m| {
            if kotlin::cadena(&m, "id") == Some(id) {
                cambio(&mut m);
            }
            m
        })
        .collect();
    d.escribir_mensajes(&lista).unwrap();
}

fn gz(ruta: &std::path::Path, texto: &str) {
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    let mut e = flate2::write::GzEncoder::new(
        std::fs::File::create(ruta).unwrap(),
        flate2::Compression::default(),
    );
    e.write_all(texto.as_bytes()).unwrap();
    e.finish().unwrap();
}

fn dibujo(d: &DiscoAndroid, id: &str, texto: &str, foto: Option<&str>) {
    let files = match foto {
        Some(f) => format!(
            r#""{f}":{{"path":"{}"}}"#,
            d.absoluta(&format!("pins/draw/files/{f}"))
        ),
        None => String::new(),
    };
    let json = format!(r#"{{"elements":[{{"text":"{texto}"}}],"files":{{{files}}}}}"#);
    gz(
        &d.raiz().join(format!("pins/draw/{id}.excalidraw.gz")),
        &json,
    );
}

fn texto_del_dibujo(d: &DiscoAndroid, id: &str) -> String {
    let b = std::fs::read(d.raiz().join(format!("pins/draw/{id}.excalidraw.gz"))).unwrap();
    let mut s = String::new();
    flate2::read::GzDecoder::new(&b[..])
        .read_to_string(&mut s)
        .unwrap();
    s
}

fn lienzo(d: &DiscoAndroid, id: &str, figuras: &[String]) {
    gz(
        &d.raiz().join(format!("pins/draw/{id}.excalidraw.gz")),
        &format!(r#"{{"elements":[{}],"files":{{}}}}"#, figuras.join(",")),
    );
}

fn fig(id: &str, x: i64, color: &str, v: i64, updated: i64) -> String {
    format!(
        r#"{{"id":"{id}","type":"rectangle","x":{x},"strokeColor":"{color}","version":{v},"versionNonce":7,"updated":{updated}}}"#
    )
}

fn f(id: &str) -> String {
    fig(id, 0, "#000", 1, 1)
}

fn figuras(texto: &str) -> Vec<String> {
    texto
        .split("\"id\":\"")
        .skip(1)
        .filter_map(|t| t.split('"').next().map(str::to_string))
        .collect()
}

fn senas(d: &DiscoAndroid, chat: &str) -> BTreeSet<String> {
    d.leer_mensajes()
        .iter()
        .filter(|m| kotlin::chat_de(m) == chat)
        .filter_map(kotlin::de_chat)
        .collect()
}

fn texto_de(d: &DiscoAndroid) -> Vec<String> {
    d.leer_mensajes()
        .iter()
        .map(|m| kotlin::cadena(m, "texto").unwrap_or_default().to_string())
        .collect()
}

fn proyecto(d: &DiscoAndroid, p: serde_json::Value) {
    d.guardar_proyecto(&kotlin::normalizar_proyecto(&Json::de_valor(&p)).unwrap())
        .unwrap();
}

fn hojas(d: &DiscoAndroid) -> Vec<String> {
    let p = d.leer_proyectos().into_iter().next().unwrap();
    p.como_objeto()
        .unwrap()
        .obtener("hojas")
        .unwrap()
        .como_lista()
        .unwrap()
        .iter()
        .map(|h| kotlin::cadena(h, "id").unwrap().to_string())
        .collect()
}

// ------------------------------------------------------------------ pruebas

#[test]
fn unirse_da_la_siguiente_letra_y_los_dos_se_conocen() {
    let p = montar("unirse");
    p.emparejar();
    let tel = p.tel.identidad().unwrap();
    let tab = p.tab.identidad().unwrap();
    assert_eq!(tel.yo.letra.as_deref(), Some("a"));
    assert_eq!(tab.yo.letra.as_deref(), Some("b"));
    assert_eq!(tel.codigo, tab.codigo);
    let ids = |i: &crate::disco::Identidad| -> BTreeSet<String> {
        i.miembros.iter().map(|m| m.id.clone()).collect()
    };
    assert_eq!(ids(&tel), ["id-tab".to_string(), "id-tel".into()].into());
    assert_eq!(ids(&tab), ids(&tel));
}

#[test]
fn con_otro_codigo_no_se_entienden() {
    let p = montar("otro-codigo");
    crear_grupo(&p.tel, "ABCDE23456", 1);
    crear_grupo(&p.tab, "ZZZZZ99999", 1);
    assert!(conectado(&p.tel, &p.tab, false, None, &p.reloj, |_| {}).is_err());
}

#[test]
fn lo_de_antes_del_grupo_se_sella_con_el_codigo_del_aparato() {
    let p = montar("antes-del-grupo");
    mensaje(&p.tel, &p.reloj, nota("viejo1", "de antes"));
    mensaje(&p.tab, &p.reloj, nota("viejo2", "de antes también"));
    p.emparejar();
    let tel = crate::grupo::codigo_de_aparato("id-tel");
    let tab = crate::grupo::codigo_de_aparato("id-tab");
    assert_eq!(senas(&p.tel, GENERAL), [format!("1·{tel}")].into());
    assert_eq!(senas(&p.tab, GENERAL), [format!("1·{tab}")].into());
    let m = p.tel.leer_mensajes().pop().unwrap();
    assert_eq!(
        kotlin::cadena(&m, "uid"),
        Some(kotlin::codigo_de("m:viejo1").as_str())
    );
    p.general();
    assert_eq!(
        senas(&p.tel, GENERAL),
        [format!("1·{tel}"), format!("1·{tab}")].into()
    );
    assert_eq!(senas(&p.tel, GENERAL), senas(&p.tab, GENERAL));
}

#[test]
fn la_primera_vuelta_junta_los_dos_chats_sin_preguntar_nada() {
    let p = montar("primera");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "hola desde el teléfono"));
    mensaje(&p.tel, &p.reloj, nota("t2", "otra"));
    mensaje(&p.tab, &p.reloj, nota("b1", "hola desde la tableta"));
    assert_eq!(p.general().fusionados, 0);
    assert_eq!(
        senas(&p.tel, GENERAL),
        ["1a".to_string(), "2a".into(), "1b".into()].into()
    );
    assert_eq!(senas(&p.tel, GENERAL), senas(&p.tab, GENERAL));
    let orden = |d: &DiscoAndroid| {
        let mut v: Vec<(i64, String)> = d
            .leer_mensajes()
            .iter()
            .map(|m| {
                (
                    kotlin::numero(m, "cuando").unwrap(),
                    kotlin::cadena(m, "id").unwrap().to_string(),
                )
            })
            .collect();
        v.sort();
        v
    };
    assert_eq!(orden(&p.tel), orden(&p.tab));
}

#[test]
fn sincronizar_dos_veces_seguidas_no_mueve_nada_y_los_sellos_cuadran() {
    let p = montar("dos-veces");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "hola"));
    mensaje(&p.tab, &p.reloj, nota("b1", "adiós"));
    p.general();
    // Lo acordado quedo igual en los dos, con el mismo sello.
    let a = p.tel.base("id-tab", GENERAL).unwrap();
    let b = p.tab.base("id-tel", GENERAL).unwrap();
    assert_eq!(a.sello(), b.sello());
    assert_eq!(a.mensajes.len(), 2);
    conectado(&p.tel, &p.tab, false, None, &p.reloj, |s| {
        let prep = s.preparar(GENERAL).unwrap();
        assert!(prep.pasos.is_empty(), "sin pasos: {:?}", prep.pasos);
        s.aplicar(&prep, &mut Hecho::default()).unwrap();
        assert!(s.preparar_archivos(&prep).unwrap().pasos.is_empty());
    })
    .unwrap();
}

#[test]
fn lo_que_cambio_en_un_solo_lado_pasa_solo() {
    let p = montar("un-lado");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "versión 1"));
    p.general();
    editar(&p.tab, "t1", |m| {
        kotlin::poner(m, "texto", Json::cadena("versión 2, desde la tableta"))
    });
    assert_eq!(p.general().fusionados, 0);
    assert_eq!(texto_de(&p.tel), ["versión 2, desde la tableta"]);
}

#[test]
fn una_nota_cambiada_en_los_dos_se_junta_parrafo_por_parrafo() {
    let p = montar("parrafos");
    p.emparejar();
    mensaje(
        &p.tel,
        &p.reloj,
        nota("t1", "Introducción\n\nMétodo\n\nResultados"),
    );
    p.general();
    editar(&p.tel, "t1", |m| {
        kotlin::poner(
            m,
            "texto",
            Json::cadena("Introducción corregida\n\nMétodo\n\nResultados"),
        )
    });
    editar(&p.tab, "t1", |m| {
        kotlin::poner(
            m,
            "texto",
            Json::cadena("Introducción\n\nMétodo\n\nResultados\n\nConclusiones"),
        )
    });
    assert_eq!(p.general().fusionados, 1);
    let esperado = "Introducción corregida\n\nMétodo\n\nResultados\n\nConclusiones";
    assert_eq!(texto_de(&p.tel), [esperado]);
    assert_eq!(texto_de(&p.tab), [esperado]);
    conectado(&p.tel, &p.tab, false, None, &p.reloj, |s| {
        assert!(s.preparar(GENERAL).unwrap().pasos.is_empty())
    })
    .unwrap();
}

#[test]
fn lo_borrado_en_un_lado_y_sin_tocar_en_el_otro_se_borra_en_los_dos() {
    let p = montar("borrado");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "para borrar"));
    mensaje(&p.tel, &p.reloj, nota("t2", "se queda"));
    p.general();
    borrar(&p.tel, &p.reloj, "t1");
    p.general();
    assert_eq!(senas(&p.tab, GENERAL), ["2a".to_string()].into());
    assert_eq!(senas(&p.tel, GENERAL), ["2a".to_string()].into());
    p.general();
    assert_eq!(
        senas(&p.tel, GENERAL),
        ["2a".to_string()].into(),
        "no resucita"
    );
}

#[test]
fn una_marca_de_borrado_de_antes_de_los_codigos_sigue_borrando() {
    let p = montar("marca-vieja");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "viejo"));
    mensaje(&p.tel, &p.reloj, nota("t2", "queda"));
    p.general();
    let quedan: Vec<Json> = p
        .tel
        .leer_mensajes()
        .into_iter()
        .filter(|m| kotlin::cadena(m, "id") != Some("t1"))
        .collect();
    p.tel.escribir_mensajes(&quedan).unwrap();
    p.tel
        .anotar_borrados(&[Marca {
            chat: GENERAL.into(),
            sena: "1a".into(),
            cuando: p.reloj.tic(),
            uid: None,
        }])
        .unwrap();
    p.general();
    assert_eq!(senas(&p.tab, GENERAL), ["2a".to_string()].into());
    p.general();
    assert_eq!(senas(&p.tel, GENERAL), ["2a".to_string()].into());
}

#[test]
fn lo_borrado_en_un_lado_y_cambiado_en_el_otro_se_queda_en_los_dos() {
    let p = montar("borrado-cambiado");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "borrador"));
    p.general();
    borrar(&p.tel, &p.reloj, "t1");
    editar(&p.tab, "t1", |m| {
        kotlin::poner(m, "texto", Json::cadena("borrador mejorado en la tableta"))
    });
    p.general();
    assert_eq!(texto_de(&p.tel), ["borrador mejorado en la tableta"]);
    assert_eq!(texto_de(&p.tab), ["borrador mejorado en la tableta"]);
    p.general();
    assert_eq!(p.tel.leer_mensajes().len(), 1);
}

#[test]
fn los_numeros_nuevos_de_cada_aparato_no_chocan() {
    let p = montar("numeros");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "uno"));
    p.general();
    mensaje(&p.tel, &p.reloj, nota("t2", "segundo del teléfono"));
    mensaje(&p.tab, &p.reloj, nota("b2", "segundo de la tableta"));
    p.general();
    assert_eq!(
        senas(&p.tel, GENERAL),
        ["1a".to_string(), "2a".into(), "2b".into()].into()
    );
    assert_eq!(senas(&p.tel, GENERAL), senas(&p.tab, GENERAL));
}

#[test]
fn los_adjuntos_viajan_y_llegan_con_la_ruta_del_otro_aparato() {
    let p = montar("adjuntos");
    p.emparejar();
    let bytes: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
    let pdf = p.tel.raiz().join("guardados/123_plano.pdf");
    std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
    std::fs::write(&pdf, &bytes).unwrap();
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "ARCHIVO",
            ruta: Some(p.tel.absoluta("guardados/123_plano.pdf")),
            ..nota("t1", "")
        },
    );
    p.general();
    let llegado = p.tab.leer_mensajes().pop().unwrap();
    let ruta = kotlin::cadena(&llegado, "ruta").unwrap();
    assert!(
        ruta.starts_with(p.tab.prefijo()),
        "con la carpeta de la tableta: {ruta}"
    );
    assert_eq!(
        std::fs::read(p.tab.raiz().join("guardados/123_plano.pdf")).unwrap(),
        bytes
    );
}

#[test]
fn un_lienzo_viaja_con_su_foto_y_sin_rutas_del_telefono_dentro() {
    let p = montar("lienzo-foto");
    p.emparejar();
    let foto = p.tel.raiz().join("pins/draw/files/foto1");
    std::fs::create_dir_all(foto.parent().unwrap()).unwrap();
    std::fs::write(&foto, [1, 2, 3]).unwrap();
    dibujo(&p.tel, "d1", "planta", Some("foto1"));
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d1"),
            ..nota("t1", "")
        },
    );
    p.general();
    let texto = texto_del_dibujo(&p.tab, "d1");
    assert!(texto.contains("planta"));
    assert!(
        texto.contains(&p.tab.absoluta("pins/draw/files/foto1")),
        "{texto}"
    );
    assert!(!texto.contains(p.tel.prefijo()));
    assert_eq!(
        std::fs::read(p.tab.raiz().join("pins/draw/files/foto1")).unwrap(),
        [1, 2, 3]
    );
}

#[test]
fn un_lienzo_dibujado_en_los_dos_se_junta_figura_por_figura() {
    let p = montar("lienzo-junto");
    p.emparejar();
    lienzo(&p.tel, "d1", &[f("A")]);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d1"),
            ..nota("t1", "Planta baja")
        },
    );
    p.general();
    lienzo(&p.tel, "d1", &[f("A"), f("B")]);
    lienzo(&p.tab, "d1", &[fig("A", 0, "#f00", 2, 50), f("C")]);
    let hecho = p.general();
    assert_eq!(hecho.fusionados, 1);
    assert_eq!(figuras(&texto_del_dibujo(&p.tel, "d1")), ["A", "B", "C"]);
    assert_eq!(figuras(&texto_del_dibujo(&p.tab, "d1")), ["A", "B", "C"]);
    assert!(texto_del_dibujo(&p.tel, "d1").contains("#f00"));
    assert_eq!(
        canonico::de(&texto_del_dibujo(&p.tel, "d1")),
        canonico::de(&texto_del_dibujo(&p.tab, "d1"))
    );
    assert_eq!(p.general().fusionados, 0);
    conectado(&p.tel, &p.tab, false, None, &p.reloj, |s| {
        let prep = s.preparar(GENERAL).unwrap();
        s.aplicar(&prep, &mut Hecho::default()).unwrap();
        assert!(s.preparar_archivos(&prep).unwrap().pasos.is_empty());
    })
    .unwrap();
}

#[test]
fn una_figura_borrada_en_un_lado_y_movida_en_el_otro_vuelve() {
    let p = montar("figura-vuelve");
    p.emparejar();
    lienzo(&p.tel, "d1", &[f("A"), f("B")]);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d1"),
            ..nota("t1", "Planta")
        },
    );
    p.general();
    lienzo(&p.tel, "d1", &[f("A")]);
    lienzo(&p.tab, "d1", &[f("A"), fig("B", 300, "#000", 2, 90)]);
    let hecho = p.general();
    assert_eq!(figuras(&texto_del_dibujo(&p.tel, "d1")), ["A", "B"]);
    assert_eq!(hecho.rescatados, 1);
}

#[test]
fn solo_viajan_los_cambios_de_un_lienzo_grande() {
    let p = montar("parches");
    p.emparejar();
    let muchas: Vec<String> = (1..=400)
        .map(|i| fig(&format!("f{i}"), i, "#000", 1, 1))
        .collect();
    lienzo(&p.tel, "d1", &muchas);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d1"),
            ..nota("t1", "Grande")
        },
    );
    let (_, primera) = sincronizar(&p.tel, &p.tab, &[GENERAL], &p.reloj, &|| {});
    let cambiadas: Vec<String> = muchas
        .iter()
        .map(|x| {
            if x.contains("\"f9\"") {
                fig("f9", 999, "#000", 2, 5)
            } else {
                x.clone()
            }
        })
        .collect();
    lienzo(&p.tab, "d1", &cambiadas);
    let (hecho, segunda) = sincronizar(&p.tel, &p.tab, &[GENERAL], &p.reloj, &|| {});
    assert!(texto_del_dibujo(&p.tel, "d1").contains("999"));
    assert!(segunda * 3 < primera, "{segunda} contra {primera}");
    assert!(hecho.ahorrados > 0);
}

#[test]
fn un_proyecto_nuevo_llega_entero_y_sus_hojas_se_juntan_despues() {
    let p = montar("proyecto");
    p.emparejar();
    dibujo(&p.tab, "d1", "hoja uno", None);
    proyecto(
        &p.tab,
        json!({"id":"pr-1","nombre":"Reforma","hojas":[{"id":"h1","nombre":"Planta","dibujo":"d1"}],"tocado":5}),
    );
    mensaje(
        &p.tab,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            proyecto: Some("pr-1"),
            referencia: Some("d1"),
            ..nota("b1", "Planta")
        },
    );
    p.sincronizar(&["pr-1"]);
    let tel = p.tel.leer_proyectos();
    assert_eq!(kotlin::cadena(&tel[0], "nombre"), Some("Reforma"));
    assert!(texto_del_dibujo(&p.tel, "d1").contains("hoja uno"));
    assert_eq!(senas(&p.tel, "pr-1"), ["1b".to_string()].into());

    // Cada uno anade una hoja: quedan las dos.
    let con = |d: &DiscoAndroid, h: &str, tocado: i64| {
        let mut pr = d.leer_proyectos().pop().unwrap();
        let mut l = pr
            .como_objeto()
            .unwrap()
            .obtener("hojas")
            .unwrap()
            .como_lista()
            .unwrap()
            .to_vec();
        l.push(Json::de_valor(&json!({"id": h})));
        kotlin::poner(&mut pr, "hojas", Json::Lista(l));
        kotlin::poner(&mut pr, "tocado", Json::numero(tocado));
        d.guardar_proyecto(&pr).unwrap();
    };
    con(&p.tel, "h2", 6);
    con(&p.tab, "h3", 7);
    p.sincronizar(&["pr-1"]);
    assert_eq!(hojas(&p.tel), ["h1", "h2", "h3"]);
    let mut t = hojas(&p.tab);
    t.sort();
    assert_eq!(t, ["h1", "h2", "h3"]);

    // Una hoja quitada a mano en un lado se quita en los dos.
    let mut r = p.tab.leer_proyectos().pop().unwrap();
    let l: Vec<Json> = r
        .como_objeto()
        .unwrap()
        .obtener("hojas")
        .unwrap()
        .como_lista()
        .unwrap()
        .iter()
        .filter(|h| kotlin::cadena(h, "id") != Some("h2"))
        .cloned()
        .collect();
    kotlin::poner(&mut r, "hojas", Json::Lista(l));
    kotlin::poner(&mut r, "tocado", Json::numero(8));
    kotlin::poner(&mut r, "quitadas", Json::Lista(vec![Json::cadena("h2")]));
    p.tab.guardar_proyecto(&r).unwrap();
    p.sincronizar(&["pr-1"]);
    assert_eq!(hojas(&p.tel), ["h1", "h3"]);

    // Pero una que simplemente falta, sin marca, vuelve.
    let mut t = p.tel.leer_proyectos().pop().unwrap();
    let l: Vec<Json> = t
        .como_objeto()
        .unwrap()
        .obtener("hojas")
        .unwrap()
        .como_lista()
        .unwrap()
        .iter()
        .filter(|h| kotlin::cadena(h, "id") != Some("h3"))
        .cloned()
        .collect();
    kotlin::poner(&mut t, "hojas", Json::Lista(l));
    kotlin::poner(&mut t, "tocado", Json::numero(9));
    p.tel.guardar_proyecto(&t).unwrap();
    p.sincronizar(&["pr-1"]);
    let mut h = hojas(&p.tel);
    h.sort();
    assert_eq!(h, ["h1", "h3"]);
}

#[test]
fn un_chat_de_cientos_de_mensajes_pasa_en_tandas() {
    let p = montar("tandas");
    p.emparejar();
    let largo = "x".repeat(4_000);
    for i in 0..450 {
        mensaje(
            &p.tel,
            &p.reloj,
            nota(&format!("t{i}"), &format!("nota {i} {largo}")),
        );
    }
    p.general();
    assert_eq!(p.tab.leer_mensajes().len(), 450);
    assert_eq!(senas(&p.tel, GENERAL), senas(&p.tab, GENERAL));
}

#[test]
fn un_aparato_que_ya_esta_sincronizando_dice_que_esta_ocupado() {
    let p = montar("ocupado");
    p.emparejar();
    assert!(protocolo::ocupar(&p.tab));
    conectado(&p.tel, &p.tab, false, None, &p.reloj, |s| {
        match s.catalogo() {
            Err(ErrorSincro::Remoto(e)) => assert_eq!(e, crate::mensajes::OCUPADO),
            otro => panic!("debería decir ocupado: {otro:?}"),
        }
    })
    .unwrap();
    protocolo::soltar(&p.tab);
    // Y el que dirigia queda libre para la siguiente.
    assert!(protocolo::ocupar(&p.tel));
    protocolo::soltar(&p.tel);
}

#[test]
fn lo_tocado_mientras_se_sincronizaba_no_trae_lo_viejo_la_vuelta_siguiente() {
    let p = montar("a-ultima-hora");
    p.emparejar();
    dibujo(&p.tel, "d1", "v1", None);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d1"),
            ..nota("t1", "Planta")
        },
    );
    p.general();
    dibujo(&p.tel, "d1", "v2", None);
    sincronizar(&p.tel, &p.tab, &[GENERAL], &p.reloj, &|| {
        dibujo(&p.tel, "d1", "v3 guardado a última hora", None)
    });
    assert!(texto_del_dibujo(&p.tab, "d1").contains("v2"));
    assert_eq!(p.general().fusionados, 0, "nada que juntar");
    assert!(texto_del_dibujo(&p.tel, "d1").contains("v3"));
    assert!(
        texto_del_dibujo(&p.tab, "d1").contains("v3"),
        "la tableta recibe lo último"
    );
}

#[test]
fn cada_chat_dice_cuando_se_toco_por_ultima_vez() {
    let p = montar("tocado");
    mensaje(&p.tel, &p.reloj, nota("t1", "hola"));
    proyecto(&p.tel, json!({"id":"pr-1","nombre":"Obra","tocado":5}));
    let tocado = p.reloj.ahora();
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("pr-1"),
            ..nota("t2", "en la obra")
        },
    );
    let chats = p.tel.chats().unwrap();
    assert_eq!(chats[0].tocado, tocado - 1);
    assert_eq!(chats[1].tocado, tocado);
}

#[test]
fn el_caso_de_tesis_sin_maestro_no_se_pierde_ningun_lienzo() {
    let p = montar("tesis");
    p.emparejar();
    dibujo(&p.tab, "d1", "uno", None);
    dibujo(&p.tab, "d2", "dos", None);
    proyecto(
        &p.tab,
        json!({"id":"tesis","nombre":"Tesis","hojas":[{"id":"h1","nombre":"Uno","dibujo":"d1"},{"id":"h2","nombre":"Dos","dibujo":"d2"}],"tocado":5}),
    );
    p.sincronizar(&["tesis"]);
    assert_eq!(hojas(&p.tel), ["h1", "h2"]);

    p.reloj.saltar(1_000);
    dibujo(&p.tab, "d3", "tres", None);
    let mut t = p.tab.leer_proyectos().pop().unwrap();
    let mut l = t
        .como_objeto()
        .unwrap()
        .obtener("hojas")
        .unwrap()
        .como_lista()
        .unwrap()
        .to_vec();
    l.push(Json::de_valor(
        &json!({"id":"h3","nombre":"Tres","dibujo":"d3"}),
    ));
    kotlin::poner(&mut t, "hojas", Json::Lista(l));
    kotlin::poner(&mut t, "tocado", Json::numero(6));
    p.tab.guardar_proyecto(&t).unwrap();
    // El telefono queda roto como lo dejo el envio: el mismo lienzo tres
    // veces, tocado despues.
    let mut r = p.tel.leer_proyectos().pop().unwrap();
    let dos = r
        .como_objeto()
        .unwrap()
        .obtener("hojas")
        .unwrap()
        .como_lista()
        .unwrap()[1]
        .clone();
    kotlin::poner(
        &mut r,
        "hojas",
        Json::Lista(vec![dos.clone(), dos.clone(), dos]),
    );
    kotlin::poner(&mut r, "tocado", Json::numero(9));
    p.tel.guardar_proyecto(&r).unwrap();
    assert_eq!(hojas(&p.tel), ["h2"], "nunca dos hojas con el mismo id");

    p.sincronizar(&["tesis"]);
    let mut a = hojas(&p.tel);
    a.sort();
    assert_eq!(a, ["h1", "h2", "h3"]);
    let mut b = hojas(&p.tab);
    b.sort();
    assert_eq!(b, ["h1", "h2", "h3"]);
    assert!(texto_del_dibujo(&p.tel, "d3").contains("tres"));
    // Y en los dos quedo copia de como estaba antes de esa vuelta.
    let copia = &crate::copias::lista(&p.tel, "tesis")[0];
    assert_eq!(copia.hojas, 1);
    assert!(copia.motivo.contains("Tableta"));
    assert_eq!(crate::copias::lista(&p.tab, "tesis")[0].hojas, 3);
}

#[test]
fn dos_aparatos_que_repararon_su_chat_por_separado_acaban_con_uno() {
    let p = montar("registros");
    p.emparejar();
    proyecto(&p.tel, json!({"id":"p","nombre":"Obra","tocado":1}));
    p.sincronizar(&["p"]);
    let id = "registro-p-d1";
    for d in [&p.tel, &p.tab] {
        mensaje(
            d,
            &p.reloj,
            Nuevo {
                clase: "DIBUJO",
                proyecto: Some("p"),
                referencia: Some("d1"),
                ..nota(id, "Lienzo")
            },
        );
    }
    p.sincronizar(&["p"]);
    let cuantos = |d: &DiscoAndroid| {
        d.leer_mensajes()
            .iter()
            .filter(|m| kotlin::cadena(m, "id") == Some(id))
            .count()
    };
    assert_eq!(cuantos(&p.tel), 1);
    assert_eq!(cuantos(&p.tab), 1);
    assert_eq!(senas(&p.tel, "p"), senas(&p.tab, "p"));
    p.sincronizar(&["p"]);
    assert_eq!(cuantos(&p.tel), 1);
}

#[test]
fn un_proyecto_borrado_en_un_lado_se_borra_en_el_otro_con_su_lapida() {
    let p = montar("lapida");
    p.emparejar();
    proyecto(&p.tel, json!({"id":"pr-9","nombre":"Viejo","tocado":1}));
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("pr-9"),
            ..nota("t1", "algo")
        },
    );
    p.sincronizar(&["pr-9"]);
    assert_eq!(p.tab.leer_proyectos().len(), 1);
    p.reloj.saltar(10_000);
    p.tel
        .borrar_chat("pr-9", "prueba", p.reloj.ahora(), "")
        .unwrap();
    // Sin preguntar («con todos») no se borra en ningun lado: se avisa.
    let v = p.vuelta_del_tel(None);
    assert_eq!(p.tab.leer_proyectos().len(), 1, "sin preguntar no se borra");
    assert!(
        v.avisos.iter().any(|a| a.contains("Viejo")),
        "y se dice que quedo pendiente: {:?}",
        v.avisos
    );
    // Preguntando, la lista lo avisa en rojo y al sincronizar se borra alli.
    let mut vistos = Vec::new();
    p.vuelta_del_tel(Some(&mut |pr: crate::vuelta::Pregunta| {
        vistos = pr.se_borran_alli.clone();
        Some(crate::vuelta::Eleccion {
            elegidos: pr.filas.iter().map(|f| f.id.clone()).collect(),
            lo_mio_manda: false,
        })
    }));
    assert_eq!(vistos, ["Viejo"], "la lista dice lo que se borrara alli");
    assert!(p.tab.leer_proyectos().is_empty(), "borrado alli tambien");
    assert!(senas(&p.tab, "pr-9").is_empty());
}

/// Un proyecto que la tableta borro y el telefono no ha tocado desde
/// entonces: una vuelta desde el telefono lo borraria AQUI (en el telefono).
fn borrado_en_la_tableta(etiqueta: &str) -> Par {
    let p = montar(etiqueta);
    p.emparejar();
    proyecto(&p.tel, json!({"id":"pr-9","nombre":"Tesis","tocado":1}));
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("pr-9"),
            ..nota("t1", "horas de trabajo")
        },
    );
    p.sincronizar(&["pr-9"]);
    p.reloj.saltar(10_000);
    p.tab
        .borrar_chat("pr-9", "prueba", p.reloj.ahora(), "")
        .unwrap();
    assert!(p.tab.leer_proyectos().is_empty());
    p
}

fn todo_marcado(pr: &crate::vuelta::Pregunta, lo_mio_manda: bool) -> crate::vuelta::Eleccion {
    crate::vuelta::Eleccion {
        elegidos: pr.filas.iter().map(|f| f.id.clone()).collect(),
        lo_mio_manda,
    }
}

#[test]
fn lo_borrado_en_el_otro_llega_sin_marcar_y_desmarcado_no_se_borra_aqui() {
    // K4: el caso del usuario. Borrar aqui por una lapida solo con la
    // casilla marcada; y la casilla empieza SIN marcar.
    let p = borrado_en_la_tableta("k4-desmarcado");
    let mut fila = None;
    p.vuelta_del_tel(Some(&mut |pr: crate::vuelta::Pregunta| {
        fila = pr.filas.iter().find(|f| f.id == "pr-9").cloned();
        // El usuario pulsa «Sincronizar» sin tocar nada.
        Some(crate::vuelta::Eleccion {
            elegidos: pr
                .filas
                .iter()
                .filter(|f| f.marcado)
                .map(|f| f.id.clone())
                .collect(),
            lo_mio_manda: false,
        })
    }));
    let fila = fila.expect("sale en la lista para poder decidir");
    assert!(fila.se_borra_aqui, "la fila dice que se borraria aqui");
    assert!(!fila.marcado, "y empieza sin marcar");
    assert_eq!(p.tel.leer_proyectos().len(), 1, "desmarcado no se borra");
    assert_eq!(senas(&p.tel, "pr-9").len(), 1, "ni sus mensajes");
    assert!(
        p.tab.leer_proyectos().is_empty(),
        "y tampoco se sincroniza: alli sigue borrado"
    );
}

#[test]
fn lo_borrado_en_el_otro_se_borra_aqui_solo_si_se_marca() {
    let p = borrado_en_la_tableta("k4-marcado");
    p.vuelta_del_tel(Some(&mut |pr: crate::vuelta::Pregunta| {
        Some(todo_marcado(&pr, false))
    }));
    assert!(p.tel.leer_proyectos().is_empty(), "marcado: se borra aqui");
    assert!(senas(&p.tel, "pr-9").is_empty());
    // Con copia antes: se puede volver.
    assert!(
        !crate::copias::lista(&p.tel, "pr-9").is_empty(),
        "queda copia de lo borrado"
    );
}

#[test]
fn sin_preguntar_lo_borrado_en_el_otro_no_se_borra_aqui_y_se_avisa() {
    // «Sincronizar con todos» no ensena la lista: nada se borra.
    let p = borrado_en_la_tableta("k4-sin-preguntar");
    let v = p.vuelta_del_tel(None);
    assert_eq!(p.tel.leer_proyectos().len(), 1);
    assert_eq!(senas(&p.tel, "pr-9").len(), 1, "los mensajes siguen");
    assert!(
        v.avisos
            .iter()
            .any(|a| a.contains("Tesis") && a.contains("Tableta")),
        "{:?}",
        v.avisos
    );
    assert!(p.tab.leer_proyectos().is_empty(), "ni resucita alli");
}

#[test]
fn con_lo_mio_manda_lo_borrado_en_el_otro_no_se_borra_aqui_y_vuelve_alla() {
    // El usuario vacio un aparato y quiere que el otro lo vuelva a llenar.
    let p = borrado_en_la_tableta("k4-mio-manda");
    let mut guardado_antes = p.tel.elegidos("id-tab");
    p.vuelta_del_tel(Some(&mut |pr: crate::vuelta::Pregunta| {
        Some(todo_marcado(&pr, true))
    }));
    assert_eq!(p.tel.leer_proyectos().len(), 1, "aqui no se borra nada");
    assert_eq!(p.tab.leer_proyectos().len(), 1, "y alli vuelve entero");
    assert_eq!(senas(&p.tab, "pr-9"), senas(&p.tel, "pr-9"));
    assert!(
        p.tab.lapidas().iter().all(|l| l.chat != "pr-9"),
        "alli se levanta la lapida, o la siguiente vuelta lo quitaria"
    );
    // Lo mio manda es de una vez: no cambia lo elegido de siempre.
    assert_eq!(p.tel.elegidos("id-tab"), guardado_antes.take());
}

// ------------------------------------------------- lo mio manda (una direccion)

#[test]
fn lo_borrado_en_el_otro_se_borra_aqui_de_siempre_y_con_lo_mio_manda_vuelve_alla() {
    // El mismo montaje dos veces: primero como siempre, para dejar escrito
    // que `Acordar` no cambia, y luego con el telefono al mando.
    let normal = montar("mando-borrado-normal");
    let mando = montar("mando-borrado-mando");
    for (p, al_mando) in [(&normal, false), (&mando, true)] {
        p.emparejar();
        mensaje(&p.tel, &p.reloj, nota("t1", "la que borran alli"));
        p.general();
        assert_eq!(p.tab.leer_mensajes().len(), 1);
        p.reloj.saltar(10_000);
        borrar(&p.tab, &p.reloj, "t1");
        if al_mando {
            p.mandando(&[GENERAL]);
        } else {
            p.general();
        }
        if al_mando {
            assert_eq!(texto_de(&p.tel), ["la que borran alli"], "aqui no se borra");
            assert_eq!(texto_de(&p.tab), ["la que borran alli"], "y vuelve alla");
        } else {
            assert!(texto_de(&p.tel).is_empty(), "de siempre: se borra aqui");
            assert!(texto_de(&p.tab).is_empty());
        }
    }
}

#[test]
fn con_lo_mio_manda_lo_que_borre_aqui_no_vuelve_a_entrar_y_lo_que_solo_hay_alli_llega() {
    let p = montar("mando-borrado-mio");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "la mia borrada"));
    p.general();
    p.reloj.saltar(10_000);
    borrar(&p.tel, &p.reloj, "t1");
    mensaje(&p.tab, &p.reloj, nota("b1", "solo suya"));
    p.mandando(&[GENERAL]);
    let aqui = texto_de(&p.tel);
    assert!(!aqui.contains(&"la mia borrada".to_string()), "{aqui:?}");
    assert!(aqui.contains(&"solo suya".to_string()), "lo suyo si llega");
    assert!(
        !texto_de(&p.tab).contains(&"la mia borrada".to_string()),
        "mi borrado va alla"
    );
}

#[test]
fn con_lo_mio_manda_lo_cambiado_en_los_dos_no_se_junta_sino_que_se_manda() {
    let p = montar("mando-fusion");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("t1", "original"));
    p.general();
    p.reloj.saltar(10_000);
    editar(&p.tel, "t1", |m| {
        kotlin::poner(m, "texto", Json::cadena("mio"))
    });
    editar(&p.tab, "t1", |m| {
        kotlin::poner(m, "texto", Json::cadena("suyo"));
    });
    let hecho = p.mandando(&[GENERAL]);
    assert_eq!(hecho.fusionados, 0, "no se junta nada");
    assert_eq!(texto_de(&p.tel), ["mio"]);
    assert_eq!(texto_de(&p.tab), ["mio"]);
}

#[test]
fn con_lo_mio_manda_mi_archivo_no_lo_pisa_el_suyo_y_el_que_aqui_no_hay_se_trae() {
    let p = montar("mando-archivos");
    p.emparejar();
    dibujo(&p.tel, "d1", "mio v1", None);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d1"),
            ..nota("t1", "Planta")
        },
    );
    p.general();
    p.reloj.saltar(10_000);
    // Solo el otro lo toca: de siempre esto seria un «traer».
    dibujo(&p.tab, "d1", "suyo v2", None);
    dibujo(&p.tab, "d2", "solo suyo", None);
    mensaje(
        &p.tab,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d2"),
            ..nota("b1", "Alzado")
        },
    );
    p.mandando(&[GENERAL]);
    assert!(
        texto_del_dibujo(&p.tel, "d1").contains("mio v1"),
        "no me pisa"
    );
    assert!(
        texto_del_dibujo(&p.tab, "d1").contains("mio v1"),
        "el mio va"
    );
    assert!(
        texto_del_dibujo(&p.tel, "d2").contains("solo suyo"),
        "lo que aqui no existe si se trae"
    );
}

#[test]
fn de_siempre_el_archivo_que_solo_toco_el_otro_si_me_pisa() {
    // El caso negativo del de arriba: sin mando, lo tocado alli llega.
    let p = montar("mando-archivos-normal");
    p.emparejar();
    dibujo(&p.tel, "d1", "mio v1", None);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d1"),
            ..nota("t1", "Planta")
        },
    );
    p.general();
    p.reloj.saltar(10_000);
    dibujo(&p.tab, "d1", "suyo v2", None);
    p.general();
    assert!(texto_del_dibujo(&p.tel, "d1").contains("suyo v2"));
}

#[test]
fn con_lo_mio_manda_mi_proyecto_queda_entero_y_el_suyo_solo_viaja_si_aqui_no_hay() {
    let p = montar("mando-proyecto");
    p.emparejar();
    proyecto(
        &p.tel,
        json!({"id":"pr-1","nombre":"Reforma","hojas":[{"id":"h1"}],"tocado":5}),
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("pr-1"),
            ..nota("t1", "algo")
        },
    );
    p.sincronizar(&["pr-1"]);
    // El otro le anade una hoja mas tarde: de siempre se sumarian.
    let mut suyo = p.tab.leer_proyectos().pop().unwrap();
    let mut l = suyo
        .como_objeto()
        .unwrap()
        .obtener("hojas")
        .unwrap()
        .como_lista()
        .unwrap()
        .to_vec();
    l.push(Json::de_valor(&json!({"id":"h2"})));
    kotlin::poner(&mut suyo, "hojas", Json::Lista(l));
    kotlin::poner(&mut suyo, "tocado", Json::numero(99));
    p.tab.guardar_proyecto(&suyo).unwrap();
    // Y un proyecto que aqui no existe.
    proyecto(
        &p.tab,
        json!({"id":"pr-2","nombre":"Solo suyo","hojas":[{"id":"z1"}],"tocado":7}),
    );
    mensaje(
        &p.tab,
        &p.reloj,
        Nuevo {
            proyecto: Some("pr-2"),
            ..nota("b1", "algo")
        },
    );
    p.mandando(&["pr-1", "pr-2"]);
    let mio = |d: &DiscoAndroid, id: &str| -> Vec<String> {
        let pr = d
            .leer_proyectos()
            .into_iter()
            .find(|p| kotlin::cadena(p, "id") == Some(id))
            .unwrap();
        pr.como_objeto()
            .unwrap()
            .obtener("hojas")
            .unwrap()
            .como_lista()
            .unwrap()
            .iter()
            .map(|h| kotlin::cadena(h, "id").unwrap().to_string())
            .collect()
    };
    assert_eq!(mio(&p.tel, "pr-1"), ["h1"], "el mio, entero");
    assert_eq!(mio(&p.tab, "pr-1"), ["h1"], "y el suyo queda como el mio");
    assert_eq!(mio(&p.tel, "pr-2"), ["z1"], "el que aqui no hay, llega");
}

/// **Tareas al instante** (10-oct-2026): la vuelta recortada a un chat
/// junta ese chat y nada mas, con el cable de siempre: quien responde no
/// nota la diferencia.
#[test]
fn la_vuelta_de_solo_estos_chats_junta_esos_y_deja_los_demas_para_la_entera() {
    let p = montar("solo-estos");
    p.emparejar();
    proyecto(
        &p.tel,
        json!({"id":"pr-1","nombre":"Obra","hojas":[],"tocado":5}),
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("pr-1"),
            ..nota("o1", "- [ ] tarea")
        },
    );
    mensaje(&p.tel, &p.reloj, nota("g1", "en guardados"));
    let solo = |chats: &[&str]| {
        conectado(&p.tel, &p.tab, false, None, &p.reloj, |s| {
            let mut hecho = Hecho::default();
            let elegidos: BTreeSet<String> = chats.iter().map(|c| c.to_string()).collect();
            crate::vuelta::solo_estos(
                s,
                &elegidos,
                &mut hecho,
                "",
                &|| p.reloj.ahora(),
                &mut |_| {},
            )
            .unwrap();
            hecho
        })
        .unwrap()
    };
    solo(&["pr-1"]);
    assert_eq!(
        senas(&p.tab, "pr-1"),
        ["1a".to_string()].into(),
        "el chat pedido llega"
    );
    assert!(
        senas(&p.tab, GENERAL).is_empty(),
        "caso negativo: el otro no viaja"
    );
    // Lo tachado despues tambien, y otra vez sin tocar «Guardados».
    editar(&p.tel, "o1", |m| {
        kotlin::poner(m, "texto", Json::cadena("- [x] tarea"))
    });
    solo(&["pr-1"]);
    assert!(texto_de(&p.tab).contains(&"- [x] tarea".to_string()));
    assert!(senas(&p.tab, GENERAL).is_empty());
    // Un chat que no esta en ninguno de los dos no rompe nada.
    solo(&["no-existe"]);
    // La entera se lleva el resto.
    p.vuelta_del_tel(None);
    assert_eq!(senas(&p.tab, GENERAL), ["1a".to_string()].into());
}

/// Un chat desmarcado con ese aparato en «¿Que sincronizar?» no se cuela
/// por la vuelta recortada.
#[test]
fn la_vuelta_recortada_respeta_lo_desmarcado_con_ese_aparato() {
    let p = montar("solo-desmarcado");
    p.emparejar();
    mensaje(&p.tel, &p.reloj, nota("g1", "en guardados"));
    p.tel
        .guardar_elegidos("id-tab", &BTreeSet::from(["otro".to_string()]))
        .unwrap();
    conectado(&p.tel, &p.tab, false, None, &p.reloj, |s| {
        let mut hecho = Hecho::default();
        let chats = BTreeSet::from([GENERAL.to_string()]);
        crate::vuelta::solo_estos(s, &chats, &mut hecho, "", &|| p.reloj.ahora(), &mut |_| {})
            .unwrap();
    })
    .unwrap();
    assert!(senas(&p.tab, GENERAL).is_empty());
}
