//! Pruebas de la revision: cada regla encuentra lo suyo, y un dibujo limpio
//! no da ningun aviso.

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::texto::AlineacionTexto;
use pixpin_motor2d::vector::Punto2;

use super::*;

fn caja(x: f32, y: f32, w: f32, h: f32) -> Elemento {
    Elemento {
        figura: Figura::Rectangulo,
        x,
        y,
        ancho: w,
        alto: h,
        ..Default::default()
    }
}

fn texto(t: &str, x: f32, y: f32) -> Elemento {
    let mut e = Elemento {
        figura: Figura::Texto {
            texto: String::new(),
            tam: 20.0,
            familia: "Segoe UI".into(),
        },
        x,
        y,
        ..Default::default()
    };
    pixpin_motor2d::texto_rico::poner_fuente(&mut e, t);
    e
}

/// Un texto centrado en una caja.
fn en(t: &str, c: &Elemento) -> Elemento {
    let mut e = texto(t, 0.0, 0.0);
    e.extras.alineacion = Some(AlineacionTexto::Centro);
    e.x = c.x + c.ancho / 2.0 - e.ancho / 2.0;
    e.y = c.y + c.alto / 2.0 - e.alto / 2.0;
    e
}

fn flecha(a: (f32, f32), b: (f32, f32)) -> Elemento {
    Elemento {
        figura: Figura::Flecha {
            puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            punta_inicio: Default::default(),
            punta_fin: Default::default(),
            codos: false,
        },
        x: a.0.min(b.0),
        y: a.1.min(b.1),
        ancho: (a.0 - b.0).abs(),
        alto: (a.1 - b.1).abs(),
        ..Default::default()
    }
}

fn avisos(s: &Escena) -> Vec<String> {
    revisar_con(s, None, &OrtografiaBasica)
        .into_iter()
        .map(|h| h.texto)
        .collect()
}

/// Dos cajas en fila con su rotulo y una flecha de una a otra.
fn limpio() -> (Escena, u64, u64, u64) {
    let mut s = Escena::nueva();
    let a = caja(0.0, 0.0, 160.0, 60.0);
    let b = caja(260.0, 0.0, 220.0, 60.0);
    let ta = en("Inicio", &a);
    let tb = en("Fin del proceso", &b);
    let ia = s.anadir(a);
    let ib = s.anadir(b);
    s.anadir(ta);
    s.anadir(tb);
    let f = s.anadir(flecha((162.0, 30.0), (258.0, 30.0)));
    s.anadir(texto("Número de página: también", 0.0, 200.0));
    (s, ia, ib, f)
}

#[test]
fn un_dibujo_limpio_no_da_avisos() {
    let (s, ..) = limpio();
    assert_eq!(avisos(&s), Vec::<String>::new());
    assert_eq!(para_el_modelo(&[]), "");
}

#[test]
fn textos_encimados_y_figuras_que_se_pisan() {
    let mut s = Escena::nueva();
    let a = s.anadir(texto("Primero", 0.0, 0.0));
    let b = s.anadir(texto("Segundo", 10.0, 4.0));
    let v = avisos(&s);
    assert!(
        v.iter()
            .any(|t| t == &format!("#{a} y #{b}: textos encimados")),
        "{v:?}"
    );
    let c = s.anadir(caja(300.0, 0.0, 100.0, 100.0));
    let d = s.anadir(caja(350.0, 50.0, 100.0, 100.0));
    assert!(
        avisos(&s)
            .iter()
            .any(|t| t.starts_with(&format!("#{c} y #{d}: figuras que se pisan")))
    );
    // Solo lo tocado: con #c tocado no sale lo de los textos.
    let solo = revisar_con(&s, Some(&[c]), &OrtografiaBasica);
    assert!(solo.iter().all(|h| h.ids.contains(&c)), "{solo:?}");
}

#[test]
fn texto_que_se_sale_de_su_caja() {
    let mut s = Escena::nueva();
    let c = caja(0.0, 0.0, 140.0, 40.0);
    let t = s.anadir(en("Un rotulo demasiado largo", &c));
    let ic = s.anadir(c);
    let v = avisos(&s);
    assert!(
        v.iter()
            .any(|x| x.starts_with(&format!("#{t} se sale de #{ic} (sobra"))),
        "{v:?}"
    );
}

#[test]
fn flecha_que_no_llega_y_marco_y_casi_alineados() {
    let (mut s, a, b, _) = limpio();
    // Una flecha que se queda a 20 px de la caja #b.
    let f = s.anadir(flecha((80.0, 62.0), (300.0, 100.0)));
    let v = avisos(&s);
    assert!(
        v.iter().any(
            |x| x == &format!("#{f}: el fin de la flecha se queda a 40 px de #{b} sin tocarlo")
        ),
        "{v:?}"
    );
    // Algo medio fuera de un marco.
    let m = s.anadir(Elemento {
        figura: Figura::Marco {
            nombre: "Hoja".into(),
        },
        x: 0.0,
        y: 300.0,
        ancho: 200.0,
        alto: 200.0,
        ..Default::default()
    });
    let fuera = s.anadir(caja(150.0, 350.0, 100.0, 50.0));
    assert!(
        avisos(&s)
            .iter()
            .any(|x| x == &format!("#{fuera} se sale a medias del marco #{m}"))
    );
    // Una tercera caja a 4 px de la fila de #a.
    let c = s.anadir(caja(520.0, 4.0, 160.0, 60.0));
    let v = avisos(&s);
    assert!(
        v.iter()
            .any(|x| x.contains(&format!("#{c}")) && x.contains("casi en la misma fila (4 px")),
        "{v:?}"
    );
    let _ = a;
}

#[test]
fn faltan_tildes_en_espanol() {
    let mut s = Escena::nueva();
    let t = s.anadir(texto("El numero de pagina y la informacion", 0.0, 0.0));
    let q = s.anadir(texto("¿Que es esto?", 0.0, 100.0));
    let e = s.anadir(texto("La tarea esta terminada", 0.0, 200.0));
    let v = avisos(&s);
    assert!(
        v.iter().any(|x| x
            == &format!(
                "#{t} ortografia: «numero»→«número», «pagina»→«página», «informacion»→«información»"
            )),
        "{v:?}"
    );
    assert!(
        v.iter()
            .any(|x| x == &format!("#{q} ortografia: «Que»→«Qué»")),
        "{v:?}"
    );
    assert!(
        v.iter()
            .any(|x| x == &format!("#{e} ortografia: «esta»→«está»")),
        "{v:?}"
    );
    // «Esta casa» y «que» sin pregunta estan bien.
    let mut s = Escena::nueva();
    s.anadir(texto("Esta casa es la que vimos", 0.0, 0.0));
    assert_eq!(avisos(&s), Vec::<String>::new());
    // En ingles no se busca nada de esto.
    let mut s = Escena::nueva();
    s.anadir(texto("The menu of the area", 0.0, 0.0));
    assert_eq!(avisos(&s), Vec::<String>::new());
}

#[test]
fn el_idioma_de_cada_texto() {
    assert_eq!(idioma_de("流程开始"), Some("zh"));
    assert_eq!(idioma_de("Inicio del proceso"), Some("es"));
    assert_eq!(idioma_de("Start of the process"), Some("en"));
    assert_eq!(idioma_de("A"), None);
    assert_eq!(idioma_de("42"), None);
}

#[test]
fn un_corrector_de_fuera_se_enchufa_por_el_rasgo() {
    struct Siempre;
    impl Ortografia for Siempre {
        fn revisar(&self, texto: &str, _: &str) -> Vec<Falta> {
            vec![Falta {
                palabra: texto.to_string(),
                arreglo: None,
            }]
        }
    }
    let mut s = Escena::nueva();
    let t = s.anadir(texto("Inicio del proceso", 0.0, 0.0));
    let v = revisar_con(&s, None, &Siempre);
    assert_eq!(v[0].texto, format!("#{t} ortografia: «Inicio del proceso»"));
}

#[test]
fn el_resumen_para_el_modelo_es_corto() {
    let muchos: Vec<Hallazgo> = (0..20)
        .map(|i| aviso(vec![i], format!("aviso {i}")))
        .collect();
    let r = para_el_modelo(&muchos);
    assert_eq!(r.lines().count(), MAX_LINEAS + 1, "{r}");
    assert!(r.ends_with("(+13 avisos mas)"), "{r}");
}
