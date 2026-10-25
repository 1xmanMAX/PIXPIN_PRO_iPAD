//! Las pruebas de la galeria que viaja: las reglas, una a una, como
//! `GaleriaCompartidaTest.kt` (10), y dos «aparatos» que se hablan por un
//! socket con el protocolo y el cifrado de verdad, como
//! `GaleriaQueViajaTest.kt` (6).

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::*;
use crate::disco_android::DiscoAndroid;
use crate::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar};
use crate::protocolo::{Hecho, Respondedor, Sesion};

const T: i64 = 1_790_000_000_000;

fn r0() -> Caducidad {
    Caducidad::default()
}

fn local(nombre: &str, cuando: i64, bytes: i64) -> Local {
    Local {
        nombre: nombre.into(),
        cuando,
        bytes,
        mime: "image/png".into(),
        texto: String::new(),
    }
}

fn entrada(nombre: &str, cuando: i64) -> Entrada {
    Entrada::nueva(nombre, cuando)
}

fn nombres(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ------------------------------------------------------------ las reglas

#[test]
fn una_captura_nueva_entra_con_su_fecha_de_irse() {
    let e = al_dia(
        &Estado::default(),
        Some(&[local("a.png", T, 10)]),
        &r0(),
        7,
        T,
        "tel",
    );
    let a = e.por_nombre()["a.png"].clone();
    assert_eq!(a.se_va, Some(T + 7 * DIA_MS));
    assert!(!a.conservada);
    assert_eq!(a.de, "tel");
    assert_eq!(e.tenia, nombres(&["a.png"]));
}

#[test]
fn sin_poder_listar_no_se_toca_nada() {
    let antes = Estado {
        entradas: vec![Entrada {
            se_va: Some(T + DIA_MS),
            ..entrada("a.png", T)
        }],
        tenia: nombres(&["a.png"]),
    };
    assert_eq!(al_dia(&antes, None, &r0(), 7, T, "tel"), antes);
}

#[test]
fn la_que_falta_se_marca_borrada_solo_si_estaba_aqui_y_no_le_tocaba_irse() {
    let viva = Entrada {
        se_va: Some(T + 5 * DIA_MS),
        ..entrada("viva.png", T)
    };
    let caducada = Entrada {
        se_va: Some(T - 2 * DIA_MS),
        ..entrada("vieja.png", T - 9 * DIA_MS)
    };
    // Nunca estuvo aqui.
    let ajena = Entrada {
        se_va: Some(T + 5 * DIA_MS),
        ..entrada("ajena.png", T)
    };
    let conservada = Entrada {
        conservada: true,
        ..entrada("guardada.png", T)
    };
    let e = al_dia(
        &Estado {
            entradas: vec![viva, caducada, ajena, conservada],
            tenia: nombres(&["viva.png", "vieja.png", "guardada.png"]),
        },
        Some(&[]),
        &r0(),
        7,
        T,
        "tel",
    );
    let p = e.por_nombre();
    assert!(p["viva.png"].borrada);
    assert_eq!(p["viva.png"].cambiado, T);
    assert!(!p["vieja.png"].borrada);
    assert!(!p["ajena.png"].borrada);
    assert!(
        p["guardada.png"].borrada,
        "quitar una conservada tambien es quitarla"
    );
    assert!(e.tenia.is_empty());
}

#[test]
fn conservar_y_prorrogar_aqui_cambian_la_entrada_con_la_hora_de_ahora() {
    let a = Entrada {
        se_va: Some(T + 7 * DIA_MS),
        cambiado: T,
        ..entrada("a.png", T)
    };
    let b = Entrada {
        se_va: Some(T + 7 * DIA_MS),
        cambiado: T,
        ..entrada("b.png", T)
    };
    let mut r = r0();
    r.conservadas.insert("a.png".into());
    r.fijadas.insert("b.png".into(), T + 14 * DIA_MS);
    let l = [local("a.png", T, 1), local("b.png", T, 1)];
    let e = al_dia(
        &Estado {
            entradas: vec![a, b],
            tenia: nombres(&["a.png", "b.png"]),
        },
        Some(&l),
        &r,
        7,
        T + 99,
        "tel",
    );
    let p = e.por_nombre();
    assert!(p["a.png"].conservada);
    assert_eq!(p["a.png"].se_va, None);
    assert_eq!(p["a.png"].cambiado, T + 99);
    assert_eq!(p["b.png"].se_va, Some(T + 14 * DIA_MS));
    assert_eq!(p["b.png"].cambiado, T + 99);
}

#[test]
fn sin_caducidad_aqui_no_se_cambia_la_fecha_de_los_demas() {
    let a = Entrada {
        se_va: Some(T + 7 * DIA_MS),
        cambiado: T,
        ..entrada("a.png", T)
    };
    let e = al_dia(
        &Estado {
            entradas: vec![a.clone()],
            tenia: nombres(&["a.png"]),
        },
        Some(&[local("a.png", T, 1)]),
        &r0(),
        0,
        T + 5,
        "tel",
    );
    assert_eq!(e.por_nombre()["a.png"], &a);
}

#[test]
fn juntar_se_queda_con_lo_mas_reciente_y_da_lo_mismo_en_los_dos_sentidos() {
    let vieja = Entrada {
        se_va: Some(T + DIA_MS),
        cambiado: 10,
        ..entrada("a.png", T)
    };
    let nueva = Entrada {
        se_va: Some(T + 9 * DIA_MS),
        cambiado: 20,
        ..entrada("a.png", T)
    };
    let borrada = Entrada {
        se_va: Some(T + DIA_MS),
        borrada: true,
        cambiado: 20,
        ..entrada("a.png", T)
    };
    let sola = Entrada {
        cambiado: 1,
        ..entrada("b.png", T)
    };
    assert_eq!(
        juntar(std::slice::from_ref(&vieja), &[nueva.clone(), sola.clone()]),
        vec![nueva.clone(), sola.clone()]
    );
    assert_eq!(
        juntar(std::slice::from_ref(&nueva), &[vieja.clone(), sola.clone()]),
        juntar(&[vieja.clone(), sola.clone()], std::slice::from_ref(&nueva))
    );
    // A la misma hora gana la borrada, venga de donde venga.
    assert!(juntar(std::slice::from_ref(&nueva), std::slice::from_ref(&borrada))[0].borrada);
    assert!(juntar(std::slice::from_ref(&borrada), std::slice::from_ref(&nueva))[0].borrada);
}

#[test]
fn aplicar_pone_conservadas_y_fechas_y_olvida_las_borradas() {
    let mut r = r0();
    r.conservadas.insert("x.png".into());
    r.prorrogadas.insert("x.png".into(), 5);
    r.fijadas.insert("x.png".into(), 6);
    let n = aplicar(
        &r,
        &[
            Entrada {
                borrada: true,
                ..entrada("x.png", T)
            },
            Entrada {
                conservada: true,
                ..entrada("c.png", T)
            },
            Entrada {
                se_va: Some(77),
                ..entrada("f.png", T)
            },
        ],
    );
    assert_eq!(n.conservadas, nombres(&["c.png"]));
    assert!(n.prorrogadas.is_empty());
    assert_eq!(n.fijadas, [("f.png".to_string(), 77)].into_iter().collect());
    // Y la fecha acordada manda sobre la regla.
    assert_eq!(se_va_el(&n, "f.png", T, 7), Some(77));
}

#[test]
fn que_traer_y_que_mandar_no_cuentan_borradas_ni_caducadas() {
    let l = vec![
        Entrada {
            se_va: Some(T + DIA_MS),
            ..entrada("viva.png", T)
        },
        Entrada {
            borrada: true,
            ..entrada("borrada.png", T)
        },
        Entrada {
            se_va: Some(T - DIA_MS),
            ..entrada("caducada.png", T - 9 * DIA_MS)
        },
        Entrada {
            conservada: true,
            ..entrada("guardada.png", T - 99 * DIA_MS)
        },
    ];
    let todas = nombres(&["viva.png", "borrada.png", "caducada.png", "guardada.png"]);
    let vacio = BTreeSet::new();
    let n = |v: Vec<Entrada>| v.into_iter().map(|e| e.nombre).collect::<Vec<_>>();
    assert_eq!(
        n(que_traer(&l, &vacio, &todas, T)),
        ["viva.png", "guardada.png"]
    );
    assert_eq!(
        n(que_mandar(&l, &todas, &vacio, T)),
        ["viva.png", "guardada.png"]
    );
    assert!(que_traer(&l, &todas, &todas, T).is_empty());
    assert_eq!(a_tirar(&l, &todas), ["borrada.png"]);
}

#[test]
fn el_registro_con_fijadas_se_escribe_y_se_lee_y_uno_sin_ellas_tambien() {
    let mut r = r0();
    r.fijadas.insert("a.png".into(), 5);
    let texto = serde_json::to_string(&r).unwrap();
    assert_eq!(serde_json::from_str::<Caducidad>(&texto).unwrap(), r);
    // Caso negativo: sin fijadas, el fichero queda como antes.
    assert!(!serde_json::to_string(&r0()).unwrap().contains("fijadas"));
    let viejo: Caducidad = serde_json::from_str(r#"{"desde":1,"conservadas":[]}"#).unwrap();
    assert!(viejo.fijadas.is_empty());
    assert_eq!(viejo.desde, 1);
}

#[test]
fn prorrogar_una_con_fecha_acordada_mueve_esa() {
    let mut r = r0();
    r.fijadas.insert("a.png".into(), T + DIA_MS);
    prorrogar(&mut r, "a.png", T - 6 * DIA_MS, T, 7);
    assert_eq!(r.fijadas.get("a.png"), Some(&(T + 8 * DIA_MS)));
    assert!(r.prorrogadas.is_empty());
    // Sin fecha acordada va a las prorrogas, como siempre.
    let mut s = r0();
    prorrogar(&mut s, "b.png", T, T, 7);
    assert_eq!(s.prorrogadas.get("b.png"), Some(&(T + 14 * DIA_MS)));
    assert!(s.fijadas.is_empty());
}

#[test]
fn una_entrada_se_escribe_como_en_android_sin_los_campos_por_omision() {
    // `encodeDefaults = false` alli: lo que vale lo de siempre no viaja.
    let e = Entrada {
        se_va: Some(5),
        ..entrada("a.png", 1)
    };
    assert_eq!(
        entradas_a_texto(&[e]),
        r#"[{"nombre":"a.png","cuando":1,"seVa":5}]"#
    );
    // Y lo que escribe Android, con todo, se lee.
    let de_android = r#"[{"nombre":"PixPin_20261008_101500.png","cuando":1791000000000,"bytes":48213,"mime":"image/png","seVa":1791604800000,"cambiado":1791000000000,"de":"id-del-aparato","cosaNueva":1}]"#;
    let l = entradas_de_texto(de_android).unwrap();
    assert_eq!(l[0].bytes, 48213);
    assert_eq!(l[0].se_va, Some(1_791_604_800_000));
    assert_eq!(l[0].de, "id-del-aparato");
    let estado: Estado =
        serde_json::from_str(r#"{"entradas":[{"nombre":"a.png","cuando":1}],"tenia":["a.png"]}"#)
            .unwrap();
    assert_eq!(estado.tenia, nombres(&["a.png"]));
    assert_eq!(serde_json::to_string(&Estado::default()).unwrap(), "{}");
}

#[test]
fn un_nombre_con_ruta_no_se_acepta() {
    for malo in [
        "../fuera.png",
        "a/b.png",
        "",
        "..",
        "x\\y.png",
        " ",
        "c:a.png",
    ] {
        assert!(nombre_valido(Some(malo)).is_err(), "{malo}");
    }
    assert!(nombre_valido(None).is_err());
    assert_eq!(
        nombre_valido(Some("captura 1.png")).unwrap(),
        "captura 1.png"
    );
}

// ------------------------------------------------------------ dos aparatos

/// Las capturas de un aparato en una carpeta; la papelera, otra. La hora de
/// cada una, su fecha de modificacion. El registro de caducidad, un JSON en
/// la raiz, como el del PC.
struct EnCarpeta {
    raiz: PathBuf,
    carpeta: PathBuf,
    papelera: PathBuf,
    dias: i64,
}

impl EnCarpeta {
    fn nueva(raiz: &Path) -> EnCarpeta {
        let carpeta = raiz.join("Pictures/PixPin");
        let papelera = raiz.join("papelera");
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::create_dir_all(&papelera).unwrap();
        EnCarpeta {
            raiz: raiz.to_path_buf(),
            carpeta,
            papelera,
            dias: 7,
        }
    }

    fn hacer(&self, nombre: &str, cuando: i64, bytes: usize) {
        let ruta = self.carpeta.join(nombre);
        let datos: Vec<u8> = (0..bytes).map(|i| (i * 7 + nombre.len()) as u8).collect();
        std::fs::write(&ruta, datos).unwrap();
        poner_hora(&ruta, cuando);
    }

    fn nombres(&self) -> Vec<String> {
        listado(&self.carpeta)
    }

    /// Donde guarda este «aparato» el texto leido de cada captura (en el
    /// PC, la cache del OCR de la galeria).
    fn ruta_del_texto(&self, nombre: &str) -> PathBuf {
        self.raiz.join("textos").join(format!("{nombre}.txt"))
    }

    /// El texto leido de una captura, como si la galeria la hubiera leido.
    fn leer_texto(&self, nombre: &str, texto: &str) {
        let r = self.ruta_del_texto(nombre);
        std::fs::create_dir_all(r.parent().unwrap()).unwrap();
        std::fs::write(r, texto).unwrap();
    }

    fn texto(&self, nombre: &str) -> String {
        std::fs::read_to_string(self.ruta_del_texto(nombre)).unwrap_or_default()
    }

    fn se_va(&self, nombre: &str, ahora: i64) -> Option<i64> {
        let cuando = hora_de(&self.carpeta.join(nombre));
        se_va_el(&self.caducidad(ahora), nombre, cuando, self.dias)
    }

    fn ruta_del_registro(&self) -> PathBuf {
        self.raiz.join("capturas-caducidad.json")
    }
}

fn listado(carpeta: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(carpeta)
        .map(|l| {
            l.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn poner_hora(ruta: &Path, ms: i64) {
    std::fs::File::options()
        .write(true)
        .open(ruta)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_millis(ms as u64))
        .unwrap();
}

fn hora_de(ruta: &Path) -> i64 {
    std::fs::metadata(ruta)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

impl CapturasDelAparato for EnCarpeta {
    fn raiz(&self) -> PathBuf {
        self.raiz.clone()
    }

    fn listar(&self) -> Option<Vec<Local>> {
        Some(
            listado(&self.carpeta)
                .into_iter()
                .map(|n| {
                    let ruta = self.carpeta.join(&n);
                    Local {
                        cuando: hora_de(&ruta),
                        bytes: std::fs::metadata(&ruta).unwrap().len() as i64,
                        mime: if n.ends_with(".mp4") {
                            "video/mp4".into()
                        } else {
                            "image/png".into()
                        },
                        texto: self.texto(&n),
                        nombre: n,
                    }
                })
                .collect(),
        )
    }

    fn abrir(&self, nombre: &str) -> Option<PathBuf> {
        Some(self.carpeta.join(nombre)).filter(|r| r.is_file())
    }

    fn guardar(
        &self,
        e: &Entrada,
        escribir: &mut dyn FnMut(&mut dyn Write) -> Resultado<bool>,
    ) -> Resultado<bool> {
        let ruta = self.carpeta.join(&e.nombre);
        let mut f = std::fs::File::create(&ruta)?;
        let bien = escribir(&mut f)?;
        drop(f);
        if !bien {
            let _ = std::fs::remove_file(&ruta);
            return Ok(false);
        }
        poner_hora(&ruta, e.cuando);
        if !e.texto.is_empty() {
            self.leer_texto(&e.nombre, &e.texto);
        }
        Ok(true)
    }

    fn apuntar_texto(&self, nombre: &str, texto: &str) {
        self.leer_texto(nombre, texto);
    }

    fn tirar(&self, nombres: &[String]) {
        for n in nombres {
            let _ = std::fs::rename(self.carpeta.join(n), self.papelera.join(n));
        }
    }

    fn dias(&self) -> i64 {
        self.dias
    }

    fn caducidad(&self, ahora: i64) -> Caducidad {
        if let Ok(t) = std::fs::read_to_string(self.ruta_del_registro())
            && let Ok(r) = serde_json::from_str(&t)
        {
            return r;
        }
        let r = Caducidad {
            desde: ahora,
            ..Default::default()
        };
        std::fs::write(self.ruta_del_registro(), serde_json::to_vec(&r).unwrap()).unwrap();
        r
    }

    fn cambiar_caducidad(&self, ahora: i64, f: &mut dyn FnMut(&mut Caducidad)) -> io::Result<()> {
        let mut r = self.caducidad(ahora);
        f(&mut r);
        std::fs::write(self.ruta_del_registro(), serde_json::to_vec(&r)?)
    }
}

/// Un lunes cualquiera, a mediodia (las horas de los ficheros van en
/// segundos enteros).
const T0: i64 = 1_790_000_000_000;

struct Par {
    dir: PathBuf,
    tel: DiscoAndroid,
    tab: DiscoAndroid,
    gal_tel: EnCarpeta,
    gal_tab: EnCarpeta,
    reloj: Reloj,
}

impl Drop for Par {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Par {
    let dir =
        std::env::temp_dir().join(format!("pixpin-galeria-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let tel = DiscoAndroid::nuevo(dir.join("telefono/files"), "tel");
    let tab = DiscoAndroid::nuevo(dir.join("tableta/files"), "tab");
    presentar(&tel, "id-tel", "Teléfono");
    presentar(&tab, "id-tab", "Tableta");
    let gal_tel = EnCarpeta::nueva(&dir.join("telefono/files"));
    let gal_tab = EnCarpeta::nueva(&dir.join("tableta/files"));
    // El registro de cada uno empieza antes que todo: nada de «lo que ya
    // habia no se va».
    gal_tel.caducidad(T0 - 30 * DIA_MS);
    gal_tab.caducidad(T0 - 30 * DIA_MS);
    let reloj = Reloj::nuevo();
    reloj.saltar(T0 - reloj.ahora());
    crear_grupo(&tel, "ABCDE23456", reloj.ahora());
    conectado(&tab, &tel, true, Some("ABCDE23456"), &reloj, |_| {}).unwrap();
    Par {
        dir,
        tel,
        tab,
        gal_tel,
        gal_tab,
        reloj,
    }
}

/// Uno responde en otro hilo, con su galeria o sin ella, y el otro dirige
/// aqui con la suya.
fn con_galeria<T>(
    desde: &DiscoAndroid,
    gal_desde: &EnCarpeta,
    hacia: &DiscoAndroid,
    gal_hacia: Option<&EnCarpeta>,
    reloj: &Reloj,
    uso: impl FnOnce(&mut Sesion<'_, DiscoAndroid, TcpStream>) -> T,
) -> T {
    let escucha = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let puerto = escucha.local_addr().unwrap().port();

    std::thread::scope(|hilos| {
        let respondio = hilos.spawn(|| {
            let (flujo, _) = escucha.accept().unwrap();
            let r = Respondedor {
                disco: hacia,
                estado: &|_| {},
                ahora: &|| reloj.ahora(),
                mi_puerto: 0,
                al_saludar: &|_, _| {},
                suelto: None,
            };
            let galeria = gal_hacia.map(|c| c as &dyn CapturasDelAparato);
            r.atender_con_galeria(flujo, [7u8; 32], galeria)
                .map_err(|e| e.to_string())
        });
        let flujo = TcpStream::connect(("127.0.0.1", puerto)).unwrap();
        let mut s =
            Sesion::conectar(flujo, desde, false, None, || reloj.ahora(), 0, [3u8; 32]).unwrap();
        s.con_galeria(gal_desde);
        let r = uso(&mut s);
        s.adios();
        drop(s);
        respondio.join().unwrap().expect("el que responde no falla");
        r
    })
}

impl Par {
    /// Una vuelta solo de galeria, con el telefono dirigiendo o la tableta.
    fn sincronizar(&self, desde_tel: bool) -> Hecho {
        let (d, gd, h, gh) = if desde_tel {
            (&self.tel, &self.gal_tel, &self.tab, &self.gal_tab)
        } else {
            (&self.tab, &self.gal_tab, &self.tel, &self.gal_tel)
        };
        con_galeria(d, gd, h, Some(gh), &self.reloj, |s| {
            let mut hecho = Hecho::default();
            assert!(
                s.galeria(&mut hecho, self.reloj.ahora(), &mut |_| {})
                    .unwrap()
            );
            hecho
        })
    }

    fn poner_reloj(&self, ms: i64) {
        self.reloj.saltar(ms - self.reloj.ahora());
    }
}

#[test]
fn las_capturas_de_cada_uno_pasan_al_otro_y_se_van_a_la_vez_en_los_dos() {
    let p = montar("pasan");
    // Mas de un trozo.
    p.gal_tel
        .hacer("captura-tel.png", T0 - 2 * DIA_MS, 1_300_000);
    p.gal_tab.hacer("captura-tab.png", T0 - DIA_MS, 3000);
    p.poner_reloj(T0);
    let h = p.sincronizar(true);
    assert_eq!(h.capturas, 2);
    assert_eq!(p.gal_tel.nombres(), ["captura-tab.png", "captura-tel.png"]);
    assert_eq!(p.gal_tel.nombres(), p.gal_tab.nombres());
    assert_eq!(
        std::fs::read(p.gal_tel.carpeta.join("captura-tel.png")).unwrap(),
        std::fs::read(p.gal_tab.carpeta.join("captura-tel.png")).unwrap()
    );
    // Llego con su hora de verdad, y se va el mismo dia en los dos: a los 7
    // dias de hacerse.
    let ahora = p.reloj.ahora();
    for n in p.gal_tel.nombres() {
        assert_eq!(
            p.gal_tel.se_va(&n, ahora),
            p.gal_tab.se_va(&n, ahora),
            "{n}"
        );
    }
    assert_eq!(
        hora_de(&p.gal_tab.carpeta.join("captura-tel.png")),
        T0 - 2 * DIA_MS
    );
    assert_eq!(
        p.gal_tab.se_va("captura-tel.png", ahora),
        Some(T0 - 2 * DIA_MS + 7 * DIA_MS)
    );
    // Otra vuelta: ya estan iguales, no pasa nada.
    assert_eq!(p.sincronizar(false).capturas, 0);
}

#[test]
fn siete_dias_mas_y_conservar_en_uno_se_ven_en_el_otro() {
    let p = montar("prorroga");
    p.gal_tel.hacer("a.png", T0 - DIA_MS, 3000);
    p.gal_tel.hacer("b.png", T0 - DIA_MS, 3000);
    p.sincronizar(true);
    // En la tableta: «7 dias mas» a una, como hace la galeria.
    p.poner_reloj(T0 + 1000);
    let ahora = p.reloj.ahora();
    let cuando_a = hora_de(&p.gal_tab.carpeta.join("a.png"));
    p.gal_tab
        .cambiar_caducidad(ahora, &mut |r| prorrogar(r, "a.png", cuando_a, ahora, 7))
        .unwrap();
    // Y en el telefono se conserva la otra.
    p.gal_tel
        .cambiar_caducidad(ahora, &mut |r| {
            r.conservadas.insert("b.png".into());
        })
        .unwrap();
    p.poner_reloj(T0 + 2000);
    p.sincronizar(true);
    let ahora = p.reloj.ahora();
    assert_eq!(
        p.gal_tel.se_va("a.png", ahora),
        Some(T0 - DIA_MS + 14 * DIA_MS)
    );
    assert_eq!(
        p.gal_tab.se_va("a.png", ahora),
        p.gal_tel.se_va("a.png", ahora)
    );
    assert_eq!(p.gal_tab.se_va("b.png", ahora), None);
    assert!(p.gal_tab.caducidad(ahora).conservadas.contains("b.png"));
}

#[test]
fn quitar_una_a_mano_antes_de_tiempo_la_quita_del_otro_a_su_papelera() {
    let p = montar("quitar");
    p.gal_tel.hacer("a.png", T0 - DIA_MS, 3000);
    p.gal_tel.hacer("b.png", T0 - DIA_MS, 3000);
    p.sincronizar(true);
    // Borrada en la tableta.
    std::fs::remove_file(p.gal_tab.carpeta.join("a.png")).unwrap();
    p.poner_reloj(T0 + 5000);
    let h = p.sincronizar(false);
    assert_eq!(p.gal_tel.nombres(), ["b.png"]);
    assert_eq!(listado(&p.gal_tel.papelera), ["a.png"]);
    assert_eq!(h.capturas, 0, "la quitada no vuelve a pasar");
    // Se tiro alli, en el que responde: aqui no se cuenta.
    assert_eq!(h.capturas_tiradas, 0);
    // Y no vuelve a la tableta en la siguiente vuelta.
    p.sincronizar(true);
    assert_eq!(p.gal_tab.nombres(), ["b.png"]);
}

#[test]
fn lo_caducado_no_viaja_ni_deja_marca_de_borrado() {
    let p = montar("caducado");
    p.gal_tel.hacer("vieja.png", T0 - 10 * DIA_MS, 3000);
    p.gal_tel.hacer("nueva.png", T0 - DIA_MS, 3000);
    p.poner_reloj(T0);
    p.sincronizar(true);
    assert_eq!(p.gal_tab.nombres(), ["nueva.png"]);
    // El telefono barre la vieja (caduco): eso no es «quitarla a mano».
    std::fs::rename(
        p.gal_tel.carpeta.join("vieja.png"),
        p.gal_tel.papelera.join("vieja.png"),
    )
    .unwrap();
    p.sincronizar(true);
    let e = leer(&p.gal_tab.raiz)
        .entradas
        .into_iter()
        .find(|e| e.nombre == "vieja.png")
        .unwrap();
    assert!(!e.borrada);
}

#[test]
fn un_aparato_sin_galeria_contesta_como_uno_de_antes_y_la_vuelta_sigue() {
    let p = montar("sin");
    p.gal_tel.hacer("a.png", T0, 3000);
    con_galeria(&p.tel, &p.gal_tel, &p.tab, None, &p.reloj, |s| {
        assert!(!s.galeria(&mut Hecho::default(), T0, &mut |_| {}).unwrap());
        // La conversacion sigue sana: se puede pedir otra cosa.
        s.lapidas().unwrap();
    });
    assert!(p.gal_tab.nombres().is_empty());
}

#[test]
fn la_vuelta_entera_pasa_la_galeria_detras_de_los_chats_y_lo_cuenta() {
    let p = montar("vuelta");
    p.gal_tab.hacer("de-la-tableta.png", T0 - DIA_MS, 3000);
    p.poner_reloj(T0);
    let mut hecho = Hecho::default();
    con_galeria(
        &p.tel,
        &p.gal_tel,
        &p.tab,
        Some(&p.gal_tab),
        &p.reloj,
        |s| {
            crate::vuelta::una(s, None, &mut hecho, "", &|| p.reloj.ahora(), &mut |_| {}).unwrap();
        },
    );
    assert_eq!(hecho.capturas, 1);
    assert_eq!(p.gal_tel.nombres(), ["de-la-tableta.png"]);
    assert!(
        crate::vuelta::contar_lo_hecho(&hecho, 0, 0.0).contains("1 captura de la galería"),
        "{}",
        crate::vuelta::contar_lo_hecho(&hecho, 0, 0.0)
    );
}

#[test]
fn un_nombre_con_ruta_que_llega_se_rechaza_y_la_conversacion_sigue() {
    // Un aparato que manda `poncaptura` con una ruta: se lee lo que trae,
    // se contesta error y la carpeta queda como estaba.
    let p = montar("ruta");
    let fuera = p.dir.join("fuera.png");
    p.gal_tel.hacer("x.png", T0, 10);
    con_galeria(
        &p.tel,
        &p.gal_tel,
        &p.tab,
        Some(&p.gal_tab),
        &p.reloj,
        |s| {
            let mut h = Hecho::default();
            let r =
                s.poncaptura_de_prueba("../../fuera.png", &p.gal_tel.carpeta.join("x.png"), &mut h);
            assert!(r.is_err(), "se rechaza");
            s.lapidas().unwrap();
        },
    );
    assert!(!fuera.exists());
    assert!(p.gal_tab.nombres().is_empty());
}

// ------------------------------------------------------------ el texto

#[test]
fn el_texto_viaja_en_la_entrada_y_sin_texto_no_se_escribe() {
    let e = Entrada {
        texto: "Factura 123\nTotal 45,60".into(),
        ..entrada("a.png", 1)
    };
    let t = entradas_a_texto(std::slice::from_ref(&e));
    assert_eq!(
        t,
        r#"[{"nombre":"a.png","cuando":1,"texto":"Factura 123\nTotal 45,60"}]"#
    );
    assert_eq!(entradas_de_texto(&t).unwrap(), vec![e]);
    // Lo de Android (sin texto) se lee con el texto vacio.
    let l = entradas_de_texto(r#"[{"nombre":"a.png","cuando":1}]"#).unwrap();
    assert!(l[0].texto.is_empty());
}

#[test]
fn al_juntar_el_texto_no_se_pierde_aunque_gane_la_entrada_sin_el() {
    let con = Entrada {
        texto: "hola".into(),
        cambiado: 1,
        ..entrada("a.png", T)
    };
    // La de Android, cambiada mas tarde (conservada alli), no trae texto.
    let sin = Entrada {
        conservada: true,
        cambiado: 2,
        ..entrada("a.png", T)
    };
    for j in [
        juntar(std::slice::from_ref(&con), std::slice::from_ref(&sin)),
        juntar(std::slice::from_ref(&sin), std::slice::from_ref(&con)),
    ] {
        assert!(j[0].conservada, "gana la mas nueva");
        assert_eq!(j[0].texto, "hola", "con el texto de la otra");
    }
}

#[test]
fn al_dia_apunta_el_texto_leido_despues_sin_contarlo_como_cambio() {
    let antes = al_dia(
        &Estado::default(),
        Some(&[local("a.png", T, 10)]),
        &r0(),
        7,
        T,
        "pc",
    );
    assert!(antes.entradas[0].texto.is_empty());
    let leida = Local {
        texto: "dentro".into(),
        ..local("a.png", T, 10)
    };
    let despues = al_dia(&antes, Some(&[leida]), &r0(), 7, T + 5, "pc");
    assert_eq!(despues.entradas[0].texto, "dentro");
    assert_eq!(despues.entradas[0].cambiado, antes.entradas[0].cambiado);
}

#[test]
fn una_captura_y_su_texto_van_y_vuelven_entre_dos_aparatos() {
    let p = montar("texto");
    p.gal_tel.hacer("captura-tel.png", T0 - DIA_MS, 3000);
    p.gal_tel
        .leer_texto("captura-tel.png", "Reunion a las 10:30");
    p.gal_tab.hacer("captura-tab.png", T0 - DIA_MS, 2000);
    p.gal_tab
        .leer_texto("captura-tab.png", "Lista de la compra");
    p.poner_reloj(T0);
    assert_eq!(p.sincronizar(true).capturas, 2);
    // Cada captura llega con su texto, donde lo busca la galeria del otro.
    assert_eq!(p.gal_tab.texto("captura-tel.png"), "Reunion a las 10:30");
    assert_eq!(p.gal_tel.texto("captura-tab.png"), "Lista de la compra");
    // Y en el estado compartido de los dos.
    for raiz in [&p.gal_tel.raiz, &p.gal_tab.raiz] {
        let e = leer(raiz);
        let pn = e.por_nombre();
        assert_eq!(pn["captura-tel.png"].texto, "Reunion a las 10:30");
        assert_eq!(pn["captura-tab.png"].texto, "Lista de la compra");
    }
}

#[test]
fn el_texto_leido_despues_llega_a_una_captura_que_el_otro_ya_tenia() {
    let p = montar("texto-tarde");
    p.gal_tel.hacer("a.png", T0 - DIA_MS, 3000);
    p.poner_reloj(T0);
    p.sincronizar(true);
    assert_eq!(p.gal_tab.nombres(), ["a.png"]);
    assert_eq!(p.gal_tab.texto("a.png"), "");
    // El telefono la lee despues; dirige la tableta: ninguna captura pasa,
    // pero el texto si.
    p.gal_tel.leer_texto("a.png", "leida luego");
    p.poner_reloj(T0 + 1000);
    assert_eq!(p.sincronizar(false).capturas, 0);
    assert_eq!(p.gal_tab.texto("a.png"), "leida luego");
}

#[test]
fn caso_negativo_lo_juntado_sin_textos_no_borra_los_de_aqui() {
    // Quien dirige es un Android, que no sabe de textos: su «galeriajunta»
    // llega sin ellos y aqui no se puede olvidar ninguno.
    let p = montar("junta-android");
    p.gal_tab.hacer("a.png", T0 - DIA_MS, 3000);
    p.gal_tab.leer_texto("a.png", "de aqui");
    p.poner_reloj(T0);
    let mia = poner_al_dia(&p.gal_tab, "id-tab", T0).unwrap();
    assert_eq!(mia.entradas[0].texto, "de aqui");
    let de_android = entradas_de_texto(&entradas_a_texto(&mia.entradas)).unwrap();
    let de_android: Vec<Entrada> = de_android
        .into_iter()
        .map(|e| Entrada {
            texto: String::new(),
            conservada: true,
            cambiado: T0 + 1,
            ..e
        })
        .collect();
    aplicar_juntas(&p.gal_tab, &de_android, T0 + 1).unwrap();
    let e = leer(&p.gal_tab.raiz);
    assert!(e.entradas[0].conservada, "lo de Android se aplica");
    assert_eq!(e.entradas[0].texto, "de aqui", "y el texto se queda");
    assert_eq!(p.gal_tab.texto("a.png"), "de aqui");
}
