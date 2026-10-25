//! Las pruebas de la ventana de las figuras (`galeria_figuras.rs`): las de
//! `ventana/pruebas.rs` y `ventana_chat/figuras.rs` de Windows con el
//! puente del Mac.

use super::*;
use crate::editor::FasePuntero;
use std::path::PathBuf;

fn raiz(nombre: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!("pixpin-galfig-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&r);
    r
}

fn nucleo(r: &std::path::Path) -> Arc<Nucleo> {
    Nucleo::abrir(r.display().to_string(), "Mac".into()).unwrap()
}

fn editor(r: &std::path::Path) -> Arc<EditorLienzo> {
    std::fs::create_dir_all(r).unwrap();
    EditorLienzo::de_fichero(r.join("f.excalidraw").display().to_string(), true).unwrap()
}

fn figura(nombre: &str) -> FiguraGuardada {
    serie::catalogo()
        .into_iter()
        .map(|f| FiguraGuardada {
            id: format!("mia-{nombre}"),
            nombre: nombre.into(),
            ..f
        })
        .next()
        .unwrap()
}

fn valor(clave: &str, texto: &str) -> ValorDeCampo {
    ValorDeCampo {
        clave: clave.into(),
        texto: texto.into(),
    }
}

#[test]
fn las_categorias_van_en_su_orden_con_su_cuenta_y_mis_figuras_al_final() {
    let mut figuras = serie::catalogo();
    figuras.insert(0, figura("Mi árbol"));
    figuras.push(figura("Colección · Sello"));
    let cats = categorias(&figuras);
    assert_eq!(cats[0], serie::CATEGORIAS[0]);
    assert_eq!(cats.last().map(String::as_str), Some(MIS_FIGURAS));
    assert_eq!(
        cats[cats.len() - 2],
        "Colección",
        "la del usuario detras de las de serie"
    );
    // Todas salen una vez, en el orden de las categorias.
    let todas = visibles(&figuras, None, "");
    assert_eq!(todas.len(), figuras.len());
    assert_eq!(*todas.last().unwrap(), 0, "Mis figuras, al final");
    // Buscar sin tildes ni mayusculas, por el nombre o la categoria.
    let arbol = visibles(&figuras, None, "ARBOL");
    assert!(arbol.contains(&0));
    let diario = visibles(&figuras, Some("Diario"), "");
    assert!(!diario.is_empty());
    assert!(diario.iter().all(|i| de_que_es(&figuras[*i]).0 == "Diario"));
    // Caso negativo: lo que no esta no sale.
    assert!(visibles(&figuras, None, "zzzz nada").is_empty());
    assert!(visibles(&figuras, Some("No existe"), "").is_empty());
}

#[test]
fn el_intervalo_sale_de_los_campos_y_avisa_de_lo_que_no_es_un_numero() {
    assert_eq!(intervalo_de(&[]).unwrap(), None, "vacio: la de serie");
    assert_eq!(intervalo_de(&[valor("desde", "  ")]).unwrap(), None);
    let iv = intervalo_de(&[
        valor("desde", "1"),
        valor("hasta", "40"),
        valor("de_a", "10"),
    ])
    .unwrap()
    .unwrap();
    assert_eq!((iv.desde, iv.hasta, iv.de_a), (Some(1), Some(40), Some(10)));
    let iv = intervalo_de(&[valor("mes", "2026-11"), valor("semanas_desde", "3")])
        .unwrap()
        .unwrap();
    assert_eq!(iv.mes, Some((2026, 11)));
    assert_eq!(iv.semanas, Some((3, 3)));
    // Casos negativos.
    assert!(
        intervalo_de(&[valor("desde", "uno")])
            .unwrap_err()
            .contains("no es un número")
    );
    assert!(intervalo_de(&[valor("mes", "pronto")]).is_err());
    // Los campos de cada plantilla.
    assert_eq!(campos_de("planificador-mensual"), &["mes", "meses"]);
    assert!(campos_de("lista-de-tareas").contains(&"de_a"));
}

#[test]
fn la_ventana_busca_pinta_alarga_inserta_renombra_y_quita() {
    let r = raiz("ventana");
    let n = nucleo(&r);
    let g = n.clone().galeria_de_figuras().unwrap();
    // Las de serie entran solas, una vez por version.
    assert!(g.cuantas() as usize >= serie::catalogo().len());
    assert_eq!(
        std::fs::read_to_string(r.join(crate::editor_figuras::MARCA_DE_SERIE)).unwrap(),
        serie::VERSION.to_string()
    );
    let cats = g.categorias();
    assert_eq!(cats.last().unwrap().nombre, MIS_FIGURAS);
    assert_eq!(
        cats.last().unwrap().cuenta,
        0,
        "Mis figuras sale aunque este vacia"
    );
    assert!(cats.iter().take(cats.len() - 1).all(|c| c.cuenta > 0));
    let total: u32 = cats.iter().map(|c| c.cuenta).sum();
    assert_eq!(total, g.cuantas());

    // La plantilla que se alarga: sus campos, su previa y sus hojas.
    let lista = g.visibles(None, "lista de tareas".into());
    let tareas = lista
        .iter()
        .find(|f| f.id == "serie-lista-de-tareas")
        .expect("la lista de tareas")
        .clone();
    assert!(tareas.alargable && tareas.de_serie);
    assert_eq!(tareas.categoria, "Planificación");
    assert!(tareas.piezas > 0 && tareas.ancho > 0.0);
    let campos = g.campos(tareas.id.clone());
    assert_eq!(
        campos.iter().map(|c| c.clave.as_str()).collect::<Vec<_>>(),
        ["desde", "hasta", "de_a"]
    );
    assert!(!campos[0].pista.is_empty(), "la pista del tramo de serie");
    assert!(campos[2].a_lo_ancho);
    assert_eq!(g.previa(tareas.id.clone(), vec![]), None);
    let pedido = vec![
        valor("desde", "1"),
        valor("hasta", "40"),
        valor("de_a", "10"),
    ];
    assert_eq!(
        g.previa(tareas.id.clone(), pedido.clone()),
        Some(PreviaDeIntervalo {
            texto: "4 hojas".into(),
            error: false
        })
    );
    assert!(
        g.previa(tareas.id.clone(), vec![valor("desde", "x")])
            .unwrap()
            .error
    );

    // La miniatura y el detalle.
    let plano = g.plano(tareas.id.clone()).expect("su miniatura");
    let (cab, ordenes) = crate::lienzo::decodificar(&plano).unwrap();
    assert!(!ordenes.is_empty());
    let _ = cab;
    let d = g.detalle(tareas.id.clone()).unwrap();
    assert_eq!(d.total, d.trazos + d.lineas + d.formas + d.textos + d.otras);
    assert!(!d.letras.is_empty());
    assert!(g.plano("no-esta".into()).is_none());

    // Insertar: alargada, cuatro hojas en fila, en un paso y elegidas.
    let ed = editor(&r.join("lienzo"));
    let iv = Intervalo {
        desde: Some(1),
        hasta: Some(40),
        de_a: Some(10),
        ..Default::default()
    };
    let hojas = serie::alargar("lista-de-tareas", &iv).unwrap();
    assert_eq!(hojas.len(), 4);
    assert!(
        g.insertar(tareas.id.clone(), pedido, ed.clone(), 0.0, 0.0)
            .unwrap()
    );
    let n_hojas = ed.cuantos_elementos();
    let piezas: usize = hojas.iter().map(|h| h.elementos.len()).sum();
    assert_eq!(n_hojas, piezas as u64, "las cuatro hojas enteras");
    assert_eq!(ed.cuantos_elegidos(), n_hojas);
    assert!(ed.deshacer());
    assert_eq!(ed.cuantos_elementos(), 0, "un solo paso de deshacer");
    // Tal cual, sin intervalo.
    assert!(
        g.insertar(tareas.id.clone(), vec![], ed.clone(), 0.0, 0.0)
            .unwrap()
    );
    assert_eq!(ed.cuantos_elementos(), tareas.piezas as u64);
    // Lo que se propone al guardarla otra vez: con su categoria.
    assert!(
        ed.nombre_propuesto_de_figura()
            .starts_with("Planificación · ")
    );
    // Casos negativos: un intervalo que no vale y una que no esta.
    assert!(
        g.insertar(
            tareas.id.clone(),
            vec![valor("desde", "x")],
            ed.clone(),
            0.0,
            0.0
        )
        .is_err()
    );
    assert!(
        !g.insertar("no-esta".into(), vec![], ed.clone(), 0.0, 0.0)
            .unwrap()
    );

    // Una propia: se guarda, se renombra a una categoria y se quita.
    ed.seleccionar_todo();
    assert!(n.guardar_figura(ed.clone(), "Mi hoja".into()).unwrap());
    g.releer().unwrap();
    let mia = g.visibles(Some(MIS_FIGURAS.into()), String::new());
    assert_eq!(mia.len(), 1);
    assert!(!mia[0].de_serie);
    assert!(
        g.renombrar(mia[0].id.clone(), "Colección · Mi hoja".into())
            .unwrap()
    );
    assert!(
        !g.renombrar(mia[0].id.clone(), "  ".into()).unwrap(),
        "vacio no"
    );
    assert!(
        g.categorias()
            .iter()
            .any(|c| c.nombre == "Colección" && c.cuenta == 1)
    );
    // Lo guardado viaja: otra galeria lo lee del disco.
    let otra = n.clone().galeria_de_figuras().unwrap();
    assert!(
        otra.visibles(Some("Colección".into()), String::new())
            .iter()
            .any(|f| f.corto == "Mi hoja")
    );
    assert!(g.quitar(mia[0].id.clone()).unwrap());
    assert!(!g.quitar(mia[0].id.clone()).unwrap(), "ya no esta");
    otra.releer().unwrap();
    assert_eq!(otra.cuantas(), g.cuantas());
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn una_figura_va_al_chat_en_su_lienzo_y_un_lienzo_del_chat_a_mis_figuras() {
    let r = raiz("chat");
    let n = nucleo(&r);
    let chat = n.crear_chat("Obra".into()).unwrap().id;
    // El menu de adjuntar: lo del usuario primero.
    let ed = editor(&r.join("lienzo"));
    ed.herramienta("rectangulo".into()).unwrap();
    ed.puntero(FasePuntero::Pulsar, 0.0, 0.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Mover, 50.0, 40.0, None, 0, 1.0);
    ed.puntero(FasePuntero::Soltar, 50.0, 40.0, None, 0, 1.0);
    ed.herramienta("seleccion".into()).unwrap();
    ed.seleccionar_todo();
    assert!(n.guardar_figura(ed.clone(), "Caja".into()).unwrap());
    let grupos = n.figuras_por_grupos().unwrap();
    assert_eq!(grupos[0].nombre, MIS_FIGURAS);
    assert_eq!(grupos[0].figuras[0].nombre, "Caja");
    assert_eq!(grupos[1].nombre, serie::CATEGORIAS[0]);
    let total: usize = grupos.iter().map(|g| g.figuras.len()).sum();
    assert_eq!(
        total,
        n.figuras_guardadas().unwrap().len(),
        "cada una una vez"
    );
    assert!(
        grupos[1]
            .figuras
            .iter()
            .all(|f| !f.nombre.contains(serie::SEPARADOR)),
        "el nombre corto"
    );

    // Adjuntarla: un lienzo con su nombre y la figura entera dentro.
    let caja = grupos[0].figuras[0].id.clone();
    let m = n.adjuntar_figura(chat.clone(), caja).unwrap();
    assert_eq!(m.clase, "DIBUJO");
    assert_eq!(m.nombre, "Caja");
    let dibujo = m.referencia.clone().unwrap();
    let l = n.abrir_lienzo(chat.clone(), dibujo.clone()).unwrap();
    assert_eq!(l.cuantos_elementos(), 1);
    assert!(n.adjuntar_figura(chat.clone(), "no-esta".into()).is_err());

    // Y de vuelta, con otro nombre (y «Figura de <chat>» sin nombre).
    assert_eq!(
        n.guardar_lienzo_en_figuras(chat.clone(), dibujo.clone(), "Caja 2".into(), "Obra".into())
            .unwrap()
            .as_deref(),
        Some("Caja 2")
    );
    assert_eq!(
        n.guardar_lienzo_en_figuras(
            chat.clone(),
            dibujo.clone(),
            format!("{dibujo}.excalidraw"),
            "Obra".into()
        )
        .unwrap()
        .as_deref(),
        Some("Figura de Obra")
    );
    let nombres: Vec<String> = n
        .figuras_guardadas()
        .unwrap()
        .into_iter()
        .map(|f| f.nombre)
        .collect();
    assert_eq!(&nombres[..3], ["Figura de Obra", "Caja 2", "Caja"]);
    // Casos negativos: un lienzo vacio no se guarda y un nombre raro no vale.
    let vacio = n
        .crear_lienzo(chat.clone(), None)
        .unwrap()
        .referencia
        .unwrap();
    assert_eq!(
        n.guardar_lienzo_en_figuras(chat.clone(), vacio, String::new(), "Obra".into())
            .unwrap(),
        None
    );
    assert!(
        n.guardar_lienzo_en_figuras(chat, "../x".into(), String::new(), "Obra".into())
            .is_err()
    );
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn las_de_serie_se_ponen_una_vez_y_no_pisan_las_propias() {
    let r = raiz("serie");
    let ruta = r.join("figuras.json");
    std::fs::create_dir_all(&r).unwrap();
    assert!(biblioteca::guardar(&ruta, &[figura("Mía")]));
    assert!(crate::editor_figuras::instalar_de_serie(&r, &ruta));
    let lista = biblioteca::cargar(&ruta);
    assert_eq!(lista[0].nombre, "Mía", "las propias, delante y en su orden");
    assert_eq!(lista.len(), 1 + serie::catalogo().len());
    // Con la marca puesta no se vuelven a poner.
    assert!(!crate::editor_figuras::instalar_de_serie(&r, &ruta));
    let _ = std::fs::remove_dir_all(&r);
}
