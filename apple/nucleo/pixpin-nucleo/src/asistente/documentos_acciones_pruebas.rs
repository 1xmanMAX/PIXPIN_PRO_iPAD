//! Pruebas de las herramientas de documentos del asistente (E1), sin red ni
//! Swift: unas manos falsas que apuntan los pedidos.

use std::sync::{Arc, Mutex};

use super::*;
use crate::ErrorNucleo;
use crate::asistente::turno::{MedidaPagina, RangoPaginas};

#[derive(Default)]
struct Manos {
    pedidos: Mutex<Vec<Value>>,
}

impl AppIa for Manos {
    fn confirmar(&self, _pregunta: String) -> bool {
        true
    }
    fn pedido(&self, json: String) -> Option<String> {
        self.pedidos.lock().unwrap().push(serde_json::from_str(&json).unwrap());
        None
    }
    fn leer_texto(&self, _ruta: String, _paginas: Vec<RangoPaginas>) -> Result<String, ErrorNucleo> {
        Ok(String::new())
    }
    fn pintar_vista(&self, _vista: crate::asistente::ojos::VistaParaPintar) -> Option<Vec<u8>> {
        None
    }
    fn paginas_de_pdf(&self, _ruta: String) -> Vec<MedidaPagina> {
        vec![MedidaPagina { ancho: 595.0, alto: 842.0 }]
    }
    fn pintar_pagina(&self, _ruta: String, _pagina: u32, _ancho: u32) -> Option<Vec<u8>> {
        None
    }
    fn palabras_de_imagen(&self, _imagen: Vec<u8>) -> Vec<PalabraLeida> {
        vec![
            PalabraLeida { texto: "Total".into(), x: 10.0, y: 20.0, ancho: 50.0, alto: 12.0, renglon: 0 },
            PalabraLeida { texto: "120".into(), x: 80.0, y: 20.0, ancho: 30.0, alto: 12.0, renglon: 0 },
            PalabraLeida { texto: "Señalización".into(), x: 10.0, y: 40.0, ancho: 120.0, alto: 12.0, renglon: 1 },
        ]
    }
    fn exportar(&self, _ruta: String, _formato: String, _carpeta: String) -> Result<Vec<String>, ErrorNucleo> {
        Err(ErrorNucleo::Argumento { detalle: "no".into() })
    }
}

fn accion(herramienta: &'static str, pedido: Value) -> AccionApp {
    AccionApp { herramienta, pedido, confirmar: None, por_resolver: Default::default() }
}

fn temporal(etiqueta: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pixpin-docacc-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn el_ocr_se_vuelve_una_pagina_con_la_caja_de_cada_letra_y_se_busca_sin_tildes() {
    let m = Manos::default();
    let palabras = m.palabras_de_imagen(Vec::new());
    let t = pagina_de_ocr(&palabras, 200, 100);
    assert_eq!(t.texto, "Total 120 \nSeñalización \n");
    assert_eq!(t.texto.chars().count(), t.cajas.len(), "una caja por letra");
    let p = Paginas { textos: vec![t], altos: vec![ANCHO_HOJA] };
    let s = buscar_en_paginas(&p, "senalizacion", None, 5);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].pagina, 0);
    assert!(!s[0].cajas.is_empty());
    assert!(s[0].contexto.contains("Señalización"));
    // Caso negativo: lo que no esta.
    assert!(buscar_en_paginas(&p, "presupuesto", None, 5).is_empty());
    assert!(buscar_en_paginas(&p, "total", Some(3), 5).is_empty(), "en otra hoja no");
}

#[test]
fn las_figuras_salen_del_motor_comun_una_por_sitio() {
    let cajas = [[100.0, 100.0, 300.0, 120.0], [100.0, 140.0, 250.0, 160.0]];
    for como in [ComoAnotar::Resaltar, ComoAnotar::Subrayar, ComoAnotar::Tachar] {
        let f = figuras(como, &cajas, (255, 200, 0), None, &[], 14.0);
        assert_eq!(f.len(), 2, "{como:?}");
    }
    assert!(!figuras(ComoAnotar::Encerrar, &cajas[..1], (255, 0, 0), None, &[], 14.0).is_empty());
    // Caso negativo: una nota sin texto no pone nada.
    assert!(figuras(ComoAnotar::Nota, &cajas[..1], (255, 0, 0), None, &[], 14.0).is_empty());
}

#[test]
fn abrir_por_su_nombre_busca_entre_los_adjuntos_de_los_chats_y_lo_pide_a_la_app() {
    let d = temporal("abrir");
    let n = crate::Nucleo::abrir(d.join("datos").display().to_string(), "Mac".into()).unwrap();
    let obra = n.crear_chat("Obra".into()).unwrap();
    let f = d.join("Plan de señalizacion.pdf");
    std::fs::write(&f, b"%PDF-1.4 de mentira").unwrap();
    n.adjuntar(obra.id.clone(), f.display().to_string(), None).unwrap();
    let manos: Arc<Manos> = Arc::new(Manos::default());
    let cx = Contexto {
        nucleo: &n,
        proyecto: &obra.id,
        adjuntos: &[],
        app: Some(manos.as_ref()),
        parar: &|| false,
        desfase_ms: 0,
    };
    let a = accion(ABRIR, json!({"descripcion": "el plan de senalizacion"}));
    let r = hacer(&a, &cx).unwrap();
    assert!(r.contains("Plan de señalizacion.pdf"), "{r}");
    let p = manos.pedidos.lock().unwrap();
    assert_eq!(p[0]["accion"], "abrir");
    assert!(p[0]["que"]["ruta"].as_str().unwrap().ends_with(".pdf"));
    drop(p);
    // Sin documento que sirva, se dice que hay en el chat.
    let foto = d.join("recibo.txt");
    std::fs::write(&foto, b"hola").unwrap();
    n.adjuntar(obra.id.clone(), foto.display().to_string(), None).unwrap();
    let e = elegir(&cx, None, Sirve::Escaneado);
    assert!(e.is_ok(), "el PDF del chat sirve: {e:?}");
    let otro = n.crear_chat("Vacio".into()).unwrap();
    let cx2 = Contexto { proyecto: &otro.id, ..cx };
    let e = elegir(&cx2, None, Sirve::Documento).unwrap_err();
    assert!(e.contains("en este chat no hay un PDF"), "{e}");
    // Caso negativo: una herramienta que no es de documentos.
    assert!(hacer(&accion("dibujar", json!({})), &cx).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn la_rejilla_de_la_foto_manda_y_el_excel_lleva_sus_fondos_y_anchos() {
    let leida = TablaLeida { filas: vec![vec!["a".into(), "b".into()]], combinadas: Vec::new() };
    let mut m = pixpin_ia::tabla_aspecto::Medida {
        columnas: vec![0.0, 100.0, 300.0],
        filas: vec![0.0, 20.0],
        fondos: vec![vec![Some([255, 0, 0]), None]],
        tam_letra: 15.0,
        celdas: vec![vec!["Total".into(), "120".into()]],
        ..Default::default()
    };
    let foto = Path::new("/x/tabla.png");
    assert_eq!(filas_de(&leida, &m, foto), m.celdas, "en una foto, la rejilla");
    let ruta = escribir_excel_con("T", &m.celdas, false, Some(&m)).unwrap();
    assert!(std::fs::metadata(&ruta).unwrap().len() > 0);
    // Caso negativo: sin rejilla, lo leido.
    m.celdas = Vec::new();
    assert_eq!(filas_de(&leida, &m, foto), leida.filas);
    // Las rayas tenues se marcan (el papel sigue blanco, lo gris se oscurece).
    let mut px = vec![255, 255, 255, 255, 200, 200, 200, 255];
    rayas_marcadas(&mut px);
    assert_eq!(px[0], 255);
    assert!(px[4] < 120, "{}", px[4]);
}
