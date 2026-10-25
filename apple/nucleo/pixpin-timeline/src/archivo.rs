//! **El archivo** (5-oct-2026): como se ordena lo pasado en las pestanas
//! «Momentos» y «Estado», copiadas de «Mis publicaciones» de WeChat (el
//! ejemplo que mando el usuario).
//!
//! - Momentos: por ano (rotulo grande), mes y dia, **del mas nuevo al mas
//!   viejo**; dentro de un dia, en el orden en que pasaron.
//! - Estado: una tarjeta por mes con algo, del mas nuevo al mas viejo, y el
//!   mes actual siempre (vacio o no: es donde se esta). Sus dias van **al
//!   reves**, desde hoy o el ultimo dia del mes hasta el 1.

use std::collections::BTreeMap;

use crate::Momento;
use crate::dias::{Dia, Mes};

#[derive(Debug)]
pub struct DiaDelArchivo<'a> {
    pub dia: Dia,
    /// En el orden en que pasaron.
    pub momentos: Vec<&'a Momento>,
}

#[derive(Debug)]
pub struct MesDelArchivo<'a> {
    pub mes: Mes,
    /// Del dia mas nuevo al mas viejo.
    pub dias: Vec<DiaDelArchivo<'a>>,
}

#[derive(Debug)]
pub struct AnioDelArchivo<'a> {
    pub anio: i32,
    pub meses: Vec<MesDelArchivo<'a>>,
}

/// Todo, por ano, mes y dia, lo mas nuevo primero.
pub fn agrupar(momentos: &[Momento], desfase: i64) -> Vec<AnioDelArchivo<'_>> {
    let mut por_dia: BTreeMap<Dia, Vec<&Momento>> = BTreeMap::new();
    for m in momentos {
        por_dia
            .entry(Dia::de_instante(m.cuando, desfase))
            .or_default()
            .push(m);
    }
    let mut anios: Vec<AnioDelArchivo<'_>> = Vec::new();
    for (dia, mut v) in por_dia.into_iter().rev() {
        v.sort_by_key(|m| (m.cuando, m.id.clone()));
        if anios.last().is_none_or(|a| a.anio != dia.anio) {
            anios.push(AnioDelArchivo {
                anio: dia.anio,
                meses: Vec::new(),
            });
        }
        let Some(anio) = anios.last_mut() else {
            continue;
        };
        let mes = Mes::de(dia);
        if anio.meses.last().is_none_or(|m| m.mes != mes) {
            anio.meses.push(MesDelArchivo {
                mes,
                dias: Vec::new(),
            });
        }
        if let Some(m) = anio.meses.last_mut() {
            m.dias.push(DiaDelArchivo { dia, momentos: v });
        }
    }
    anios
}

/// Los momentos del archivo uno detras de otro, en el orden en que se ven:
/// el que recorren las flechas del detalle.
pub fn en_orden<'a>(anios: &[AnioDelArchivo<'a>]) -> Vec<&'a Momento> {
    anios
        .iter()
        .flat_map(|a| &a.meses)
        .flat_map(|m| &m.dias)
        .flat_map(|d| d.momentos.iter().copied())
        .collect()
}

/// Los meses de la pestana «Estado»: los que tienen algo, del mas nuevo al
/// mas viejo, mas el de `hoy` siempre. Lo que caiga despues de hoy (un reloj
/// mal puesto) no abre un mes del futuro.
pub fn meses_del_estado(momentos: &[Momento], desfase: i64, hoy: Dia) -> Vec<Mes> {
    let actual = Mes::de(hoy);
    let mut v: Vec<Mes> = momentos
        .iter()
        .map(|m| Mes::de(Dia::de_instante(m.cuando, desfase)))
        .filter(|m| *m <= actual)
        .collect();
    v.push(actual);
    v.sort();
    v.dedup();
    v.reverse();
    v
}

/// Los dias que ensena la tarjeta de un mes, **del mas reciente al 1**:
/// desde hoy si es el mes de hoy, desde el ultimo si ya paso. Un mes del
/// futuro no ensena ninguno.
pub fn dias_al_reves(mes: Mes, hoy: Dia) -> Vec<Dia> {
    let actual = Mes::de(hoy);
    if mes > actual {
        return Vec::new();
    }
    let hasta = if mes == actual { hoy.dia } else { mes.dias() };
    (1..=hasta)
        .rev()
        .map(|dia| Dia {
            anio: mes.anio,
            mes: mes.mes,
            dia,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::dias::DIA_MS;

    fn m(id: &str, dia: Dia, minuto: i64) -> Momento {
        let cuando = dia.numero() * DIA_MS + minuto * 60_000;
        Momento::nuevo(id.into(), cuando, id.into(), String::new())
    }

    fn d(anio: i32, mes: u8, dia: u8) -> Dia {
        Dia { anio, mes, dia }
    }

    #[test]
    fn por_ano_mes_y_dia_lo_nuevo_primero_y_dentro_del_dia_en_orden() {
        let v = [
            m("viejo", d(2025, 4, 20), 10),
            m("tarde", d(2026, 10, 5), 600),
            m("manana", d(2026, 10, 5), 60),
            m("septiembre", d(2026, 9, 30), 0),
        ];
        let a = agrupar(&v, 0);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].anio, 2026);
        assert_eq!(
            a[0].meses[0].mes,
            Mes {
                anio: 2026,
                mes: 10
            }
        );
        let dia = &a[0].meses[0].dias[0];
        let ids: Vec<&str> = dia.momentos.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["manana", "tarde"]);
        assert_eq!(a[0].meses[1].mes.mes, 9);
        assert_eq!(a[1].anio, 2025);
        let todos: Vec<&str> = en_orden(&a).iter().map(|x| x.id.as_str()).collect();
        assert_eq!(todos, ["manana", "tarde", "septiembre", "viejo"]);
    }

    #[test]
    fn caso_negativo_sin_momentos_no_hay_archivo() {
        assert!(agrupar(&[], 0).is_empty());
    }

    #[test]
    fn los_meses_del_estado_llevan_siempre_el_actual_y_nunca_el_futuro() {
        let hoy = d(2026, 10, 5);
        let v = [
            m("a", d(2026, 8, 1), 0),
            m("b", d(2026, 8, 2), 0),
            m("futuro", d(2026, 12, 1), 0),
        ];
        let meses = meses_del_estado(&v, 0, hoy);
        assert_eq!(
            meses,
            [
                Mes {
                    anio: 2026,
                    mes: 10
                },
                Mes { anio: 2026, mes: 8 }
            ]
        );
        // Sin nada, solo el actual.
        assert_eq!(
            meses_del_estado(&[], 0, hoy),
            [Mes {
                anio: 2026,
                mes: 10
            }]
        );
    }

    #[test]
    fn los_dias_van_al_reves_desde_hoy_o_desde_el_ultimo() {
        let hoy = d(2026, 10, 5);
        let v = dias_al_reves(
            Mes {
                anio: 2026,
                mes: 10,
            },
            hoy,
        );
        let n: Vec<u8> = v.iter().map(|x| x.dia).collect();
        assert_eq!(n, [5, 4, 3, 2, 1]);
        let feb = dias_al_reves(Mes { anio: 2026, mes: 2 }, hoy);
        assert_eq!(feb.len(), 28);
        assert_eq!(feb[0].dia, 28);
        assert_eq!(feb[27].dia, 1);
        // Caso negativo: un mes del futuro no ensena dias.
        assert!(
            dias_al_reves(
                Mes {
                    anio: 2026,
                    mes: 11
                },
                hoy
            )
            .is_empty()
        );
    }
}
