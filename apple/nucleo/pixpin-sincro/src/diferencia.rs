//! Que hay que hacer para que dos aparatos queden iguales.
//!
//! Puerto de `sincro/Diferencia.kt`. Es el corazon de sincronizar, y esta
//! aqui sin red a proposito: quien gana cuando los dos tocaron lo mismo, que
//! pasa si uno lo borro, todo vive en una funcion pura que se prueba sin dos
//! aparatos. Lo que va por el cable es lo facil; lo que se decide aqui es lo
//! que se puede hacer mal sin que se note hasta que alguien pierde un plano.
//!
//! El [Apunte] es el mismo que viaja en el inventario (`mensajes::Apunte`):
//! no se duplica.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::canonico::comparar_como_kotlin;
pub use crate::mensajes::Apunte;

/// El prefijo de una marca de borrado de antes de los codigos, que solo
/// sabe la sena del mensaje (`sena:47a`).
pub const MARCA_VIEJA: &str = "sena:";

/// Un paso del plan de sincronizacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paso {
    /// Me lo traigo del otro (o su borrado, si alli se borro).
    Traer(String),
    /// Se lo mando al otro (o mi borrado).
    Mandar(String),
    /// Cambio en los dos: se juntan (15-sep-2026). Ya no se pregunta ni
    /// manda un aparato; se suma lo de cada lado figura por figura, celda
    /// por celda, parrafo por parrafo. Ver `fusion`.
    Fusionar(String),
}

impl Paso {
    pub fn sena(&self) -> &str {
        match self {
            Paso::Traer(s) | Paso::Mandar(s) | Paso::Fusionar(s) => s,
        }
    }
}

/// El plan para ponerse al dia con el otro aparato.
///
/// `mio` y `suyo` son los inventarios de cada uno. `base` es lo que se
/// acordo la ultima vez con este aparato: la clave y el resumen que tenian
/// cuando termino aquella sincronizacion. Con ella se sabe quien se movio:
/// el que difiere de la base.
///
/// Nadie manda (lo pidio el usuario el 15-sep-2026, tras perder lienzos con
/// un aparato de maestro):
///
/// - Cambio en un solo lado: pasa al otro.
/// - Cambio en los dos (o nunca se acordo nada y son distintos): se fusiona.
/// - Borrado en un lado y sin tocar en el otro: se borra en los dos.
/// - Borrado en un lado y cambiado en el otro (o sin base con que saberlo):
///   se queda lo cambiado, en los dos. Borrar quita lo que uno vio; lo que
///   no vio, sobrevive.
///
/// Los pasos salen ordenados por clave para que dos aparatos calculen la
/// misma lista en el mismo orden.
pub fn plan(mio: &[Apunte], suyo: &[Apunte], base: &BTreeMap<String, String>) -> Vec<Paso> {
    let aqui = por_sena(con_marcas_viejas(mio, suyo));
    let alli = por_sena(con_marcas_viejas(suyo, mio));
    let mut senas: Vec<&String> = aqui.keys().chain(alli.keys()).collect();
    // El orden es el de Kotlin (`sorted()` sobre String), por si una clave
    // lleva letras fuera del plano basico.
    senas.sort_by(|a, b| comparar_como_kotlin(a, b));
    senas.dedup();
    let mut pasos = Vec::new();
    for sena in senas {
        let a = aqui.get(sena);
        let b = alli.get(sena);
        match (a, b) {
            // Solo el lo tiene. Si lo suyo es una marca de borrado, no hay
            // nada que traerse: lo que el borro yo ni siquiera lo llegue a
            // tener.
            (None, Some(b)) => {
                if !b.borrado {
                    pasos.push(Paso::Traer(sena.clone()));
                }
            }
            (None, None) => {}
            // Solo yo lo tengo: se lo mando, borrado incluido. La marca
            // tambien viaja, o el me lo devolveria en la siguiente vuelta.
            (Some(_), None) => pasos.push(Paso::Mandar(sena.clone())),
            (Some(a), Some(b)) => {
                if a.borrado && b.borrado {
                    // De acuerdo en que no esta.
                    continue;
                }
                if a.resumen == b.resumen && !a.borrado && !b.borrado {
                    // Identicos.
                    continue;
                }
                // Lo acordado antes de los codigos va por la sena: se busca
                // tambien por ella, primero la mia y luego la suya.
                let acordado = base
                    .get(sena)
                    .or_else(|| a.alias.as_ref().and_then(|al| base.get(al)))
                    .or_else(|| b.alias.as_ref().and_then(|al| base.get(al)));
                pasos.push(que_hacer_con_los_dos(
                    sena,
                    a,
                    b,
                    acordado.map(String::as_str),
                ));
            }
        }
    }
    pasos
}

fn que_hacer_con_los_dos(sena: &str, a: &Apunte, b: &Apunte, base: Option<&str>) -> Paso {
    let sena = sena.to_string();
    if a.borrado != b.borrado {
        let vivo_es_mio = b.borrado;
        let vivo = if vivo_es_mio { a } else { b };
        // Lo modificado gana a lo borrado. Sin base no se sabe si se toco:
        // se conserva.
        let tocado = base.is_none_or(|base| vivo.resumen != base);
        return if tocado {
            if vivo_es_mio {
                Paso::Mandar(sena)
            } else {
                Paso::Traer(sena)
            }
        } else if vivo_es_mio {
            // Intacto contra borrado: el borrado pasa al lado que aun lo
            // tenia.
            Paso::Traer(sena)
        } else {
            Paso::Mandar(sena)
        };
    }
    let Some(base) = base else {
        return Paso::Fusionar(sena);
    };
    let yo_me_movi = a.resumen != base;
    let el_se_movio = b.resumen != base;
    match (yo_me_movi, el_se_movio) {
        (false, true) => Paso::Traer(sena),
        (true, false) => Paso::Mandar(sena),
        _ => Paso::Fusionar(sena),
    }
}

/// `associateBy { it.sena }`: de dos apuntes con la misma sena se queda el
/// ultimo.
fn por_sena(lista: Vec<Apunte>) -> HashMap<String, Apunte> {
    let mut salida = HashMap::with_capacity(lista.len());
    for ap in lista {
        salida.insert(ap.sena.clone(), ap);
    }
    salida
}

/// Las marcas de borrado de antes de los codigos solo sabian la sena
/// (`47a`). Si el otro aparato tiene vivo un mensaje con esa sena, la marca
/// pasa a llevar su codigo unico, y asi se sigue reconociendo como el
/// borrado de ese mensaje. Si aqui ya hay vivo uno con ese codigo, la marca
/// sobra y se tira.
pub fn con_marcas_viejas(lista: &[Apunte], del_otro: &[Apunte]) -> Vec<Apunte> {
    let es_vieja = |ap: &Apunte| ap.borrado && ap.sena.starts_with(MARCA_VIEJA);
    if !lista.iter().any(es_vieja) {
        return lista.to_vec();
    }
    // `associate`: el ultimo gana, y `lista` va detras de `delOtro`.
    let mut por_alias: HashMap<&str, &str> = HashMap::new();
    for ap in del_otro.iter().chain(lista.iter()) {
        if !ap.borrado {
            if let Some(alias) = &ap.alias {
                por_alias.insert(alias, &ap.sena);
            }
        }
    }
    let vivos: HashSet<&str> = lista
        .iter()
        .filter(|ap| !ap.borrado)
        .map(|ap| ap.sena.as_str())
        .collect();
    lista
        .iter()
        .filter_map(|ap| {
            if !es_vieja(ap) {
                return Some(ap.clone());
            }
            let sena_vieja = &ap.sena[MARCA_VIEJA.len()..];
            let Some(&clave) = por_alias.get(sena_vieja) else {
                return Some(ap.clone());
            };
            if vivos.contains(clave) {
                None
            } else {
                Some(Apunte {
                    sena: clave.to_string(),
                    ..ap.clone()
                })
            }
        })
        .collect()
}

/// Donde va cada mensaje que llega: por su hora de creacion.
///
/// Si el orden dependiera de cuando llego, cada aparato tendria el chat en
/// un orden distinto y «mira el 47» dejaria de senalar el mismo sitio. A
/// igualdad de hora ordena la sena, que es unica: ni dos mensajes creados
/// en el mismo milisegundo en dos aparatos dejan el orden al azar.
pub fn ordenar(apuntes: &[Apunte]) -> Vec<Apunte> {
    let mut salida = apuntes.to_vec();
    salida.sort_by(|a, b| {
        a.creado
            .cmp(&b.creado)
            .then_with(|| comparar_como_kotlin(&a.sena, &b.sena))
    });
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ap(sena: &str, resumen: &str) -> Apunte {
        Apunte {
            sena: sena.into(),
            resumen: resumen.into(),
            ..Default::default()
        }
    }

    fn creado(sena: &str, resumen: &str, creado: i64) -> Apunte {
        Apunte {
            creado,
            tocado: creado,
            ..ap(sena, resumen)
        }
    }

    fn borrado(sena: &str, resumen: &str) -> Apunte {
        Apunte {
            borrado: true,
            ..ap(sena, resumen)
        }
    }

    fn base(pares: &[(&str, &str)]) -> BTreeMap<String, String> {
        pares
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn sin_base() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn lo_que_solo_tiene_el_otro_me_lo_traigo() {
        let p = plan(&[], &[ap("1t", "x")], &sin_base());
        assert_eq!(p, vec![Paso::Traer("1t".into())]);
    }

    #[test]
    fn lo_que_solo_tengo_yo_se_lo_mando() {
        let p = plan(&[ap("1a", "x")], &[], &sin_base());
        assert_eq!(p, vec![Paso::Mandar("1a".into())]);
    }

    #[test]
    fn lo_identico_no_mueve_nada() {
        let p = plan(&[ap("1a", "x")], &[ap("1a", "x")], &sin_base());
        assert!(p.is_empty(), "sincronizar dos veces seguidas no manda nada");
    }

    #[test]
    fn si_cambio_solo_uno_gana_ese_sin_preguntar() {
        let b = base(&[("1a", "viejo")]);
        assert_eq!(
            plan(&[ap("1a", "viejo")], &[ap("1a", "nuevo")], &b),
            vec![Paso::Traer("1a".into())]
        );
        assert_eq!(
            plan(&[ap("1a", "nuevo")], &[ap("1a", "viejo")], &b),
            vec![Paso::Mandar("1a".into())]
        );
    }

    #[test]
    fn si_cambiaron_los_dos_se_fusiona_sin_preguntar() {
        let p = plan(
            &[ap("1a", "mio")],
            &[ap("1a", "suyo")],
            &base(&[("1a", "viejo")]),
        );
        assert_eq!(p, vec![Paso::Fusionar("1a".into())]);
    }

    #[test]
    fn sin_haber_sincronizado_antes_dos_distintos_se_fusionan() {
        // Sin base no se sabe quien se movio: se junta todo, que perder algo
        // es peor que tener de mas.
        let p = plan(&[ap("1a", "mio")], &[ap("1a", "suyo")], &sin_base());
        assert_eq!(p, vec![Paso::Fusionar("1a".into())]);
    }

    #[test]
    fn borrar_deja_rastro_y_el_rastro_viaja() {
        // Si la marca no se mandara, el otro me devolveria el mensaje y lo
        // resucitaria.
        let p = plan(&[borrado("1a", "x")], &[], &sin_base());
        assert_eq!(p, vec![Paso::Mandar("1a".into())]);
    }

    #[test]
    fn lo_que_el_otro_borro_y_yo_nunca_tuve_no_se_trae() {
        let p = plan(&[], &[borrado("1t", "x")], &sin_base());
        assert!(p.is_empty());
    }

    #[test]
    fn borrado_en_uno_e_intacto_en_el_otro_se_borra_en_los_dos() {
        let b = base(&[("1a", "x")]);
        let p = plan(&[borrado("1a", "x")], &[ap("1a", "x")], &b);
        assert_eq!(p, vec![Paso::Mandar("1a".into())]);
        let al_reves = plan(&[ap("1a", "x")], &[borrado("1a", "x")], &b);
        assert_eq!(al_reves, vec![Paso::Traer("1a".into())]);
    }

    #[test]
    fn borrado_en_uno_y_cambiado_en_el_otro_se_queda_lo_cambiado() {
        // Borrar quita lo que uno vio: una mejora que el otro no vio
        // sobrevive (add-wins).
        let p = plan(
            &[borrado("1a", "x")],
            &[ap("1a", "mejorado")],
            &base(&[("1a", "x")]),
        );
        assert_eq!(p, vec![Paso::Traer("1a".into())]);
        // Y sin base con que saber si se toco, tambien se queda.
        assert_eq!(
            plan(&[ap("1a", "y")], &[borrado("1a", "x")], &sin_base()),
            vec![Paso::Mandar("1a".into())]
        );
    }

    #[test]
    fn una_marca_de_borrado_de_antes_de_los_codigos_se_reconoce_por_su_sena() {
        let vivo = Apunte {
            alias: Some("47a".into()),
            ..ap("K7Q2ABCDEF", "x")
        };
        let marca = Apunte {
            alias: Some("47a".into()),
            ..borrado(&format!("{MARCA_VIEJA}47a"), "borrado")
        };
        // Lo acordado antes iba por la sena.
        let p = plan(&[marca], &[vivo], &base(&[("47a", "x")]));
        assert_eq!(p, vec![Paso::Mandar("K7Q2ABCDEF".into())]);
    }

    #[test]
    fn una_marca_vieja_de_un_mensaje_que_aqui_sigue_vivo_se_tira() {
        // Si el mensaje con ese codigo sigue vivo en esta lista, la marca es
        // de una version anterior que ya se deshizo: no cuenta.
        let vivo = Apunte {
            alias: Some("47a".into()),
            ..ap("K7Q2ABCDEF", "x")
        };
        let marca = Apunte {
            alias: Some("47a".into()),
            ..borrado(&format!("{MARCA_VIEJA}47a"), "borrado")
        };
        let lista = con_marcas_viejas(&[vivo.clone(), marca.clone()], &[]);
        assert_eq!(lista, vec![vivo]);
        // Y una marca vieja que nadie reconoce se queda como esta.
        let sola = con_marcas_viejas(std::slice::from_ref(&marca), &[]);
        assert_eq!(sola, vec![marca]);
    }

    #[test]
    fn si_los_dos_lo_borraron_no_hay_nada_que_hacer() {
        let p = plan(&[borrado("1a", "x")], &[borrado("1a", "y")], &sin_base());
        assert!(p.is_empty());
    }

    #[test]
    fn el_chat_se_ordena_por_la_hora_de_creacion_y_no_por_cuando_llego() {
        let desordenados = [creado("9a", "x", 300), creado("2t", "x", 100)];
        assert_eq!(
            ordenar(&desordenados)
                .iter()
                .map(|a| a.sena.clone())
                .collect::<Vec<_>>(),
            vec!["2t", "9a"]
        );
    }

    #[test]
    fn a_la_misma_hora_ordena_la_sena_y_no_el_azar() {
        let a = creado("5t", "x", 100);
        let b = creado("5a", "x", 100);
        let senas = |l: &[Apunte]| {
            ordenar(l)
                .iter()
                .map(|a| a.sena.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(senas(&[a.clone(), b.clone()]), vec!["5a", "5t"]);
        assert_eq!(senas(&[b, a]), vec!["5a", "5t"]);
    }

    #[test]
    fn el_plan_sale_en_el_mismo_orden_en_los_dos_aparatos() {
        let mio = [ap("9a", "x"), ap("2t", "y")];
        let suyo = [ap("2t", "z")];
        let p = plan(&mio, &suyo, &sin_base());
        let senas: Vec<&str> = p.iter().map(Paso::sena).collect();
        assert_eq!(senas, vec!["2t", "9a"]);
    }
}
