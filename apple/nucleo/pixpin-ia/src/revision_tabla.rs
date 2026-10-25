//! **Las cuentas de una tabla, comprobadas en Rust** (16-oct-2026, banco de
//! documentos: «revisa el Excel y marca los errores de calculo» salia una
//! vez de cada tres sin marcar nada, y el anotador ligero llego a marcar como
//! error una fila que estaba bien).
//!
//! Regla de la casa (`calculo.rs`): los calculos no los hace el modelo. Aqui
//! se buscan las dos relaciones de una tabla de obra o de notas:
//!
//! - **Productos por fila**: una columna que, en casi todas las filas, es el
//!   producto de otras dos (Parcial = Metrado x P. unit.). Las filas donde no
//!   cuadra son el error, con lo que deberia decir.
//! - **Totales**: una fila «Total» (o la ultima) cuyo valor es la suma de la
//!   columna de arriba; si no cuadra, el error, y cuanto daria con los
//!   productos corregidos.
//!
//! Lo que sale va dicho en palabras para el modelo (y para el anotador): el
//! modelo solo lo marca y lo explica.

use crate::tabla_escaneada::como_numero;

/// Un error encontrado: la fila y la columna (desde 0, en la tabla dada) y lo
/// que se le dice al modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct Descuadre {
    pub fila: usize,
    pub columna: usize,
    pub dice: f64,
    pub deberia: f64,
    pub texto: String,
}

fn numero(s: &str) -> Option<f64> {
    como_numero(s).and_then(|n| n.parse().ok())
}

fn cerca(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.005 * a.abs().max(b.abs()).max(1.0)
}

/// Un numero como se escribe en la tabla (sin decimales de mas).
pub fn bonito(x: f64) -> String {
    if (x - x.round()).abs() < 1e-6 {
        format!("{}", x.round() as i64)
    } else {
        format!("{:.2}", x)
    }
}

fn es_total(fila: &[String]) -> bool {
    fila.iter().any(|c| {
        let t = crate::acciones::normalizar(c.trim());
        t.starts_with("total") || t.starts_with("suma") || t == "subtotal"
    })
}

/// **Revisa** las cuentas de `filas` (la primera puede ser la cabecera:
/// sus nombres se usan al decirlo). Vacio si todo cuadra o no hay cuentas.
pub fn revisar(filas: &[Vec<String>]) -> Vec<Descuadre> {
    let columnas = filas.iter().map(Vec::len).max().unwrap_or(0);
    if filas.len() < 3 || columnas < 3 {
        return Vec::new();
    }
    let celda = |f: usize, c: usize| filas[f].get(c).map(String::as_str).unwrap_or("");
    let cabecera = (0..columnas).all(|c| numero(celda(0, c)).is_none());
    let nombre = |c: usize| {
        let n = if cabecera { celda(0, c).trim() } else { "" };
        if n.is_empty() {
            format!("la columna {}", (b'A' + c as u8) as char)
        } else {
            format!("«{n}»")
        }
    };
    let datos: Vec<usize> = (usize::from(cabecera)..filas.len())
        .filter(|&f| !es_total(&filas[f]))
        .collect();
    let mut errores = Vec::new();
    // Productos: k = i * j en casi todas las filas con los tres numeros.
    let mut corregido: Vec<Vec<Option<f64>>> = filas
        .iter()
        .map(|f| f.iter().map(|c| numero(c)).collect())
        .collect();
    let mut usada = vec![false; columnas];
    for k in 0..columnas {
        let mut mejor: Option<(usize, usize, usize, usize)> = None;
        for i in 0..columnas {
            for j in i + 1..columnas {
                if i == k || j == k {
                    continue;
                }
                let (mut con, mut bien) = (0, 0);
                for &f in &datos {
                    if let (Some(a), Some(b), Some(c)) = (
                        numero(celda(f, i)),
                        numero(celda(f, j)),
                        numero(celda(f, k)),
                    ) {
                        con += 1;
                        bien += usize::from(cerca(a * b, c));
                    }
                }
                if con >= 3 && bien * 10 >= con * 6 && mejor.is_none_or(|m| bien > m.2) {
                    mejor = Some((i, j, bien, con));
                }
            }
        }
        let Some((i, j, _, _)) = mejor else { continue };
        usada[k] = true;
        for &f in &datos {
            if let (Some(a), Some(b), Some(c)) = (
                numero(celda(f, i)),
                numero(celda(f, j)),
                numero(celda(f, k)),
            ) {
                if !cerca(a * b, c) {
                    let d = a * b;
                    corregido[f][k] = Some(d);
                    errores.push(Descuadre {
                        fila: f,
                        columna: k,
                        dice: c,
                        deberia: d,
                        texto: format!(
                            "fila {} ({}): {} x {} = {}, pero {} dice {}",
                            f + 1,
                            celda(f, 0).trim(),
                            bonito(a),
                            bonito(b),
                            bonito(d),
                            nombre(k),
                            bonito(c)
                        ),
                    });
                }
            }
        }
    }
    // Totales: la fila «Total» con la suma de su columna.
    for f in (0..filas.len()).filter(|&f| es_total(&filas[f])) {
        for c in 0..columnas {
            let Some(dice) = numero(celda(f, c)) else {
                continue;
            };
            let arriba: Vec<usize> = datos.iter().copied().filter(|&g| g < f).collect();
            let valores: Vec<f64> = arriba.iter().filter_map(|&g| numero(celda(g, c))).collect();
            if valores.len() < 2 {
                continue;
            }
            let suma: f64 = valores.iter().sum();
            let suma_bien: f64 = arriba.iter().filter_map(|&g| corregido[g][c]).sum();
            if !cerca(suma, dice) {
                errores.push(Descuadre {
                    fila: f,
                    columna: c,
                    dice,
                    deberia: suma,
                    texto: format!(
                        "fila {} ({}): la suma de {} es {}, pero dice {}",
                        f + 1,
                        celda(f, 0).trim(),
                        nombre(c),
                        bonito(suma),
                        bonito(dice)
                    ),
                });
            } else if usada[c] && !cerca(suma_bien, dice) {
                errores.push(Descuadre {
                    fila: f,
                    columna: c,
                    dice,
                    deberia: suma_bien,
                    texto: format!(
                        "fila {} ({}): suma bien lo que hay ({}), pero con los parciales corregidos el total seria {}",
                        f + 1,
                        celda(f, 0).trim(),
                        bonito(dice),
                        bonito(suma_bien)
                    ),
                });
            }
        }
    }
    errores
}

/// Lo revisado dicho para el modelo: una linea por error, o que todo cuadra.
/// `None` si la tabla no tiene cuentas que revisar.
pub fn para_el_modelo(filas: &[Vec<String>]) -> Option<String> {
    let columnas = filas.iter().map(Vec::len).max().unwrap_or(0);
    let numeros = filas
        .iter()
        .flatten()
        .filter(|c| numero(c).is_some())
        .count();
    if filas.len() < 3 || columnas < 3 || numeros < 6 {
        return None;
    }
    let e = revisar(filas);
    Some(if e.is_empty() {
        "Cuentas comprobadas por PixPin (productos por fila y totales): todo cuadra.".to_string()
    } else {
        format!(
            "Cuentas comprobadas por PixPin (exactas; no las rehagas): {}.",
            e.iter()
                .map(|d| d.texto.clone())
                .collect::<Vec<_>>()
                .join("; ")
        )
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn filas(v: &[&[&str]]) -> Vec<Vec<String>> {
        v.iter()
            .map(|f| f.iter().map(|c| c.to_string()).collect())
            .collect()
    }

    const METRADOS: &[&[&str]] = &[
        &[
            "Partida",
            "Unidad",
            "Metrado",
            "P. unit. (S/)",
            "Parcial (S/)",
        ],
        &["Muro de ladrillo KK", "m2", "180", "62.00", "11160.00"],
        &["Tarrajeo de muros", "m2", "120", "28.50", "3240.00"],
        &["Contrapiso e=4cm", "m2", "95", "34.00", "3230.00"],
        &["Pintura látex 2 manos", "m2", "300", "12.50", "3750.00"],
        &["Puerta contraplacada", "und", "8", "380.00", "3040.00"],
        &["Total", "", "", "", "24420.00"],
    ];

    #[test]
    fn el_parcial_mal_y_el_total_que_cambia_salen_con_lo_que_deberian_decir() {
        let e = revisar(&filas(METRADOS));
        assert_eq!(e.len(), 2, "{e:?}");
        assert_eq!((e[0].fila, e[0].columna), (2, 4));
        assert!((e[0].deberia - 3420.0).abs() < 1e-6);
        assert!(e[0].texto.contains("Tarrajeo") && e[0].texto.contains("3420"));
        assert!((e[1].deberia - 24600.0).abs() < 1e-6, "{e:?}");
        let t = para_el_modelo(&filas(METRADOS)).unwrap();
        assert!(t.contains("3420") && t.contains("24600"));
    }

    #[test]
    fn una_tabla_que_cuadra_no_da_errores_y_un_texto_no_se_revisa() {
        let mut bien = filas(METRADOS);
        bien[2][4] = "3420.00".into();
        bien[6][4] = "24600".into();
        assert!(revisar(&bien).is_empty());
        assert!(para_el_modelo(&bien).unwrap().contains("todo cuadra"));
        // Negativo: sin numeros no hay cuentas.
        assert!(
            para_el_modelo(&filas(&[
                &["a", "b", "c"],
                &["d", "e", "f"],
                &["g", "h", "i"]
            ]))
            .is_none()
        );
        // Un total mal sumado sin productos.
        let notas = filas(&[
            &["Item", "Costo", "x"],
            &["a", "100", "1"],
            &["b", "50", "1"],
            &["c", "25", "1"],
            &["Total", "180", "3"],
        ]);
        let e = revisar(&notas);
        assert!(
            e.iter()
                .any(|d| d.columna == 1 && (d.deberia - 175.0).abs() < 1e-6),
            "{e:?}"
        );
    }
}
