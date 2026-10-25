//! Puerto de `ChatsCompartidosTest.kt` (quien esta en cada chat y que se
//! cree de lo que llega), mas los vectores de Java que prueban que las
//! claves, las firmas y el secreto son los del movil.

use super::*;

fn yo(id: &str, nombre: &str, codigo: &str) -> Yo {
    Yo {
        id: id.into(),
        nombre: nombre.into(),
        codigo: codigo.into(),
        llaves: Llaves::nuevas(),
    }
}

struct Gente {
    max: Yo,
    ana: Yo,
    eva: Yo,
    luis: Yo,
}

fn gente() -> Gente {
    Gente {
        max: yo("id-max", "Max", "MAXX"),
        ana: yo("id-ana", "Ana", "ANAA"),
        eva: yo("id-eva", "Eva", "EVAA"),
        luis: yo("id-luis", "Luis", "LUIS"),
    }
}

fn con_ana_y_eva(g: &Gente) -> ChatCompartido {
    let c = crear("p1", "Tesis", &g.max, 100);
    let c = invitar(&c, &g.max, &g.ana.ficha(0), 200).unwrap();
    invitar(&c, &g.ana, &g.eva.ficha(0), 300).unwrap()
}

fn ids(c: &ChatCompartido) -> Vec<&str> {
    c.personas.iter().map(|p| p.id.as_str()).collect()
}

fn dentro_ids(c: &ChatCompartido) -> HashSet<String> {
    c.personas
        .iter()
        .filter(|p| p.dentro())
        .map(|p| p.id.clone())
        .collect()
}

#[test]
fn crear_e_invitar_deja_dentro_a_todos_y_con_firmas_buenas() {
    let g = gente();
    let c = con_ana_y_eva(&g);
    assert_eq!(ids(&c), ["id-max", "id-ana", "id-eva"]);
    assert!(c.personas.iter().all(Persona::dentro));
    // Lo que llega a quien no tenia nada se cree entero.
    assert_eq!(juntar(None, &c), c);
    assert_eq!(en_comun(&[c.clone()], "id-ana", "id-eva"), ["p1"]);
    let a = autores(&c);
    assert_eq!(a.get("MAXX").map(String::as_str), Some("Max"));
    assert_eq!(a.get("ANAA").map(String::as_str), Some("Ana"));
    assert_eq!(a.get("EVAA").map(String::as_str), Some("Eva"));
}

#[test]
fn juntar_da_lo_mismo_en_cualquier_orden() {
    let g = gente();
    let base = invitar(&crear("p1", "Tesis", &g.max, 100), &g.max, &g.ana.ficha(0), 200).unwrap();
    let de_ana = invitar(&base, &g.ana, &g.eva.ficha(0), 300).unwrap();
    let de_max = invitar(&base, &g.max, &g.luis.ficha(0), 310).unwrap();
    let a = juntar(Some(&de_ana), &de_max);
    let b = juntar(Some(&de_max), &de_ana);
    let ordenar = |c: &ChatCompartido| {
        let mut v = c.personas.clone();
        v.sort_by(|x, y| x.id.cmp(&y.id));
        v
    };
    assert_eq!(ordenar(&a), ordenar(&b));
    assert_eq!(
        dentro_ids(&a),
        ["id-max", "id-ana", "id-eva", "id-luis"].map(String::from).into()
    );
    // Juntar dos veces no cambia nada.
    assert_eq!(juntar(Some(&a), &de_ana), a);
}

#[test]
fn solo_el_creador_quita_a_otros_y_cada_uno_puede_irse() {
    let g = gente();
    let c = con_ana_y_eva(&g);
    assert!(!puede_quitar(&c, "id-ana", "id-eva"));
    assert!(puede_quitar(&c, "id-max", "id-eva"));
    assert!(puede_quitar(&c, "id-eva", "id-eva"));
    // Caso negativo: Ana no puede, y se dice.
    assert!(quitar(&c, &g.ana, "id-eva", 400).is_err());
    let sin_eva = quitar(&c, &g.max, "id-eva", 400).unwrap();
    assert!(!dentro(&sin_eva, "id-eva"));
    assert!(en_comun(&[sin_eva.clone()], "id-ana", "id-eva").is_empty());
    let en_ana = juntar(Some(&c), &sin_eva);
    assert!(!dentro(&en_ana, "id-eva"));
    let sin_ana = quitar(&en_ana, &g.ana, "id-ana", 500).unwrap();
    assert!(!dentro(&juntar(Some(&sin_eva), &sin_ana), "id-ana"));
}

#[test]
fn una_salida_que_no_firma_el_creador_no_se_cree() {
    let g = gente();
    let c = con_ana_y_eva(&g);
    let eva0 = c.persona("id-eva").unwrap().clone();
    // Ana intenta echar a Eva diciendo que fue el creador, con su firma.
    let mut falsa = Persona {
        fuera: 400,
        quito: Some("id-max".into()),
        firma_fuera: None,
        ..eva0.clone()
    };
    falsa.firma_fuera = Some(g.ana.llaves.firmar(&texto_de_salida("p1", &falsa)));
    let cambiar = |p: &Persona| ChatCompartido {
        personas: c
            .personas
            .iter()
            .map(|x| if x.id == "id-eva" { p.clone() } else { x.clone() })
            .collect(),
        ..c.clone()
    };
    assert!(dentro(&juntar(Some(&c), &cambiar(&falsa)), "id-eva"));
    // Ni con «quito ella misma» firmado por otro.
    let mut otra = Persona {
        fuera: 400,
        quito: Some("id-eva".into()),
        firma_fuera: None,
        ..eva0
    };
    otra.firma_fuera = Some(g.ana.llaves.firmar(&texto_de_salida("p1", &otra)));
    assert!(dentro(&juntar(Some(&c), &cambiar(&otra)), "id-eva"));
}

#[test]
fn nadie_le_cambia_la_clave_ni_el_nombre_a_otro() {
    let g = gente();
    let c = con_ana_y_eva(&g);
    let impostora = yo("id-eva", "Eva", "EVAA");
    let trampa = invitar(&crear("p1", "Tesis", &g.max, 100), &g.max, &impostora.ficha(0), 999).unwrap();
    let junto = juntar(Some(&c), &trampa);
    assert_eq!(junto.persona("id-eva").unwrap().clave, g.eva.clave());
    // Cambiarle el nombre sin volver a firmar tampoco cuela.
    let renombrada = ChatCompartido {
        personas: c
            .personas
            .iter()
            .map(|x| {
                if x.id == "id-eva" {
                    Persona {
                        nombre: "Max".into(),
                        desde: 9_999,
                        ..x.clone()
                    }
                } else {
                    x.clone()
                }
            })
            .collect(),
        ..c.clone()
    };
    assert_eq!(juntar(Some(&c), &renombrada).persona("id-eva").unwrap().nombre, "Eva");
}

#[test]
fn una_lista_inventada_no_entra_aunque_llegue_a_quien_no_tenia_nada() {
    let g = gente();
    let c = con_ana_y_eva(&g);
    let sin_firma = ChatCompartido {
        personas: c
            .personas
            .iter()
            .map(|p| Persona {
                firma: "AAAA".into(),
                ..p.clone()
            })
            .collect(),
        ..c.clone()
    };
    assert!(juntar(None, &sin_firma).personas.is_empty());
    let huerfana = ChatCompartido {
        personas: c.personas.iter().filter(|p| p.id != "id-ana").cloned().collect(),
        ..c.clone()
    };
    assert_eq!(ids(&juntar(None, &huerfana)), ["id-max"]);
    // Y un creador que dice ser otro: quien se firma a si mismo sin ser el
    // creador no entra.
    let mut colada = g.luis.ficha(50);
    colada.invito = "id-luis".into();
    colada.firma = g.luis.llaves.firmar(&texto_de_entrada("p1", &colada));
    let mut con_luis = c.clone();
    con_luis.personas.push(colada);
    assert!(!dentro(&juntar(Some(&c), &con_luis), "id-luis"));
}

#[test]
fn quien_vuelve_invitado_otra_vez_esta_dentro() {
    let g = gente();
    let c = quitar(&con_ana_y_eva(&g), &g.max, "id-eva", 400).unwrap();
    let otra_vez = invitar(&c, &g.ana, &g.eva.ficha(0), 350).unwrap();
    assert!(dentro(&otra_vez, "id-eva"));
    assert!(dentro(&juntar(Some(&c), &otra_vez), "id-eva"));
    assert!(dentro(&juntar(Some(&otra_vez), &c), "id-eva"));
}

#[test]
fn quien_ya_no_esta_no_invita() {
    let g = gente();
    let c = quitar(&con_ana_y_eva(&g), &g.max, "id-ana", 400).unwrap();
    assert!(invitar(&c, &g.ana, &g.luis.ficha(0), 500).is_err());
    // Ni se le cambia la clave a quien ya esta.
    let impostor = yo("id-eva", "Eva", "EVAA");
    assert!(invitar(&c, &g.max, &impostor.ficha(0), 500).is_err());
}

#[test]
fn por_clave_solo_se_encuentra_a_quien_esta_en_un_chat_conmigo() {
    let g = gente();
    let c = con_ana_y_eva(&g);
    let l = [c.clone()];
    assert_eq!(por_clave(&l, "id-ana", &g.eva.clave()).unwrap().id, "id-eva");
    assert!(por_clave(&l, "id-luis", &g.eva.clave()).is_none());
    assert!(por_clave(&l, "id-ana", &g.luis.clave()).is_none());
    let sin_eva = [quitar(&c, &g.max, "id-eva", 400).unwrap()];
    assert!(por_clave(&sin_eva, "id-ana", &g.eva.clave()).is_some());
    assert_eq!(con_rastro(&sin_eva, "id-ana", "id-eva"), ["p1"]);
    assert!(en_comun(&sin_eva, "id-ana", "id-eva").is_empty());
}

#[test]
fn entre_aparatos_del_grupo_el_otro_entra_en_mis_chats_compartidos() {
    let g = gente();
    let c = invitar(&crear("p1", "Tesis", &g.max, 100), &g.max, &g.ana.ficha(0), 200).unwrap();
    let tableta = yo("id-max-tab", "Max", "MTAB");
    let en_el_telefono = juntar_del_grupo(&[c.clone()], &[], &g.max, Some(&tableta.ficha(0)), 300);
    assert!(dentro(&en_el_telefono[0], "id-max-tab"));
    let en_la_tableta = juntar_del_grupo(&[], &en_el_telefono, &tableta, Some(&g.max.ficha(0)), 300);
    assert_eq!(dentro_ids(&en_la_tableta[0]), dentro_ids(&en_el_telefono[0]));
    assert!(dentro(&juntar(Some(&c), &en_el_telefono[0]), "id-max-tab"));
}

#[test]
fn las_claves_acuerdan_el_mismo_secreto_y_firman() {
    let g = gente();
    let s1 = g.max.llaves.acordar(g.ana.llaves.publica()).unwrap();
    let s2 = g.ana.llaves.acordar(g.max.llaves.publica()).unwrap();
    assert_eq!(s1, s2);
    let f = g.max.llaves.firmar(b"hola");
    assert!(Llaves::verificar(&g.max.clave(), b"hola", &f));
    assert!(!Llaves::verificar(&g.ana.clave(), b"hola", &f));
    assert!(!Llaves::verificar(&g.max.clave(), b"hola", "basura"));
    assert!(!Llaves::verificar(&g.max.clave(), b"adios", &f));
    assert!(!Llaves::verificar("no es base64 !!", b"hola", &f));
    assert!(g.max.llaves.acordar(b"basura").is_err());
    // La publica es la de Java: SPKI de P-256 sin comprimir, 91 bytes.
    assert_eq!(g.max.llaves.publica().len(), 91);
}

// ------------------------------------------------------ vectores de Java

/// Sacados con `Vectores.java` (en el informe del 12-oct): `KeyPairGenerator
/// ("EC")` con `secp256r1`, `Signature("SHA256withECDSA")` y
/// `KeyAgreement("ECDH")`, las mismas llamadas que `Llaves` de Android.
const MAX_PUBLICA: &str = "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEhaVHFo/5hOPsWex473SEo+YmgDXg7SPKVXOmVKwI1Ny5IGSbc5jXDzzOqFJ50bioFnF4d3KJDRRDLpOUJ6DvPw==";
const MAX_PRIVADA: &str = "MEECAQAwEwYHKoZIzj0CAQYIKoZIzj0DAQcEJzAlAgEBBCA7o0IrCwtogfwmAcMRkTu3+bNAYimDTocQMvKIanyqaA==";
const ANA_PUBLICA: &str = "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE1HW4yh9ZT/MndY+cqw1W+9yXS/5pFB3LELNauMp0mdGKaYTnfT/OMF4aNoewKF42YjQwh3bLmWeZwhXFf4iBvg==";
const ANA_PRIVADA: &str = "MEECAQAwEwYHKoZIzj0CAQYIKoZIzj0DAQcEJzAlAgEBBCCBUuKli3ITQQusMk95bTnnAMKp0KiLkhCTwE/qcR/n7Q==";
/// Firmas de Max: 0 y 3 sobre la entrada de Max (con tildes y eñe), 1 y 4
/// sobre la de Ana, 2 y 5 sobre la salida. La 2, 3, 4 y 5 llevan la `s`
/// alta, que Java no normaliza.
const FIRMAS: [&str; 6] = [
    "MEQCIHqA67LmvoepEaFPktGru6q0RJXs4oFijphmIXn5t3huAiAdXGao/Zqa6lNTitmCEVgtv67czlAZsCdUmNcnLTKc4A==",
    "MEQCIAbVI2yTygkts6iA2C0byML7ZAWMdA6kG+te6ylXcZvJAiBqvg82R3+w/g2O6tRhfpOVLzKQIT45/fHc/nQOCy605g==",
    "MEYCIQDdx6wjoj++r4IAZVueVqhdPk+xduXs52UfrhE7UKP3DwIhAN8gBfVYDjkMj0pWtYoj2kaVoSSTdhwlvIYZVMioAfRf",
    "MEYCIQC3r30dwywuFflu6JLtHHfAun/kn/a/+FzxAqwsI3R+4gIhAKvnXKttFh2RdK9jiobxdX069z8NUg6KJUn298UfBFyG",
    "MEUCIG/N6AxtqoAOQGJnHYLQ7kuMt+zqWyxv8EeBxAG3VQHSAiEAqIG8hCimepzVjDyj0aQ88lRJdQApJqD1Ml45fCCCDCk=",
    "MEYCIQDwBlynvsdMWWSHGLRZQBwAf+56fZOEm2MaU2MLcABkwQIhAJw/WaXeDQJ4RcJYH3TuZ2IgnkN/NtGlEqWeWEKFGmk8",
];
const SECRETO: &str = "fa16659066121088f5d3a7b458a2c15513b61989d4cca19c92649680ed84b3b0";

fn persona_de_java(id: &str, nombre: &str, codigo: &str, clave: &str, desde: i64) -> Persona {
    Persona {
        id: id.into(),
        nombre: nombre.into(),
        codigo: codigo.into(),
        clave: clave.into(),
        desde,
        invito: "id-max".into(),
        firma: String::new(),
        fuera: 0,
        quito: None,
        firma_fuera: None,
    }
}

#[test]
fn las_firmas_de_java_se_creen_aqui_sobre_los_mismos_textos() {
    let max = persona_de_java("id-max", "Máx Ñandú", "MAXX", MAX_PUBLICA, 100);
    let ana = persona_de_java("id-ana", "Ana", "ANAA", ANA_PUBLICA, 200);
    let sale = Persona {
        fuera: 400,
        quito: Some("id-max".into()),
        ..ana.clone()
    };
    let textos = [
        texto_de_entrada("p1", &max),
        texto_de_entrada("p1", &ana),
        texto_de_salida("p1", &sale),
    ];
    for (i, f) in FIRMAS.iter().enumerate() {
        assert!(Llaves::verificar(MAX_PUBLICA, &textos[i % 3], f), "firma {i}");
        // Caso negativo: sobre el texto de otro, o con la clave de Ana, no.
        assert!(!Llaves::verificar(MAX_PUBLICA, &textos[(i + 1) % 3], f), "firma {i} cruzada");
        assert!(!Llaves::verificar(ANA_PUBLICA, &textos[i % 3], f), "firma {i} con otra clave");
    }
    // Y la lista entera, firmada en Java, se junta aqui como alli.
    let c = ChatCompartido {
        chat: "p1".into(),
        nombre: "Tesis".into(),
        creador: "id-max".into(),
        creado: 100,
        personas: vec![
            Persona {
                firma: FIRMAS[0].into(),
                ..max
            },
            Persona {
                firma: FIRMAS[1].into(),
                fuera: 400,
                quito: Some("id-max".into()),
                firma_fuera: Some(FIRMAS[2].into()),
                ..ana
            },
        ],
    };
    let junto = juntar(None, &c);
    assert_eq!(junto, c);
    assert!(dentro(&junto, "id-max"));
    assert!(!dentro(&junto, "id-ana"));
}

#[test]
fn las_claves_de_java_se_leen_y_dan_su_mismo_secreto() {
    let max = Llaves::de(MAX_PUBLICA, MAX_PRIVADA).unwrap();
    let ana = Llaves::de(ANA_PUBLICA, ANA_PRIVADA).unwrap();
    assert_eq!(max.clave(), MAX_PUBLICA, "la publica sale igual que en Java");
    let hex = |b: [u8; 32]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    assert_eq!(hex(max.acordar(&desde_b64(ANA_PUBLICA).unwrap()).unwrap()), SECRETO);
    assert_eq!(hex(ana.acordar(&desde_b64(MAX_PUBLICA).unwrap()).unwrap()), SECRETO);
    // Una firma hecha aqui con la privada de Java se cree con su publica.
    assert!(Llaves::verificar(MAX_PUBLICA, b"x", &max.firmar(b"x")));
    // Caso negativo: la publica de uno con la privada de otro no se acepta.
    assert!(Llaves::de(ANA_PUBLICA, MAX_PRIVADA).is_err());
    assert!(Llaves::de(MAX_PUBLICA, "AAAA").is_err());
}

#[test]
fn el_json_es_el_de_kotlinx_con_los_nulos_escritos() {
    let c = ChatCompartido {
        chat: "p1".into(),
        nombre: "Tesis".into(),
        creador: "id-max".into(),
        creado: 1_791_000_000_000,
        personas: vec![Persona {
            id: "id-max".into(),
            nombre: "Ana".into(),
            codigo: "K7Q2".into(),
            clave: "QUJD".into(),
            desde: 1_791_000_000_000,
            invito: "id-max".into(),
            firma: "RklS".into(),
            fuera: 0,
            quito: None,
            firma_fuera: None,
        }],
    };
    assert_eq!(
        a_texto(std::slice::from_ref(&c)),
        r#"[{"chat":"p1","nombre":"Tesis","creador":"id-max","creado":1791000000000,"personas":[{"id":"id-max","nombre":"Ana","codigo":"K7Q2","clave":"QUJD","desde":1791000000000,"invito":"id-max","firma":"RklS","fuera":0,"quito":null,"firmaFuera":null}]}]"#
    );
    // Lo que escribe Kotlin sin los campos que tienen valor por defecto, o
    // con alguno de mas, tambien se lee.
    let corto = r#"[{"chat":"p1","nombre":"Tesis","creador":"id-max","creado":1,"otro":true,"personas":[{"id":"a","nombre":"A","codigo":"C","clave":"K","desde":2,"invito":"a","firma":"F"}]}]"#;
    let l = de_texto(Some(corto));
    assert_eq!(l[0].personas[0].fuera, 0);
    assert!(l[0].personas[0].quito.is_none());
    // Lo que no se entiende es una lista vacia, no un fallo.
    assert!(de_texto(Some("{basura")).is_empty());
    assert!(de_texto(None).is_empty());
}

#[test]
fn en_disco_la_privada_se_queda_y_el_nombre_se_guarda() {
    let dir = std::env::temp_dir().join(format!("pixpin-compartidos-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = CompartidosEnDisco::nuevo(&dir);
    let l1 = d.llaves().unwrap();
    let l2 = d.llaves().unwrap();
    assert_eq!(l1.clave(), l2.clave(), "nace una vez y se queda");
    assert!(d.nombre().is_none());
    d.poner_nombre("  Max  ").unwrap();
    assert_eq!(d.nombre().as_deref(), Some("Max"));
    let ficha: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sincro/persona.json")).unwrap())
            .unwrap();
    assert_eq!(ficha["publica"], l1.clave());
    assert!(ficha["privada"].as_str().is_some_and(|p| !p.is_empty()));
    let yo = d.yo("id-max", "Portátil").unwrap();
    assert_eq!(yo.nombre, "Max");
    assert_eq!(yo.codigo, crate::grupo::codigo_de_aparato("id-max"));
    let c = crear("p1", "Tesis", &yo, 1);
    d.poner(&c).unwrap();
    assert_eq!(d.de("p1"), Some(c));
    assert!(d.hay_alguno("id-max"));
    assert!(!d.hay_alguno("id-otro"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Para comprobar al reves (lo de aqui en Java): escribe una clave y una
/// firma hechas aqui. `java Vectores verificar <clave> <texto> <firma>`.
#[test]
#[ignore]
fn vectores_para_java() {
    let l = Llaves::nuevas();
    let otra = Llaves::nuevas();
    let texto = "pixpin-chat|entra|p1|id-pc|x|1|id-pc|Máx|PCPC";
    println!("CLAVE={}", l.clave());
    println!("TEXTO={texto}");
    println!("FIRMA={}", l.firmar(texto.as_bytes()));
    println!("PRIVADA_OTRA={}", otra.privada_b64());
    println!("PUBLICA={}", l.clave());
    let s = otra.acordar(l.publica()).unwrap();
    println!(
        "SECRETO={}",
        s.iter().map(|x| format!("{x:02x}")).collect::<String>()
    );
}

#[test]
fn las_claves_de_la_conversacion_entre_personas_son_las_de_java() {
    // `Sesion.java` (12-oct): `Canal.abrirEntrePersonas` con Max iniciando,
    // azar 1…1 y 2…2.
    let max = Llaves::de(MAX_PUBLICA, MAX_PRIVADA).unwrap();
    let ana_publica = desde_b64(ANA_PUBLICA).unwrap();
    let secreto = max.acordar(&ana_publica).unwrap();
    let c = crate::canal::claves_entre_personas(
        &secreto,
        &[1u8; 32],
        &[2u8; 32],
        max.publica(),
        &ana_publica,
        true,
    );
    let hex = |b: [u8; 32]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    assert_eq!(hex(c.salida), "2ebf223f65a92053e3562745c3c027c67ecfe764212115b0a75d90234c592485");
    assert_eq!(hex(c.entrada), "9a49a4851bdfc2b5540f8611ad4413647457843a05cc460b760524a41101399f");
    // Ana, que responde, las tiene al reves.
    let ana = Llaves::de(ANA_PUBLICA, ANA_PRIVADA).unwrap();
    let suyas = crate::canal::claves_entre_personas(
        &ana.acordar(max.publica()).unwrap(),
        &[1u8; 32],
        &[2u8; 32],
        max.publica(),
        &ana_publica,
        false,
    );
    assert_eq!(suyas.entrada, c.salida);
    assert_eq!(suyas.salida, c.entrada);
}

#[test]
fn la_clave_de_la_invitacion_es_la_de_java() {
    let hex = |b: [u8; 32]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    assert_eq!(
        hex(crate::invitacion::clave("K7Q2M9XP")),
        "4e1340d6faa7d9334901147b55d048623326e9ebb09386d79109a8b176d538f7"
    );
    assert_eq!(crate::invitacion::etiqueta("K7Q2M9XP"), "23d4106b5634be11");
}
