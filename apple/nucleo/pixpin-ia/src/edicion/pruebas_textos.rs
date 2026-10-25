//! Pruebas de `editar_texto`: traducir una figura entera en una llamada,
//! que lo centrado siga centrado, que se diga lo que ya no cabe, y los
//! errores que dicen como arreglarlo.

use serde_json::{Value, json};

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::texto::AlineacionTexto;

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

/// Un texto suelto medido como lo mide el motor, con su alineacion.
fn texto(t: &str, x: f32, y: f32, tam: f32, al: Option<AlineacionTexto>) -> Elemento {
    let mut e = Elemento {
        figura: Figura::Texto {
            texto: String::new(),
            tam,
            familia: "Segoe UI".into(),
        },
        x,
        y,
        ..Default::default()
    };
    pixpin_motor2d::texto_rico::poner_fuente(&mut e, t);
    e.extras.alineacion = al;
    e
}

/// Un texto centrado encima de una caja (como los pone `dibujar_en_lienzo`).
fn centrado_en(t: &str, c: &Elemento, tam: f32) -> Elemento {
    let mut e = texto(t, 0.0, 0.0, tam, Some(AlineacionTexto::Centro));
    e.x = c.x + c.ancho / 2.0 - e.ancho / 2.0;
    e.y = c.y + c.alto / 2.0 - e.alto / 2.0;
    e
}

fn llamar(escena: &mut Escena, args: Value) -> Result<Hecho, String> {
    let e = validar(fila(EDITAR_TEXTO).unwrap(), args.as_object().unwrap())
        .map_err(|e| e.to_string())?;
    aplicar(escena, &e, &Contexto::default(), &mut SinImagenes)
}

fn dice(escena: &Escena, id: u64) -> String {
    match &escena.buscar(id).unwrap().figura {
        Figura::Texto { texto, .. } => texto.clone(),
        _ => panic!("no es texto"),
    }
}

fn centro_x(escena: &Escena, id: u64) -> f32 {
    let (x0, _, x1, _) = escena.buscar(id).unwrap().caja();
    (x0 + x1) / 2.0
}

/// Una caja con su rotulo atado de verdad (`containerId` y `boundElements`).
fn caja_con_rotulo(s: &mut Escena, t: &str, x: f32, y: f32, w: f32, h: f32) -> (u64, u64) {
    let mut c = caja(x, y, w, h);
    c.extras.id_de_fichero = Some(format!("caja-{x}-{y}"));
    let mut r = texto(t, 0.0, 0.0, 20.0, Some(AlineacionTexto::Centro));
    r.extras.id_de_fichero = Some(format!("rotulo-{x}-{y}"));
    let (ic, it) = (
        c.extras.id_de_fichero.clone().unwrap(),
        r.extras.id_de_fichero.clone().unwrap(),
    );
    pixpin_motor2d::texto_en_figuras::atar(&mut c, &ic, &mut r, &it);
    let p = pixpin_motor2d::texto_en_figuras::sitio_alineado_dentro(
        &c,
        (r.ancho, r.alto),
        AlineacionTexto::Centro,
        Default::default(),
    );
    r.x = p.x;
    r.y = p.y;
    (s.anadir(c), s.anadir(r))
}

#[test]
fn traduce_una_figura_de_tres_textos_en_una_llamada_y_un_paso() {
    let mut s = Escena::nueva();
    let a = s.anadir(caja(0.0, 0.0, 200.0, 80.0));
    let b = s.anadir(caja(300.0, 0.0, 200.0, 80.0));
    let ta = s.anadir(centrado_en(
        "Inicio del proceso",
        s.buscar(a).unwrap(),
        20.0,
    ));
    let tb = s.anadir(centrado_en("Revisar datos", s.buscar(b).unwrap(), 20.0));
    let titulo = s.anadir(texto("Diagrama de flujo", 0.0, 120.0, 24.0, None));
    let centros = (centro_x(&s, ta), centro_x(&s, tb));
    let antes = s.clone();

    let h = llamar(
        &mut s,
        json!({"cambios": [
            {"ref": format!("#{ta}"), "texto": "流程开始"},
            {"ref": format!("#{tb}"), "texto": "检查数据"},
            {"ref": format!("#{titulo}"), "texto": "流程图"}
        ]}),
    )
    .unwrap();
    assert!(
        h.texto.starts_with("hecho: 3 textos cambiados"),
        "{}",
        h.texto
    );
    assert_eq!(h.ids.len(), 3);
    assert_eq!(dice(&s, ta), "流程开始");
    assert_eq!(dice(&s, tb), "检查数据");
    assert_eq!(dice(&s, titulo), "流程图");
    // Lo centrado sigue centrado; lo de la izquierda, en su sitio.
    assert!(
        (centro_x(&s, ta) - centros.0).abs() < 0.5,
        "{} {}",
        centro_x(&s, ta),
        centros.0
    );
    assert!((centro_x(&s, tb) - centros.1).abs() < 0.5);
    assert_eq!(s.buscar(titulo).unwrap().x, 0.0);
    // La caja se volvio a medir: cuatro letras miden menos que dieciocho.
    assert!(s.buscar(ta).unwrap().ancho < antes.buscar(ta).unwrap().ancho);
    // Todo de una: un deshacer lo devuelve.
    let mut copia = s.clone();
    assert!(copia.deshacer());
    for id in [ta, tb, titulo] {
        assert_eq!(dice(&copia, id), dice(&antes, id));
    }
    // Todo en chino: la revision no se queja del idioma.
    assert!(!h.texto.contains("sigue en"), "{}", h.texto);
}

#[test]
fn el_que_queda_sin_traducir_se_avisa() {
    let mut s = Escena::nueva();
    let t1 = s.anadir(texto("Inicio del proceso", 0.0, 0.0, 20.0, None));
    let t2 = s.anadir(texto("Revisar los datos", 0.0, 60.0, 20.0, None));
    let t3 = s.anadir(texto("Fin del proceso", 0.0, 120.0, 20.0, None));
    let h = llamar(&mut s, json!({"cambios": [{"ref": format!("#{t1}"), "texto": "流程开始"}, {"ref": format!("#{t2}"), "texto": "检查数据"}]})).unwrap();
    assert!(
        h.texto.contains(&format!("#{t3} sigue en espanol")),
        "{}",
        h.texto
    );
}

#[test]
fn la_caja_con_rotulo_cambia_su_rotulo_y_avisa_si_no_cabe() {
    let mut s = Escena::nueva();
    let (c, r) = caja_con_rotulo(&mut s, "Inicio", 0.0, 0.0, 120.0, 70.0);
    // Por la caja: cambia su rotulo, centrado en ella.
    let h = llamar(&mut s, json!({"cambios": [{"ref": c, "texto": "Start"}]})).unwrap();
    assert_eq!(h.ids, vec![r]);
    assert_eq!(dice(&s, r), "Start");
    assert!((centro_x(&s, r) - 60.0).abs() < 1.0, "{}", centro_x(&s, r));
    // Algo largo no cabe: se dice, y no se toca la letra.
    let largo = "Comprobar que todos los datos de entrada son validos y estan completos";
    let h = llamar(&mut s, json!({"que": format!("#{r}"), "texto": largo})).unwrap();
    assert!(
        h.texto.contains("no caben") && h.texto.contains(&format!("#{r} no cabe en #{c}")),
        "{}",
        h.texto
    );
    assert!(matches!(s.buscar(r).unwrap().figura, Figura::Texto { tam, .. } if tam == 20.0));
    // Con «ajustar»: la letra se encoge hasta que cabe.
    let h = llamar(
        &mut s,
        json!({"accion": "ajustar", "que": format!("#{r}"), "ajustar": "encoger"}),
    )
    .unwrap();
    assert!(h.texto.contains("letra encogida"), "{}", h.texto);
    assert!(!h.texto.contains("no caben"), "{}", h.texto);
    let t = s.buscar(r).unwrap();
    assert!(matches!(t.figura, Figura::Texto { tam, .. } if tam < 20.0));
    let (x0, y0, x1, y1) = t.caja();
    assert!(
        x0 >= 0.0 && y0 >= 0.0 && x1 <= 120.0 && y1 <= 70.0,
        "dentro de la caja: {:?}",
        t.caja()
    );
}

#[test]
fn partir_un_texto_suelto_a_lo_ancho_de_su_caja() {
    let mut s = Escena::nueva();
    let c = s.anadir(caja(0.0, 0.0, 160.0, 120.0));
    let t = s.anadir(centrado_en("Hola", s.buscar(c).unwrap(), 16.0));
    let h = llamar(&mut s, json!({"cambios": [{"ref": format!("#{t}"), "texto": "Un texto bastante mas largo que la caja"}], "ajustar": "partir"})).unwrap();
    assert!(!h.texto.contains("no caben"), "{}", h.texto);
    assert!(
        dice(&s, t).contains('\n'),
        "partido en renglones: {:?}",
        dice(&s, t)
    );
    assert!((centro_x(&s, t) - 80.0).abs() < 1.0);
}

#[test]
fn errores_que_dicen_como_arreglarlo() {
    let mut s = Escena::nueva();
    let t = s.anadir(texto("Hola", 0.0, 0.0, 20.0, None));
    let r = s.anadir(caja(0.0, 100.0, 50.0, 50.0));
    let e = llamar(&mut s, json!({"cambios": [{"ref": "#999", "texto": "x"}]})).unwrap_err();
    assert!(
        e.contains("no existe #999") && e.contains(&format!("#{t} \"Hola\"")),
        "{e}"
    );
    let e = llamar(
        &mut s,
        json!({"cambios": [{"ref": format!("#{r}"), "texto": "x"}]}),
    )
    .unwrap_err();
    assert!(
        e.contains("sin texto") && e.contains("dibujar_en_lienzo"),
        "{e}"
    );
    // Uno bueno y uno malo: se hace el bueno y se dice el malo.
    let h = llamar(&mut s, json!({"cambios": [{"ref": format!("#{t}"), "texto": "Adios"}, {"ref": "#999", "texto": "x"}]})).unwrap();
    assert!(
        h.texto.contains("1 texto cambiado")
            && h.texto.contains("no se pudo")
            && h.texto.contains("#999"),
        "{}",
        h.texto
    );
    // Validacion: un cambio sin texto, o sin ref.
    let f = fila(EDITAR_TEXTO).unwrap();
    let e = validar(f, json!({"cambios": [{"ref": "#1"}]}).as_object().unwrap())
        .unwrap_err()
        .to_string();
    assert!(e.contains("le falta «texto»"), "{e}");
    let e = validar(f, json!({"cambios": [{"texto": "x"}]}).as_object().unwrap())
        .unwrap_err()
        .to_string();
    assert!(e.contains("le falta «ref»"), "{e}");
}

#[test]
fn los_cambios_valen_como_mapa_o_como_texto_json() {
    let f = fila(EDITAR_TEXTO).unwrap();
    let m = validar(
        f,
        json!({"cambios": {"#3": "uno", "4": "dos"}})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        m.args["cambios"],
        json!([{"ref": "#3", "texto": "uno"}, {"ref": "#4", "texto": "dos"}])
    );
    let t = validar(
        f,
        json!({"cambios": "[{\"id\": 5, \"text\": \"tres\"}]"})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(t.args["cambios"], json!([{"ref": "#5", "texto": "tres"}]));
    // El esquema lo dice: lista de objetos con ref y texto.
    let h = herramienta(f);
    assert_eq!(
        h.parametros["properties"]["cambios"]["items"]["required"],
        json!(["ref", "texto"])
    );
}

#[test]
fn listar_da_todos_los_textos_enteros_y_no_toca_nada() {
    let mut s = Escena::nueva();
    let largo = "Este es un parrafo bastante largo que antes el resumen cortaba a las cuarenta letras y no dejaba traducirlo entero";
    let a = s.anadir(texto(largo, 0.0, 200.0, 16.0, None));
    let (c, r) = caja_con_rotulo(&mut s, "Fin", 0.0, 0.0, 100.0, 40.0);
    let h = llamar(&mut s, json!({"accion": "listar"})).unwrap();
    assert!(h.ids.is_empty());
    assert!(h.texto.starts_with("2 textos"), "{}", h.texto);
    assert!(h.texto.contains(largo));
    assert!(
        h.texto.contains(&format!("#{r} \"Fin\""))
            && h.texto.contains(&format!("(rotulo de #{c})")),
        "{}",
        h.texto
    );
    // De arriba abajo: el rotulo (y = 0) antes que el parrafo (y = 200).
    assert!(h.texto.find(&format!("#{r} ")).unwrap() < h.texto.find(&format!("#{a} ")).unwrap());
    // Y el resumen ya trae el texto entero y de quien es rotulo.
    let res = resumen(&s, &Contexto::default());
    assert!(res.contains(largo), "{res}");
    assert!(
        res.contains(&format!("(rotulo de #{c})")) && res.contains(&format!("(rotulo #{r})")),
        "{res}"
    );
}

#[test]
fn el_resumen_largo_trae_los_textos_viejos() {
    let mut s = Escena::nueva();
    let viejo = s.anadir(texto("Titulo viejo", 0.0, 0.0, 20.0, None));
    for i in 0..(MAX_RESUMEN + 5) {
        s.anadir(caja(0.0, 100.0 + i as f32 * 60.0, 50.0, 50.0));
    }
    let res = resumen(&s, &Contexto::default());
    assert!(
        res.contains(&format!("Textos de esos: #{viejo} \"Titulo viejo\"")),
        "{res}"
    );
}
