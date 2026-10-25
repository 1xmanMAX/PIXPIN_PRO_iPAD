//! **Los dias**: que se ve en la pantalla principal (las ultimas 24 horas),
//! que dias tienen algo (los puntos del calendario) y la rejilla de un mes.
//!
//! Todas las cuentas reciben el `desfase` del huso de quien mira (ms a sumar
//! a UTC, `pixpin_shell::entorno::desfase_local_ms`): un momento de las 23:30
//! de Lima es de ese dia aunque en UTC ya sea el siguiente.

use std::collections::BTreeMap;

use crate::Momento;

pub const DIA_MS: i64 = 86_400_000;

/// Un dia del calendario, sin hora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dia {
    pub anio: i32,
    pub mes: u8,
    pub dia: u8,
}

impl Dia {
    /// El dia numero `n` contado desde el 1-ene-1970 (algoritmo de Hinnant,
    /// el mismo de `pixpin_proyecto::mini::Fecha`).
    pub fn de_numero(n: i64) -> Dia {
        let z = n + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let dia = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let anio = (yoe + era * 400 + i64::from(mes <= 2)) as i32;
        Dia { anio, mes, dia }
    }

    /// Cuantos dias van del 1-ene-1970 a este.
    pub fn numero(self) -> i64 {
        let y = i64::from(self.anio) - i64::from(self.mes <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.mes);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.dia) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// El dia de un instante UTC, para quien vive en `desfase`.
    pub fn de_instante(utc_ms: i64, desfase: i64) -> Dia {
        Dia::de_numero((utc_ms + desfase).div_euclid(DIA_MS))
    }

    /// Donde empieza (UTC) y donde acaba este dia para quien vive en
    /// `desfase`: `[inicio, fin)`.
    pub fn tramo(self, desfase: i64) -> (i64, i64) {
        let inicio = self.numero() * DIA_MS - desfase;
        (inicio, inicio + DIA_MS)
    }

    /// 0 = lunes … 6 = domingo (el 1-ene-1970 fue jueves).
    pub fn dia_de_la_semana(self) -> u8 {
        (self.numero() + 3).rem_euclid(7) as u8
    }

    pub fn siguiente(self) -> Dia {
        Dia::de_numero(self.numero() + 1)
    }

    /// `AAAA-MM-DD`: el nombre de los ficheros exportados.
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.anio, self.mes, self.dia)
    }
}

/// Un mes del calendario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mes {
    pub anio: i32,
    pub mes: u8,
}

impl Mes {
    pub fn de(d: Dia) -> Mes {
        Mes {
            anio: d.anio,
            mes: d.mes,
        }
    }

    pub fn anterior(self) -> Mes {
        if self.mes == 1 {
            Mes {
                anio: self.anio - 1,
                mes: 12,
            }
        } else {
            Mes {
                mes: self.mes - 1,
                ..self
            }
        }
    }

    pub fn siguiente(self) -> Mes {
        if self.mes == 12 {
            Mes {
                anio: self.anio + 1,
                mes: 1,
            }
        } else {
            Mes {
                mes: self.mes + 1,
                ..self
            }
        }
    }

    pub fn primero(self) -> Dia {
        Dia {
            anio: self.anio,
            mes: self.mes,
            dia: 1,
        }
    }

    pub fn dias(self) -> u8 {
        (self.siguiente().primero().numero() - self.primero().numero()) as u8
    }

    /// **La rejilla del mes**, semana a semana empezando en lunes: `None` en
    /// los huecos de antes del dia 1 y de despues del ultimo. Siempre filas
    /// enteras de siete.
    pub fn rejilla(self) -> Vec<Option<Dia>> {
        let hueco = self.primero().dia_de_la_semana() as usize;
        let mut v: Vec<Option<Dia>> = vec![None; hueco];
        let mut d = self.primero();
        for _ in 0..self.dias() {
            v.push(Some(d));
            d = d.siguiente();
        }
        while v.len() % 7 != 0 {
            v.push(None);
        }
        v
    }
}

/// **Lo que sale en la pantalla principal**: lo de las ultimas 24 horas,
/// del mas viejo al mas nuevo (lo nuevo abajo, junto a la caja de escribir,
/// como en un chat). Lo anterior no se borra: se ve por el calendario.
pub fn ultimas_24h(momentos: &[Momento], ahora: i64) -> Vec<&Momento> {
    entre(momentos, ahora - DIA_MS, i64::MAX)
}

/// Lo de un dia, en orden.
pub fn del_dia(momentos: &[Momento], dia: Dia, desfase: i64) -> Vec<&Momento> {
    let (a, b) = dia.tramo(desfase);
    entre(momentos, a, b)
}

fn entre(momentos: &[Momento], desde: i64, hasta: i64) -> Vec<&Momento> {
    let mut v: Vec<&Momento> = momentos
        .iter()
        .filter(|m| m.cuando >= desde && m.cuando < hasta)
        .collect();
    v.sort_by_key(|m| (m.cuando, m.id.clone()));
    v
}

/// Cuantos momentos tiene cada dia: los puntos del calendario.
pub fn dias_con(momentos: &[Momento], desfase: i64) -> BTreeMap<Dia, usize> {
    let mut m = BTreeMap::new();
    for x in momentos {
        *m.entry(Dia::de_instante(x.cuando, desfase)).or_insert(0) += 1;
    }
    m
}

/// `HH:MM` de un instante, en el huso de quien mira.
pub fn hora(utc_ms: i64, desfase: i64) -> String {
    let min = (utc_ms + desfase).rem_euclid(DIA_MS) / 60_000;
    format!("{:02}:{:02}", min / 60, min % 60)
}

const MESES_ES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];
const MESES_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const SEMANA_ES: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];
const SEMANA_EN: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// «octubre de 2026» / «October 2026».
pub fn nombre_del_mes(m: Mes, ingles: bool) -> String {
    let i = usize::from(m.mes.clamp(1, 12) - 1);
    if ingles {
        format!("{} {}", MESES_EN[i], m.anio)
    } else {
        format!("{} de {}", MESES_ES[i], m.anio)
    }
}

/// «Lunes 5 de octubre de 2026» / «Monday, October 5, 2026».
pub fn nombre_del_dia(d: Dia, ingles: bool) -> String {
    let s = usize::from(d.dia_de_la_semana());
    let i = usize::from(d.mes.clamp(1, 12) - 1);
    if ingles {
        format!("{}, {} {}, {}", SEMANA_EN[s], MESES_EN[i], d.dia, d.anio)
    } else {
        let mut n = SEMANA_ES[s].chars();
        let primera: String = n
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        format!(
            "{primera}{} {} de {} de {}",
            n.as_str(),
            d.dia,
            MESES_ES[i],
            d.anio
        )
    }
}

/// Solo el nombre del mes: «octubre» / «October» (el rotulo pequeno de la
/// pestana «Momentos», bajo el ano).
pub fn solo_el_mes(mes: u8, ingles: bool) -> &'static str {
    let i = usize::from(mes.clamp(1, 12) - 1);
    if ingles { MESES_EN[i] } else { MESES_ES[i] }
}

/// «lun» / «Mon»: el dia de la semana en corto (bajo el numero del dia en
/// «Momentos»).
pub fn dia_corto(d: Dia, ingles: bool) -> &'static str {
    const ES: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
    const EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let s = usize::from(d.dia_de_la_semana());
    if ingles { EN[s] } else { ES[s] }
}

/// «2026.10.5 17:11»: la fecha del detalle de un momento, como la escribe
/// WeChat en el ejemplo que mando el usuario (sin ceros delante).
pub fn fecha_con_hora(utc_ms: i64, desfase: i64) -> String {
    let d = Dia::de_instante(utc_ms, desfase);
    format!("{}.{}.{} {}", d.anio, d.mes, d.dia, hora(utc_ms, desfase))
}

/// Las iniciales de la semana, de lunes a domingo.
pub fn iniciales(ingles: bool) -> [&'static str; 7] {
    if ingles {
        ["M", "T", "W", "T", "F", "S", "S"]
    } else {
        ["L", "M", "X", "J", "V", "S", "D"]
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(id: &str, cuando: i64) -> Momento {
        Momento::nuevo(id.into(), cuando, id.into(), String::new())
    }

    const LIMA: i64 = -5 * 3_600_000;

    #[test]
    fn el_numero_del_dia_va_y_vuelve() {
        for n in [-1000, 0, 1, 59, 365, 20_366, 30_000] {
            assert_eq!(Dia::de_numero(n).numero(), n);
        }
        assert_eq!(
            Dia::de_numero(0),
            Dia {
                anio: 1970,
                mes: 1,
                dia: 1
            }
        );
    }

    #[test]
    fn el_5_de_octubre_de_2026_es_lunes() {
        let d = Dia {
            anio: 2026,
            mes: 10,
            dia: 5,
        };
        assert_eq!(d.dia_de_la_semana(), 0);
        assert_eq!(nombre_del_dia(d, false), "Lunes 5 de octubre de 2026");
        assert_eq!(nombre_del_dia(d, true), "Monday, October 5, 2026");
    }

    #[test]
    fn un_momento_de_noche_es_del_dia_de_quien_mira() {
        // 2026-10-06 03:30 UTC = 2026-10-05 22:30 en Lima.
        let d = Dia {
            anio: 2026,
            mes: 10,
            dia: 6,
        };
        let utc = d.numero() * DIA_MS + 3 * 3_600_000 + 30 * 60_000;
        assert_eq!(Dia::de_instante(utc, LIMA).dia, 5);
        assert_eq!(Dia::de_instante(utc, 0).dia, 6);
        assert_eq!(hora(utc, LIMA), "22:30");
    }

    #[test]
    fn las_ultimas_24_horas_dejan_fuera_lo_de_antes_y_van_en_orden() {
        let ahora = 100 * DIA_MS;
        let v = [
            m("b", ahora - 1_000),
            m("viejo", ahora - DIA_MS - 1),
            m("a", ahora - DIA_MS + 1),
        ];
        let ids: Vec<&str> = ultimas_24h(&v, ahora)
            .iter()
            .map(|x| x.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "b"]);
    }

    #[test]
    fn un_dia_coge_justo_su_tramo_local() {
        let d = Dia::de_numero(200);
        let (a, b) = d.tramo(LIMA);
        let v = [
            m("dentro", a),
            m("fin", b - 1),
            m("fuera", b),
            m("antes", a - 1),
        ];
        let ids: Vec<&str> = del_dia(&v, d, LIMA).iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["dentro", "fin"]);
    }

    #[test]
    fn los_dias_con_algo_se_cuentan() {
        let d = Dia::de_numero(200);
        let (a, _) = d.tramo(0);
        let v = [m("1", a), m("2", a + 10), m("3", a + DIA_MS)];
        let c = dias_con(&v, 0);
        assert_eq!(c.get(&d), Some(&2));
        assert_eq!(c.get(&d.siguiente()), Some(&1));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn la_rejilla_empieza_en_lunes_y_son_semanas_enteras() {
        // Octubre de 2026 empieza en jueves: tres huecos delante.
        let r = Mes {
            anio: 2026,
            mes: 10,
        }
        .rejilla();
        assert_eq!(r.len() % 7, 0);
        assert_eq!(&r[..3], &[None, None, None]);
        assert_eq!(r[3].map(|d| d.dia), Some(1));
        assert_eq!(r.iter().flatten().count(), 31);
    }

    #[test]
    fn la_fecha_del_detalle_va_sin_ceros_delante() {
        let d = Dia {
            anio: 2026,
            mes: 10,
            dia: 5,
        };
        let utc = d.numero() * DIA_MS + 17 * 3_600_000 + 11 * 60_000;
        assert_eq!(fecha_con_hora(utc, 0), "2026.10.5 17:11");
        // Caso negativo: el huso cuenta; en Lima aun son las 12:11.
        assert_ne!(fecha_con_hora(utc, LIMA), "2026.10.5 17:11");
        assert_eq!(dia_corto(d, false), "lun");
        assert_eq!(solo_el_mes(10, false), "octubre");
        assert_eq!(solo_el_mes(4, true), "April");
    }

    #[test]
    fn febrero_bisiesto_y_el_cambio_de_anio() {
        assert_eq!(Mes { anio: 2028, mes: 2 }.dias(), 29);
        assert_eq!(Mes { anio: 2026, mes: 2 }.dias(), 28);
        assert_eq!(
            Mes {
                anio: 2026,
                mes: 12
            }
            .siguiente(),
            Mes { anio: 2027, mes: 1 }
        );
        assert_eq!(
            Mes { anio: 2026, mes: 1 }.anterior(),
            Mes {
                anio: 2025,
                mes: 12
            }
        );
    }
}
