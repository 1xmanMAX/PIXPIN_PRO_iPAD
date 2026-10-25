//! Pruebas de los operadores y las funciones nuevas. Cada funcion lleva su caso
//! bueno, sus casos raros y los negativos (de menos o de mas argumentos, tipos
//! que no son, rangos vacios, division por cero, ciclos).

use super::*;

/// Una tabla escrita de un tiron.
fn tabla(celdas: &[(&str, &str)]) -> Tabla {
    let mut t = Tabla::default();
    for (clave, contenido) in celdas {
        t.poner(ref_de(clave).unwrap(), contenido);
    }
    t
}

/// Lo que sale en `Z99` al escribir ahi `=<formula>`, ya como texto.
fn f(formula: &str) -> String {
    fc(&[], formula)
}

fn fc(celdas: &[(&str, &str)], formula: &str) -> String {
    let mut t = tabla(celdas);
    t.poner(ref_de("Z99").unwrap(), &format!("={formula}"));
    evaluar(&t, ref_de("Z99").unwrap()).to_string()
}

fn en(t: &Tabla, celda: &str) -> Valor {
    evaluar(t, ref_de(celda).unwrap())
}

/// Los datos que usan muchas pruebas: A numeros con un texto en medio, B
/// numeros, C nombres, y una tabla de busqueda en H:J.
fn datos() -> Vec<(&'static str, &'static str)> {
    vec![
        ("A1", "10"),
        ("A2", "20"),
        ("A3", "30"),
        ("A4", "x"),
        ("A5", "40"),
        ("B1", "1"),
        ("B2", "2"),
        ("B3", "3"),
        ("B4", "4"),
        ("B5", "5"),
        ("C1", "Ana"),
        ("C2", "Anibal"),
        ("C3", "Beto"),
        ("D1", "20"),
        ("H1", "10"),
        ("H2", "20"),
        ("H3", "30"),
        ("H4", "40"),
        ("I1", "a"),
        ("I2", "b"),
        ("I3", "c"),
        ("I4", "d"),
        ("J1", "1"),
        ("J2", "2"),
        ("J3", "3"),
        ("J4", "4"),
        ("L1", "1"),
        ("M1", "2"),
        ("N1", "3"),
        ("O1", "4"),
        ("L2", "w"),
        ("M2", "x"),
        ("N2", "y"),
        ("O2", "z"),
        ("P1", "30"),
        ("P2", "20"),
        ("P3", "10"),
        ("Q3", "z"),
        ("R1", "1"),
        ("R2", "1"),
        ("R3", "2"),
        ("S1", "a"),
        ("S2", "b"),
        ("S3", "c"),
        ("F1", "=1/0"),
    ]
}

fn d(formula: &str) -> String {
    fc(&datos(), formula)
}

fn varias(casos: &[(&str, &str)], con: impl Fn(&str) -> String) {
    for (formula, esperado) in casos {
        assert_eq!(con(formula), *esperado, "={formula}");
    }
}

// ---- Operadores ---------------------------------------------------------

#[test]
fn los_operadores_siguen_la_prioridad_de_excel() {
    varias(
        &[
            ("1+2*3", "7"),
            ("(1+2)*3", "9"),
            ("2^3", "8"),
            ("2^3^2", "64"),
            // El signo se pega antes que la potencia, como en Excel.
            ("-2^2", "4"),
            ("2^-1", "0.5"),
            ("50%", "0.5"),
            ("200*10%", "20"),
            ("2*50%+1", "2"),
            ("1E3+1", "1001"),
            ("--3", "3"),
            ("1+2&3", "33"),
            ("1+1=2", "VERDADERO"),
        ],
        f,
    );
}

#[test]
fn concatenar_con_el_ampersand_y_texto_entre_comillas() {
    varias(
        &[
            ("\"a\"&\"b\"", "ab"),
            ("\"n\"&1.5", "n1.5"),
            ("\"a\"\"b\"", "a\"b"),
            ("1&2", "12"),
            ("\"x\"&VERDADERO", "xVERDADERO"),
            ("\"\"&\"\"", ""),
        ],
        f,
    );
    // Casos negativos: comilla sin cerrar; un error no se concatena.
    assert_eq!(f("\"abc"), "#¡VALOR!");
    assert_eq!(f("\"a\"&1/0"), "#¡DIV/0!");
}

#[test]
fn las_comparaciones_y_los_logicos_como_valor() {
    varias(
        &[
            ("1<2", "VERDADERO"),
            ("2<=1", "FALSO"),
            ("2>=2", "VERDADERO"),
            ("1<>1", "FALSO"),
            ("1=1.0", "VERDADERO"),
            ("\"A\"=\"a\"", "VERDADERO"),
            ("\"b\">\"a\"", "VERDADERO"),
            // Primero los numeros, luego el texto, luego los logicos.
            ("1<\"a\"", "VERDADERO"),
            ("\"a\"<VERDADERO", "VERDADERO"),
            ("VERDADERO", "VERDADERO"),
            ("FALSO", "FALSO"),
            ("TRUE", "VERDADERO"),
            ("false", "FALSO"),
            // 0.1+0.2 no es 0.3 en binario, pero en una hoja si.
            ("0.1+0.2=0.3", "VERDADERO"),
            ("VERDADERO+1", "2"),
        ],
        f,
    );
    // Una celda vacia vale cero, texto vacio o falso segun con que se compare.
    varias(
        &[
            ("B9=0", "VERDADERO"),
            ("B9=\"\"", "VERDADERO"),
            ("B9<1", "VERDADERO"),
            ("B9=FALSO", "VERDADERO"),
            ("B9=\"a\"", "FALSO"),
        ],
        f,
    );
    // Negativos: un error en cualquier lado sube.
    assert_eq!(f("1/0=1"), "#¡DIV/0!");
    assert_eq!(f("1=1/0"), "#¡DIV/0!");
}

#[test]
fn las_cuentas_con_texto_potencia_y_desbordes() {
    assert_eq!(f("\"3\"+4"), "7", "un numero entre comillas se lee");
    assert_eq!(f("1+\"abc\""), "#¡VALOR!");
    assert_eq!(f("0^0"), "#¡NUM!");
    assert_eq!(f("(-8)^0.5"), "#¡NUM!");
    assert_eq!(f("0^-1"), "#¡DIV/0!");
    assert_eq!(f("10^400"), "#¡NUM!", "no cabe en un numero");
    assert_eq!(f("1E308*10"), "#¡NUM!");
    assert_eq!(f("(-8)^(1/3)"), "#¡NUM!");
    assert_eq!(f("(-2)^3"), "-8");
}

// ---- Logicas ------------------------------------------------------------

#[test]
fn si_elige_una_rama_y_calla_la_otra() {
    varias(
        &[
            ("SI(1>0;\"si\";\"no\")", "si"),
            ("SI(1<0;\"si\";\"no\")", "no"),
            ("IF(1,2,3)", "2"),
            ("SI(FALSO;1)", "FALSO"),
            ("SI(0;1;2)", "2"),
            ("SI(\"VERDADERO\";1;2)", "1"),
            // La rama que no toca ni se calcula: una division por cero ahi
            // no es un error.
            ("SI(VERDADERO;1;1/0)", "1"),
            ("SI(FALSO;1/0;2)", "2"),
            ("SI(B9;1;2)", "2"),
        ],
        f,
    );
    // Negativos.
    varias(
        &[
            ("SI(1)", "#¡VALOR!"),
            ("SI(1;2;3;4)", "#¡VALOR!"),
            ("SI()", "#¡VALOR!"),
            ("SI(\"x\";1;2)", "#¡VALOR!"),
            ("SI(1/0;1;2)", "#¡DIV/0!"),
            ("SI(VERDADERO;1/0;2)", "#¡DIV/0!"),
        ],
        f,
    );
}

#[test]
fn y_o_no() {
    varias(
        &[
            ("Y(1;1)", "VERDADERO"),
            ("Y(1;0)", "FALSO"),
            ("AND(1>0,2>1,VERDADERO)", "VERDADERO"),
            ("O(0;0;1)", "VERDADERO"),
            ("OR(0,0)", "FALSO"),
            ("NO(0)", "VERDADERO"),
            ("NOT(VERDADERO)", "FALSO"),
            ("Y(A1:A3)", "VERDADERO"),
            ("O(B9:B10;0)", "FALSO"),
            // Dentro de un rango el texto se ignora; suelto, no se entiende.
            ("Y(A1:A5)", "VERDADERO"),
            ("Y(\"hola\")", "#¡VALOR!"),
            ("Y(1;\"verdadero\")", "VERDADERO"),
            // Sin ningun logico no hay respuesta.
            ("Y(C1:C3)", "#¡VALOR!"),
        ],
        d,
    );
    // Negativos: sin argumentos, de mas, o con un error.
    assert_eq!(f("Y()"), "#¡VALOR!");
    assert_eq!(f("NO()"), "#¡VALOR!");
    assert_eq!(f("NO(1;2)"), "#¡VALOR!");
    assert_eq!(f("Y(1;1/0)"), "#¡DIV/0!");
    assert_eq!(f("NO(\"x\")"), "#¡VALOR!");
}

#[test]
fn si_error_y_si_nd() {
    varias(
        &[
            ("SI.ERROR(1/0;\"x\")", "x"),
            ("IFERROR(5;\"x\")", "5"),
            ("SI.ERROR(F1;0)", "0"),
            ("SI.ERROR(SUMA(F1:F2);-1)", "-1"),
            ("SI.ERROR(\"a\"+1;\"malo\")", "malo"),
            ("SI.ND(BUSCARV(9;H1:J4;2;0);\"no\")", "no"),
            // SI.ND solo tapa #N/A: lo demas sigue siendo error.
            ("SI.ND(1/0;\"no\")", "#¡DIV/0!"),
        ],
        d,
    );
    // Negativos.
    assert_eq!(f("SI.ERROR(1)"), "#¡VALOR!");
    assert_eq!(f("SI.ERROR(1;2;3)"), "#¡VALOR!");
    // Un ciclo no se tapa: es la hoja mal escrita, no un dato malo.
    let t = tabla(&[("A1", "=SI.ERROR(A1;0)")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
}

#[test]
fn si_conjunto_toma_el_primero_que_se_cumple() {
    varias(
        &[
            ("SI.CONJUNTO(FALSO;1;VERDADERO;2;VERDADERO;3)", "2"),
            ("IFS(1>2,\"a\",2>1,\"b\")", "b"),
            // Ninguno se cumple: #N/A.
            ("SI.CONJUNTO(FALSO;1;FALSO;2)", "#N/A"),
            // Un argumento sin pareja, o ninguno.
            ("SI.CONJUNTO(VERDADERO;1;FALSO)", "#¡VALOR!"),
            ("SI.CONJUNTO()", "#¡VALOR!"),
            ("SI.CONJUNTO(VERDADERO)", "#¡VALOR!"),
            ("SI.CONJUNTO(\"x\";1)", "#¡VALOR!"),
            // Lo que va despues del que se cumple no se calcula.
            ("SI.CONJUNTO(VERDADERO;1;1/0;2)", "1"),
        ],
        f,
    );
}

// ---- Matematicas --------------------------------------------------------

#[test]
fn redondear_como_excel() {
    varias(
        &[
            // 2.675 es 2.67499999... en binario; Excel dice 2.68.
            ("REDONDEAR(2.675;2)", "2.68"),
            ("ROUND(1.005;2)", "1.01"),
            ("REDONDEAR(2.5;0)", "3"),
            ("REDONDEAR(-2.5;0)", "-3"),
            ("REDONDEAR(2.4;0)", "2"),
            ("REDONDEAR(1234.567;-2)", "1200"),
            ("REDONDEAR(1250;-2)", "1300"),
            ("REDONDEAR(3.14159;3)", "3.142"),
            ("REDONDEAR.MAS(2.1;0)", "3"),
            ("ROUNDUP(-2.1;0)", "-3"),
            ("REDONDEAR.MAS(2.123;2)", "2.13"),
            ("REDONDEAR.MAS(3;0)", "3"),
            ("REDONDEAR.MENOS(2.9;0)", "2"),
            ("ROUNDDOWN(-2.9;0)", "-2"),
            ("REDONDEAR.MENOS(2.999;2)", "2.99"),
            ("REDONDEAR.MENOS(1999;-3)", "1000"),
            ("REDONDEAR(0;2)", "0"),
            ("ENTERO(2.9)", "2"),
            ("INT(-1.5)", "-2"),
        ],
        f,
    );
    // Negativos: de menos o de mas, y tipos.
    varias(
        &[
            ("REDONDEAR(1)", "#¡VALOR!"),
            ("REDONDEAR(1;2;3)", "#¡VALOR!"),
            ("REDONDEAR(\"x\";1)", "#¡VALOR!"),
            ("REDONDEAR(1/0;1)", "#¡DIV/0!"),
            ("ENTERO()", "#¡VALOR!"),
            ("ENTERO(\"a\")", "#¡VALOR!"),
        ],
        f,
    );
}

#[test]
fn abs_residuo_potencia_raiz_pi() {
    varias(
        &[
            ("ABS(-3)", "3"),
            ("ABS(3)", "3"),
            ("RESIDUO(10;3)", "1"),
            // El resultado lleva el signo del divisor.
            ("RESIDUO(-3;2)", "1"),
            ("MOD(3;-2)", "-1"),
            ("RESIDUO(5.5;2)", "1.5"),
            ("POTENCIA(2;10)", "1024"),
            ("POWER(9;0.5)", "3"),
            ("RAIZ(16)", "4"),
            ("SQRT(2)", "1.414213562"),
            ("RAIZ(0)", "0"),
            ("PI()", "3.141592654"),
            ("REDONDEAR(PI();2)", "3.14"),
        ],
        f,
    );
    varias(
        &[
            ("RESIDUO(1;0)", "#¡DIV/0!"),
            ("RAIZ(-1)", "#¡NUM!"),
            ("POTENCIA(0;0)", "#¡NUM!"),
            ("POTENCIA(-8;0.5)", "#¡NUM!"),
            ("POTENCIA(0;-1)", "#¡DIV/0!"),
            ("ABS()", "#¡VALOR!"),
            ("ABS(1;2)", "#¡VALOR!"),
            ("ABS(\"x\")", "#¡VALOR!"),
            ("PI(1)", "#¡VALOR!"),
            ("RAIZ()", "#¡VALOR!"),
            ("RESIDUO(1)", "#¡VALOR!"),
            ("POTENCIA(2)", "#¡VALOR!"),
        ],
        f,
    );
}

#[test]
fn producto_y_mediana() {
    varias(
        &[
            ("PRODUCTO(2;3;4)", "24"),
            ("PRODUCT(A1:A3)", "6000"),
            // El texto de un rango no multiplica.
            ("PRODUCTO(A1:A5)", "240000"),
            ("MEDIANA(1;3;2)", "2"),
            ("MEDIAN(1;2;3;4)", "2.5"),
            ("MEDIANA(A1:A5)", "25"),
            ("MEDIANA(5)", "5"),
            // Un error dentro del rango sube.
            ("MEDIANA(F1:F2)", "#¡DIV/0!"),
        ],
        d,
    );
    // Negativos: sin numeros no hay mediana; el producto vacio es cero.
    assert_eq!(f("MEDIANA(Q1:Q9)"), "#¡NUM!");
    assert_eq!(f("PRODUCTO(Q1:Q9)"), "0");
    assert_eq!(f("PRODUCTO()"), "#¡VALOR!");
    assert_eq!(f("MEDIANA()"), "#¡VALOR!");
}

#[test]
fn aleatorio_entre_queda_entre_los_limites() {
    // El motor tolera volatiles: cada calculo da un valor nuevo y, dentro de
    // un calculo, cada celda se calcula una vez.
    let mut t = Tabla::default();
    for fila in 1..=60 {
        t.poner(
            ref_de(&format!("A{fila}")).unwrap(),
            "=ALEATORIO.ENTRE(1;6)",
        );
        t.poner(ref_de(&format!("B{fila}")).unwrap(), "=ALEATORIO()");
    }
    let v = evaluar_todo(&t);
    let mut vistos = std::collections::BTreeSet::new();
    for fila in 1..=60 {
        match &v[&format!("A{fila}")] {
            Valor::Numero(n) => {
                assert!((1.0..=6.0).contains(n) && n.fract() == 0.0, "{n}");
                vistos.insert(*n as i32);
            }
            otro => panic!("{otro:?}"),
        }
        match &v[&format!("B{fila}")] {
            Valor::Numero(n) => assert!((0.0..1.0).contains(n), "{n}"),
            otro => panic!("{otro:?}"),
        }
    }
    assert!(vistos.len() >= 4, "sesenta tiradas dieron solo {vistos:?}");
    // Negativos.
    assert_eq!(f("ALEATORIO.ENTRE(5;5)"), "5");
    assert_eq!(f("ALEATORIO.ENTRE(6;1)"), "#¡NUM!");
    assert_eq!(f("ALEATORIO.ENTRE(1)"), "#¡VALOR!");
    assert_eq!(f("ALEATORIO(1)"), "#¡VALOR!");
    assert_eq!(
        f("RANDBETWEEN(2.2;2.8)"),
        "#¡NUM!",
        "no hay enteros entre 3 y 2"
    );
}

// ---- Recuentos y criterios ---------------------------------------------

#[test]
fn contara_y_contar_blanco() {
    varias(
        &[
            ("CONTARA(A1:A5)", "5"),
            ("COUNTA(A1:A100)", "5"),
            ("CONTARA(A1:A2;\"x\";9)", "4"),
            // Un error es algo escrito: se cuenta, no sube.
            ("CONTARA(F1:F2)", "1"),
            ("CONTAR.BLANCO(A1:A10)", "5"),
            ("COUNTBLANK(A1:A5)", "0"),
            ("CONTAR.BLANCO(Q1:Q9)", "8"),
            ("CONTAR.BLANCO(A1)", "0"),
            ("CONTAR.BLANCO(A9)", "1"),
        ],
        d,
    );
    // Un texto vacio devuelto por una formula tambien es blanco.
    assert_eq!(
        fc(&[("A1", "=\"\""), ("A2", "1")], "CONTAR.BLANCO(A1:A3)"),
        "2"
    );
    assert_eq!(fc(&[("A1", "=\"\""), ("A2", "1")], "CONTARA(A1:A3)"), "1");
    // Negativos.
    assert_eq!(f("CONTARA()"), "#¡VALOR!");
    assert_eq!(f("CONTAR.BLANCO()"), "#¡VALOR!");
    assert_eq!(f("CONTAR.BLANCO(A1;A2)"), "#¡VALOR!");
}

#[test]
fn contar_si_y_sumar_si() {
    varias(
        &[
            ("SUMAR.SI(A1:A5;\">15\")", "90"),
            ("SUMIF(A1:A5,\">15\")", "90"),
            ("SUMAR.SI(A1:A5;\">15\";B1:B5)", "10"),
            ("SUMAR.SI(A1:A5;\"x\";B1:B5)", "4"),
            ("SUMAR.SI(A1:A5;20)", "20"),
            ("CONTAR.SI(A1:A5;\">15\")", "3"),
            ("COUNTIF(A1:A5,\"<>10\")", "4"),
            ("CONTAR.SI(A1:A5;10)", "1"),
            ("CONTAR.SI(A1:A5;\"=x\")", "1"),
            ("CONTAR.SI(A1:A5;\"X\")", "1"),
            ("CONTAR.SI(A1:A5;D1)", "1"),
            ("CONTAR.SI(A1:A5;\">=20\")", "3"),
            ("CONTAR.SI(A1:A5;\"<=20\")", "2"),
            ("CONTAR.SI(A1:A5;\"<30\")", "2"),
            // Los comodines; las vacias no casan con «*».
            ("CONTAR.SI(C1:C4;\"An*\")", "2"),
            ("CONTAR.SI(C1:C4;\"?eto\")", "1"),
            ("CONTAR.SI(C1:C4;\"*\")", "3"),
            ("CONTAR.SI(C1:C4;\"*i*\")", "1"),
            ("CONTAR.SI(C1:C4;\"ana\")", "1"),
            ("CONTAR.SI(C1:C4;\"\")", "1"),
            ("CONTAR.SI(C1:C4;\"<>\")", "3"),
            // Los huecos de un rango largo se cuentan sin recorrerlos.
            ("CONTAR.SI(A1:A100;\"\")", "95"),
            ("CONTAR.SI(A1:A1000000;\"\")", "999995"),
            ("CONTAR.SI(A1:A100;\"<>\")", "5"),
            ("SUMAR.SI(A1:A1000000;\">0\")", "100"),
            // Nada que cumpla.
            ("CONTAR.SI(A1:A5;\">100\")", "0"),
            ("SUMAR.SI(A1:A5;\">100\")", "0"),
        ],
        d,
    );
    // Negativos: de menos o de mas, un rango que no lo es, un error de criterio.
    varias(
        &[
            ("CONTAR.SI(A1:A5)", "#¡VALOR!"),
            ("CONTAR.SI(A1:A5;1;2)", "#¡VALOR!"),
            ("CONTAR.SI(5;1)", "#¡VALOR!"),
            ("SUMAR.SI(A1:A5)", "#¡VALOR!"),
            ("SUMAR.SI(A1:A5;1;B1:B5;1)", "#¡VALOR!"),
            ("CONTAR.SI(A1:A5;1/0)", "#¡DIV/0!"),
            // Un error dentro del rango de suma que cumple el criterio sube.
            ("SUMAR.SI(A1:A5;\">0\";F1:F5)", "#¡DIV/0!"),
            // Un error en el rango de criterios no cumple nada.
            ("CONTAR.SI(F1:F2;\">0\")", "0"),
        ],
        d,
    );
}

#[test]
fn contar_si_conjunto_sumar_si_conjunto_y_promedio_si() {
    varias(
        &[
            ("SUMAR.SI.CONJUNTO(B1:B5;A1:A5;\">15\";B1:B5;\"<5\")", "5"),
            ("SUMIFS(B1:B5,A1:A5,\">15\")", "10"),
            ("CONTAR.SI.CONJUNTO(A1:A5;\">=10\";B1:B5;\"<=2\")", "2"),
            ("COUNTIFS(A1:A5,\">=10\")", "4"),
            ("CONTAR.SI.CONJUNTO(A1:A5;\">100\";B1:B5;\"<=2\")", "0"),
            ("PROMEDIO.SI(A1:A5;\">15\")", "30"),
            ("AVERAGEIF(A1:A5,\">15\",B1:B5)", "3.333333333"),
            ("PROMEDIO.SI(A1:A5;\">15\";B1:B5)", "3.333333333"),
            // Los huecos de fuera de lo escrito cuentan si todos los criterios
            // aceptan lo vacio.
            ("CONTAR.SI.CONJUNTO(A1:A100;\"\";B1:B100;\"\")", "95"),
            ("CONTAR.SI.CONJUNTO(A1:A100;\"\";B1:B100;\">0\")", "0"),
        ],
        d,
    );
    varias(
        &[
            // Rangos de distinto tamano.
            ("CONTAR.SI.CONJUNTO(A1:A5;\">1\";B1:B4;\">1\")", "#¡VALOR!"),
            ("SUMAR.SI.CONJUNTO(B1:B4;A1:A5;\">1\")", "#¡VALOR!"),
            // Criterio sin pareja, o ninguno.
            ("CONTAR.SI.CONJUNTO(A1:A5)", "#¡VALOR!"),
            ("CONTAR.SI.CONJUNTO(A1:A5;\">1\";B1:B5)", "#¡VALOR!"),
            ("SUMAR.SI.CONJUNTO(B1:B5)", "#¡VALOR!"),
            ("SUMAR.SI.CONJUNTO(B1:B5;A1:A5)", "#¡VALOR!"),
            // Nada que promediar.
            ("PROMEDIO.SI(A1:A5;\">100\")", "#¡DIV/0!"),
            ("PROMEDIO.SI(A1:A5;\"x\")", "#¡DIV/0!"),
            ("PROMEDIO.SI(A1:A5)", "#¡VALOR!"),
        ],
        d,
    );
}

#[test]
fn un_ciclo_dentro_de_un_criterio_no_se_calla() {
    let t = tabla(&[("A1", "=CONTAR.SI(A1:A3;\">0\")"), ("A2", "5"), ("A3", "6")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
    let t = tabla(&[("A1", "=CONTARA(A1:A3)"), ("A2", "5")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
    let t = tabla(&[("A1", "=CONTAR.BLANCO(A1:A3)"), ("A2", "5")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
}

// ---- Busqueda -----------------------------------------------------------

#[test]
fn buscarv_exacta_y_aproximada() {
    varias(
        &[
            ("BUSCARV(20;H1:J4;2;FALSO)", "b"),
            ("VLOOKUP(20,H1:J4,2,0)", "b"),
            ("BUSCARV(40;H1:J4;3;0)", "4"),
            ("BUSCARV(25;H1:J4;2;VERDADERO)", "b"),
            // Sin cuarto argumento es aproximada, como en Excel.
            ("BUSCARV(25;H1:J4;2)", "b"),
            ("BUSCARV(99;H1:J4;2)", "d"),
            ("BUSCARV(10;H1:J4;2)", "a"),
            ("BUSCARV(1;H1:J4;1;1)", "#N/A"),
            ("BUSCARV(5;H1:J4;2)", "#N/A"),
            ("BUSCARV(5;H1:J4;2;0)", "#N/A"),
            ("BUSCARV(25;H1:J4;2;0)", "#N/A"),
            // Texto: sin mayusculas, y con comodines si es exacta.
            ("BUSCARV(\"B\";I1:J4;2;0)", "2"),
            ("BUSCARV(\"c*\";I1:J4;2;0)", "3"),
            // En la aproximada el «*» es un asterisco mas, como en Excel: la
            // «c» queda por debajo de «c*».
            ("BUSCARV(\"c*\";I1:J4;2;1)", "3"),
            ("BUSCARV(\"zz\";I1:J4;2)", "4"),
            // Un numero no es el texto «20» ni al reves.
            ("BUSCARV(\"20\";H1:J4;2;0)", "#N/A"),
            ("BUSCARV(20;A1:B5;2;0)", "2"),
            ("SI.ERROR(BUSCARV(5;H1:J4;2;0);\"no\")", "no"),
            ("ESND(BUSCARV(5;H1:J4;2;0))", "VERDADERO"),
        ],
        d,
    );
    // Negativos: columna fuera, cero, negativa, de menos o de mas argumentos.
    varias(
        &[
            ("BUSCARV(20;H1:J4;4;0)", "#¡REF!"),
            ("BUSCARV(20;H1:J4;0;0)", "#¡VALOR!"),
            ("BUSCARV(20;H1:J4;-1;0)", "#¡VALOR!"),
            ("BUSCARV(20;H1:J4)", "#¡VALOR!"),
            ("BUSCARV(20;H1:J4;2;0;1)", "#¡VALOR!"),
            ("BUSCARV(20;H1:J4;2;\"x\")", "#¡VALOR!"),
            ("BUSCARV(20;H1:J4;\"x\";0)", "#¡VALOR!"),
            ("BUSCARV(1/0;H1:J4;2;0)", "#¡DIV/0!"),
            // Rango vacio.
            ("BUSCARV(1;X1:Y5;2;0)", "#N/A"),
            ("BUSCARV(1;X1:Y5;2)", "#N/A"),
        ],
        d,
    );
}

#[test]
fn buscarv_con_columna_desordenada_para_en_el_primero_que_se_pasa() {
    let t = [
        ("A1", "5"),
        ("A2", "50"),
        ("A3", "7"),
        ("B1", "a"),
        ("B2", "b"),
        ("B3", "c"),
    ];
    // Con 8: el 5 vale, el 50 se pasa y se para (el 7, detras, no cuenta).
    assert_eq!(fc(&t, "BUSCARV(8;A1:B3;2;1)"), "a");
}

#[test]
fn buscarh_indice_y_coincidir() {
    varias(
        &[
            ("BUSCARH(3;L1:O2;2;0)", "y"),
            ("HLOOKUP(3,L1:O2,2,FALSO)", "y"),
            ("BUSCARH(9;L1:O2;2)", "z"),
            ("BUSCARH(2.5;L1:O2;1)", "2"),
            ("BUSCARH(9;L1:O2;1;0)", "#N/A"),
            ("BUSCARH(3;L1:O2;3;0)", "#¡REF!"),
            ("BUSCARH(3;L1:O2;0;0)", "#¡VALOR!"),
            ("BUSCARH(3;L1:O2)", "#¡VALOR!"),
            ("INDICE(H1:J4;2;3)", "2"),
            ("INDEX(H1:J4,4,2)", "d"),
            ("INDICE(H1:H4;3)", "30"),
            ("INDICE(L1:O1;3)", "3"),
            ("INDICE(H1:H4;3;1)", "30"),
            ("INDICE(H1:J4;5;1)", "#¡REF!"),
            ("INDICE(H1:J4;1;4)", "#¡REF!"),
            ("INDICE(H1:J4;2)", "#¡VALOR!"),
            ("INDICE(H1:J4;0;1)", "#¡VALOR!"),
            ("INDICE(H1:J4;-1;1)", "#¡VALOR!"),
            ("INDICE(H1:J4)", "#¡VALOR!"),
            // Una celda de la matriz que esta vacia da vacio, no error.
            ("INDICE(Q1:Q5;1)", ""),
            ("COINCIDIR(30;H1:H4;0)", "3"),
            ("MATCH(30,H1:H4,0)", "3"),
            ("COINCIDIR(35;H1:H4;1)", "3"),
            ("COINCIDIR(35;H1:H4)", "3"),
            ("COINCIDIR(25;P1:P3;-1)", "1"),
            ("COINCIDIR(\"C\";I1:I4;0)", "3"),
            ("COINCIDIR(\"c?\";I1:I4;0)", "#N/A"),
            ("COINCIDIR(\"?\";I1:I4;0)", "1"),
            ("COINCIDIR(3;L1:O1;0)", "3"),
            // Los huecos cuentan para la posicion.
            ("COINCIDIR(\"z\";Q1:Q5;0)", "3"),
            ("COINCIDIR(35;H1:H4;0)", "#N/A"),
            ("COINCIDIR(5;H1:H4;1)", "#N/A"),
            // Solo una fila o una columna.
            ("COINCIDIR(1;H1:J4;0)", "#N/A"),
            ("COINCIDIR(1;H1:H4;0;1)", "#¡VALOR!"),
            ("COINCIDIR(1)", "#¡VALOR!"),
        ],
        d,
    );
}

#[test]
fn buscarx_basica() {
    varias(
        &[
            ("BUSCARX(20;H1:H4;I1:I4)", "b"),
            ("XLOOKUP(20,H1:H4,I1:I4)", "b"),
            ("BUSCARX(25;H1:H4;I1:I4)", "#N/A"),
            ("BUSCARX(25;H1:H4;I1:I4;\"nada\")", "nada"),
            ("BUSCARX(20;H1:H4;I1:I4;\"nada\")", "b"),
            // Exacto o el menor siguiente / el mayor siguiente.
            ("BUSCARX(25;H1:H4;I1:I4;\"-\";-1)", "b"),
            ("BUSCARX(25;H1:H4;I1:I4;\"-\";1)", "c"),
            ("BUSCARX(99;H1:H4;I1:I4;\"-\";1)", "-"),
            ("BUSCARX(1;H1:H4;I1:I4;\"-\";-1)", "-"),
            // Comodines solo con el modo 2.
            ("BUSCARX(\"c*\";I1:I4;H1:H4;\"-\";2)", "30"),
            ("BUSCARX(\"c*\";I1:I4;H1:H4;\"-\")", "-"),
            // De ultimo a primero, con repetidos.
            ("BUSCARX(1;R1:R3;S1:S3)", "a"),
            ("BUSCARX(1;R1:R3;S1:S3;\"x\";0;-1)", "b"),
            // En horizontal.
            ("BUSCARX(3;L1:O1;L2:O2)", "y"),
            // Buscar en una fila y devolver de una columna del mismo largo.
            ("BUSCARX(3;L1:O1;H1:H4)", "30"),
        ],
        d,
    );
    varias(
        &[
            // Largos distintos, tablas enteras, modos que no existen.
            ("BUSCARX(20;H1:H4;I1:I3)", "#¡VALOR!"),
            ("BUSCARX(20;H1:H4;I1:J4)", "#¡VALOR!"),
            ("BUSCARX(20;H1:J4;I1:I4)", "#N/A"),
            ("BUSCARX(20;H1:H4;I1:I4;\"-\";7)", "#¡VALOR!"),
            ("BUSCARX(20;H1:H4;I1:I4;\"-\";0;0)", "#¡VALOR!"),
            ("BUSCARX(20;H1:H4)", "#¡VALOR!"),
            ("BUSCARX(20;H1:H4;I1:I4;1;2;3;4)", "#¡VALOR!"),
            ("BUSCARX(1/0;H1:H4;I1:I4)", "#¡DIV/0!"),
        ],
        d,
    );
}

#[test]
fn un_ciclo_a_traves_de_buscarv_se_dice() {
    // La primera columna del rango incluye la propia celda.
    let t = tabla(&[("A1", "=BUSCARV(1;A1:B2;2;0)"), ("A2", "1"), ("B2", "5")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
    // La celda que devuelve vuelve a la formula: A1 -> C2 -> A1.
    let t = tabla(&[("A1", "=BUSCARV(2;B1:C3;2;0)"), ("B2", "2"), ("C2", "=A1")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
    // Y por INDICE y COINCIDIR.
    let t = tabla(&[("A1", "=INDICE(A1:A3;1)"), ("A2", "5")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
    let t = tabla(&[("A1", "=COINCIDIR(5;A1:A3;0)"), ("A2", "5")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
    // El ciclo no envenena a quien no esta dentro.
    let t = tabla(&[
        ("A1", "=BUSCARV(1;A1:B2;2;0)"),
        ("A2", "1"),
        ("B2", "5"),
        ("D1", "1"),
        ("E1", "7"),
        ("F1", "=BUSCARV(1;D1:E1;2;0)"),
    ]);
    assert_eq!(en(&t, "F1"), Valor::Numero(7.0));
}

// ---- Texto --------------------------------------------------------------

#[test]
fn concatenar_concat_y_unir_cadenas() {
    varias(
        &[
            ("CONCATENAR(\"a\";1;\"b\")", "a1b"),
            ("CONCATENATE(\"x\",\"y\")", "xy"),
            ("CONCATENAR(C1;\"-\";C3)", "Ana-Beto"),
            ("CONCAT(C1:C3)", "AnaAnibalBeto"),
            ("CONCAT(\"a\";C1:C2;1)", "aAnaAnibal1"),
            ("UNIRCADENAS(\"-\";VERDADERO;\"a\";\"\";\"b\")", "a-b"),
            ("TEXTJOIN(\"-\",FALSO,\"a\",\"\",\"b\")", "a--b"),
            ("UNIRCADENAS(\", \";VERDADERO;C1:C3)", "Ana, Anibal, Beto"),
            // Con los vacios a la vista: cada hueco del rango pone su separador.
            ("UNIRCADENAS(\"-\";FALSO;C1:C4)", "Ana-Anibal-Beto-"),
            ("UNIRCADENAS(\"-\";VERDADERO;C1:C4)", "Ana-Anibal-Beto"),
            ("UNIRCADENAS(\"-\";FALSO;A1:A5)", "10-20-30-x-40"),
        ],
        d,
    );
    // Negativos: un rango de varias celdas no es un texto; de menos o de mas.
    assert_eq!(d("CONCATENAR(C1:C3)"), "#¡VALOR!");
    assert_eq!(d("CONCATENAR()"), "#¡VALOR!");
    assert_eq!(d("CONCAT()"), "#¡VALOR!");
    assert_eq!(d("UNIRCADENAS(\"-\";VERDADERO)"), "#¡VALOR!");
    assert_eq!(d("UNIRCADENAS(\"-\";\"x\";\"a\")"), "#¡VALOR!");
    assert_eq!(d("CONCAT(F1:F2)"), "#¡DIV/0!");
    assert_eq!(d("CONCATENAR(\"a\";1/0)"), "#¡DIV/0!");
}

#[test]
fn izquierda_derecha_extrae_largo() {
    varias(
        &[
            ("IZQUIERDA(\"hola\";2)", "ho"),
            ("LEFT(\"hola\")", "h"),
            ("IZQUIERDA(\"hola\";10)", "hola"),
            ("IZQUIERDA(\"hola\";0)", ""),
            ("IZQUIERDA(12345;2)", "12"),
            ("IZQUIERDA(\"añb\";2)", "añ"),
            ("DERECHA(\"hola\";3)", "ola"),
            ("RIGHT(\"hola\")", "a"),
            ("DERECHA(\"hola\";10)", "hola"),
            ("EXTRAE(\"hola mundo\";6;5)", "mundo"),
            ("MID(\"hola\",2,2)", "ol"),
            ("EXTRAE(\"hola\";10;2)", ""),
            ("EXTRAE(\"hola\";3;99)", "la"),
            ("LARGO(\"año\")", "3"),
            ("LEN(123)", "3"),
            ("LARGO(\"\")", "0"),
            ("LARGO(Q9)", "0"),
        ],
        d,
    );
    varias(
        &[
            ("IZQUIERDA(\"hola\";-1)", "#¡VALOR!"),
            ("DERECHA(\"hola\";-1)", "#¡VALOR!"),
            ("IZQUIERDA()", "#¡VALOR!"),
            ("IZQUIERDA(\"a\";1;2)", "#¡VALOR!"),
            ("IZQUIERDA(\"a\";\"x\")", "#¡VALOR!"),
            ("EXTRAE(\"hola\";0;1)", "#¡VALOR!"),
            ("EXTRAE(\"hola\";1;-1)", "#¡VALOR!"),
            ("EXTRAE(\"hola\";1)", "#¡VALOR!"),
            ("LARGO()", "#¡VALOR!"),
            ("LARGO(1/0)", "#¡DIV/0!"),
        ],
        f,
    );
}

#[test]
fn mayusculas_minusculas_nombre_propio_y_espacios() {
    varias(
        &[
            ("MAYUSC(\"añb\")", "AÑB"),
            ("UPPER(\"abc\")", "ABC"),
            ("MINUSC(\"ABC Ñ\")", "abc ñ"),
            ("LOWER(\"ABC\")", "abc"),
            ("NOMPROPIO(\"hola mUNDO o'neil\")", "Hola Mundo O'Neil"),
            ("PROPER(\"ana-maria 2da\")", "Ana-Maria 2Da"),
            ("NOMPROPIO(\"\")", ""),
            ("ESPACIOS(\"  a   b  \")", "a b"),
            ("TRIM(\"a\")", "a"),
            ("ESPACIOS(\"   \")", ""),
            ("MAYUSC(12)", "12"),
        ],
        f,
    );
    for malo in [
        "MAYUSC()",
        "MINUSC(\"a\";\"b\")",
        "NOMPROPIO()",
        "ESPACIOS()",
        "MAYUSC(1/0)",
    ] {
        assert!(f(malo).starts_with('#'), "{malo}");
    }
}

#[test]
fn sustituir_encontrar_y_hallar() {
    varias(
        &[
            ("SUSTITUIR(\"a-b-c\";\"-\";\"+\")", "a+b+c"),
            ("SUBSTITUTE(\"a-b-c\",\"-\",\"+\")", "a+b+c"),
            ("SUSTITUIR(\"a-b-c\";\"-\";\"+\";2)", "a-b+c"),
            ("SUSTITUIR(\"a-b-c\";\"-\";\"+\";5)", "a-b-c"),
            ("SUSTITUIR(\"abc\";\"\";\"x\")", "abc"),
            ("SUSTITUIR(\"ABC\";\"b\";\"x\")", "ABC"),
            ("SUSTITUIR(\"aaa\";\"aa\";\"b\")", "ba"),
            ("SUSTITUIR(\"abc\";\"b\";\"\")", "ac"),
            ("ENCONTRAR(\"b\";\"abcb\")", "2"),
            ("FIND(\"b\",\"abcb\",3)", "4"),
            ("ENCONTRAR(\"\";\"abc\")", "1"),
            ("ENCONTRAR(\"ñ\";\"año\")", "2"),
            ("HALLAR(\"B\";\"abc\")", "2"),
            ("SEARCH(\"a?c\",\"xxabcx\")", "3"),
            ("HALLAR(\"a*c\";\"xxabcx\")", "3"),
            ("HALLAR(\"~?\";\"a?b\")", "2"),
        ],
        f,
    );
    varias(
        &[
            ("ENCONTRAR(\"B\";\"abc\")", "#¡VALOR!"),
            ("ENCONTRAR(\"z\";\"abc\")", "#¡VALOR!"),
            ("ENCONTRAR(\"a\";\"abc\";0)", "#¡VALOR!"),
            ("ENCONTRAR(\"a\";\"abc\";9)", "#¡VALOR!"),
            ("HALLAR(\"z\";\"abc\")", "#¡VALOR!"),
            ("SUSTITUIR(\"a\";\"a\")", "#¡VALOR!"),
            ("SUSTITUIR(\"a\";\"a\";\"b\";0)", "#¡VALOR!"),
            ("ENCONTRAR(\"a\")", "#¡VALOR!"),
        ],
        f,
    );
}

#[test]
fn texto_da_formato_a_numeros() {
    varias(
        &[
            ("TEXTO(1234.5;\"#,##0.00\")", "1,234.50"),
            ("TEXT(1234,\"#,##0\")", "1,234"),
            ("TEXTO(1234567.891;\"#,##0.0\")", "1,234,567.9"),
            ("TEXTO(-1234.5;\"#,##0.0\")", "-1,234.5"),
            ("TEXTO(0.256;\"0%\")", "26%"),
            ("TEXTO(0.256;\"0.0%\")", "25.6%"),
            ("TEXTO(5;\"000\")", "005"),
            ("TEXTO(3.14159;\"0.00\")", "3.14"),
            ("TEXTO(3;\"0.00\")", "3.00"),
            ("TEXTO(3.5;\"0.##\")", "3.5"),
            ("TEXTO(3;\"0.##\")", "3"),
            ("TEXTO(1.005;\"0.00\")", "1.01"),
            ("TEXTO(0.5;\"#.00\")", ".50"),
            ("TEXTO(0;\"0.00\")", "0.00"),
            ("TEXTO(12;\"$#,##0.00\")", "$12.00"),
            ("TEXTO(12;\"0 \"\"kg\"\"\")", "12 kg"),
            ("TEXTO(2.5;\"0\")", "3"),
            ("TEXTO(\"hola\";\"0\")", "hola"),
            ("TEXTO(\"\";\"0.00\")", ""),
            ("TEXTO(7;\"General\")", "7"),
        ],
        f,
    );
    varias(
        &[
            ("TEXTO(1)", "#¡VALOR!"),
            ("TEXTO(1;\"0\";2)", "#¡VALOR!"),
            ("TEXTO(1/0;\"0\")", "#¡DIV/0!"),
            ("TEXTO(1;\"0.0E+00\")", "#¡VALOR!"),
        ],
        f,
    );
}

#[test]
fn texto_da_formato_a_fechas() {
    varias(
        &[
            ("TEXTO(36526;\"dd/mm/aaaa\")", "01/01/2000"),
            ("TEXT(36526,\"dd/mm/yyyy\")", "01/01/2000"),
            ("TEXTO(36526;\"yyyy-mm-dd\")", "2000-01-01"),
            ("TEXTO(36526;\"d/m/yy\")", "1/1/00"),
            ("TEXTO(46306;\"dd/mm/aaaa\")", "11/10/2026"),
            ("TEXTO(46306;\"mmmm\")", "octubre"),
            ("TEXTO(46306;\"mmm\")", "oct"),
            ("TEXTO(46306;\"dddd\")", "domingo"),
            (
                "TEXTO(36526;\"dddd d \"\"de\"\" mmmm\")",
                "sábado 1 de enero",
            ),
            ("TEXTO(0.75;\"hh:mm\")", "18:00"),
            ("TEXTO(0.5;\"h:mm AM/PM\")", "12:00 PM"),
            ("TEXTO(0.25;\"hh:mm:ss\")", "06:00:00"),
            ("TEXTO(46306.5;\"dd/mm/aaaa hh:mm\")", "11/10/2026 12:00"),
            ("TEXTO(60;\"dd/mm/yyyy\")", "29/02/1900"),
            ("TEXTO(\"2000-01-01\";\"dd/mm/yyyy\")", "01/01/2000"),
        ],
        f,
    );
    // Negativos: una fecha anterior a 1900 no existe; un texto que no es
    // fecha pasa tal cual.
    assert_eq!(f("TEXTO(-1;\"dd/mm/yyyy\")"), "#¡NUM!");
    assert_eq!(f("TEXTO(\"hola\";\"dd/mm/yyyy\")"), "hola");
}

#[test]
fn valor_lee_numeros_escritos() {
    varias(
        &[
            ("VALOR(\"12,5\")", "12.5"),
            ("VALUE(\"1.234,50\")", "1234.5"),
            ("VALOR(\"15%\")", "0.15"),
            ("VALOR(\"\")", "0"),
            ("VALOR(5)", "5"),
            ("VALOR(\"2000-01-01\")", "36526"),
            ("VALOR(\"1/1/2000\")", "36526"),
            ("VALOR(\" 7 \")", "7"),
        ],
        f,
    );
    varias(
        &[
            ("VALOR(\"abc\")", "#¡VALOR!"),
            ("VALOR(\"12a\")", "#¡VALOR!"),
            ("VALOR(VERDADERO)", "#¡VALOR!"),
            ("VALOR()", "#¡VALOR!"),
            ("VALOR(1/0)", "#¡DIV/0!"),
        ],
        f,
    );
}

// ---- Fechas -------------------------------------------------------------

#[test]
fn fechas_con_el_serial_de_excel() {
    varias(
        &[
            ("FECHA(2026;10;11)", "46306"),
            ("DATE(2000,1,1)", "36526"),
            ("FECHA(1900;1;1)", "1"),
            ("FECHA(1900;3;1)", "61"),
            ("FECHA(1900;2;29)", "60"),
            ("AÑO(FECHA(2026;14;1))", "2027"),
            ("MES(FECHA(2026;14;1))", "2"),
            // Dia 0: el ultimo del mes anterior. Mes 0: diciembre anterior.
            ("DIA(FECHA(2026;1;0))", "31"),
            ("MES(FECHA(2026;1;0))", "12"),
            ("MES(FECHA(2026;0;15))", "12"),
            ("DIA(FECHA(2026;3;0))", "28"),
            // Un ano de 0 a 1899 se suma a 1900.
            ("AÑO(FECHA(99;1;1))", "1999"),
            ("AÑO(36526)", "2000"),
            ("YEAR(36526)", "2000"),
            ("ANO(36526)", "2000"),
            ("MES(36526)", "1"),
            ("MONTH(46306)", "10"),
            ("DIA(36526)", "1"),
            ("DAY(46306)", "11"),
            ("DIA(60)", "29"),
            ("MES(60)", "2"),
            ("DIA(59)", "28"),
            ("AÑO(\"2000-01-01\")", "2000"),
            ("MES(\"11/10/2026\")", "10"),
            ("DIAS(FECHA(2026;10;11);FECHA(2026;1;1))", "283"),
            ("DAYS(36527,36526)", "1"),
            ("DIAS(36526;36527)", "-1"),
            // Sabado 1 de enero de 2000.
            ("DIASEM(36526)", "7"),
            ("WEEKDAY(36526,2)", "6"),
            ("DIASEM(36526;3)", "5"),
            ("DIASEM(1)", "1"),
            ("DIASEM(46306)", "1"),
        ],
        f,
    );
    // Negativos.
    varias(
        &[
            ("FECHA(-1;1;1)", "#¡NUM!"),
            ("FECHA(10000;1;1)", "#¡NUM!"),
            ("FECHA(2026;1)", "#¡VALOR!"),
            ("FECHA(2026;1;1;1)", "#¡VALOR!"),
            ("FECHA(\"x\";1;1)", "#¡VALOR!"),
            ("AÑO(-1)", "#¡NUM!"),
            ("AÑO(\"hola\")", "#¡VALOR!"),
            ("AÑO()", "#¡VALOR!"),
            ("MES(1;2)", "#¡VALOR!"),
            ("DIA(\"31/04/2026\")", "#¡VALOR!"),
            ("DIASEM(36526;9)", "#¡NUM!"),
            ("DIAS(1)", "#¡VALOR!"),
            ("HOY(1)", "#¡VALOR!"),
            ("AHORA(1)", "#¡VALOR!"),
        ],
        f,
    );
}

#[test]
fn hoy_y_ahora_usan_el_reloj_del_calculo() {
    // 11-oct-2026 15:30 en hora local.
    let ms = (46306 - 25569) as i64 * 86_400_000 + 15 * 3_600_000 + 30 * 60_000;
    let t = tabla(&[
        ("A1", "=HOY()"),
        ("A2", "=AHORA()"),
        ("A3", "=ENTERO(AHORA())=HOY()"),
    ]);
    let mut m = Memoria {
        ahora_ms: Some(ms),
        ..Default::default()
    };
    let mut r = |s: &str| evaluar_camino(&t, ref_de(s).unwrap(), &mut m);
    assert_eq!(r("A1"), Valor::Numero(46306.0));
    match r("A2") {
        Valor::Numero(n) => assert!((n - 46306.645833333).abs() < 1e-6, "{n}"),
        otro => panic!("{otro:?}"),
    }
    assert_eq!(r("A3"), Valor::Logico(true));
    // Con el reloj de verdad: es un dia entero de este siglo.
    match en(&t, "A1") {
        Valor::Numero(n) => assert!(n > 46000.0 && n.fract() == 0.0, "{n}"),
        otro => panic!("{otro:?}"),
    }
}

// ---- Informacion --------------------------------------------------------

#[test]
fn es_error_es_blanco_es_numero() {
    varias(
        &[
            ("ESERROR(1/0)", "VERDADERO"),
            ("ISERROR(F1)", "VERDADERO"),
            ("ESERROR(BUSCARV(5;H1:J4;2;0))", "VERDADERO"),
            ("ESERROR(1)", "FALSO"),
            ("ESERROR(\"x\")", "FALSO"),
            ("ESBLANCO(A9)", "VERDADERO"),
            ("ISBLANK(A1)", "FALSO"),
            ("ESBLANCO(\"\")", "VERDADERO"),
            ("ESBLANCO(F1)", "FALSO"),
            ("ESNUMERO(1)", "VERDADERO"),
            ("ISNUMBER(A1)", "VERDADERO"),
            ("ESNUMERO(\"1\")", "FALSO"),
            ("ESNUMERO(A4)", "FALSO"),
            ("ESNUMERO(F1)", "FALSO"),
            ("ESNUMERO(VERDADERO)", "FALSO"),
            ("ESTEXTO(A4)", "VERDADERO"),
            ("ISTEXT(A1)", "FALSO"),
            ("ESND(BUSCARV(5;H1:J4;2;0))", "VERDADERO"),
            ("ESND(1/0)", "FALSO"),
        ],
        d,
    );
    // Negativos: de menos o de mas.
    varias(
        &[
            ("ESERROR()", "#¡VALOR!"),
            ("ESBLANCO(1;2)", "#¡VALOR!"),
            ("ESNUMERO()", "#¡VALOR!"),
        ],
        d,
    );
    // Un ciclo no se esconde detras de ESERROR.
    let t = tabla(&[("A1", "=ESERROR(A1)")]);
    assert_eq!(en(&t, "A1"), Valor::Error(ErrorFormula::Ciclo));
}

// ---- Nombres, separadores, compatibilidad --------------------------------

#[test]
fn cada_nombre_del_catalogo_lo_atiende_alguna_funcion() {
    // Si un nombre se anade al catalogo y se olvida en su modulo, saldria
    // `#¿NOMBRE?` justo en la funcion que dice existir.
    for nombre in motor::todos_los_nombres() {
        assert!(motor::existe(nombre));
        for n in 0..=6 {
            let args = vec!["1"; n].join(";");
            let r = d(&format!("{nombre}({args})"));
            assert_ne!(r, "#¿NOMBRE?", "{nombre} con {n} argumentos");
        }
    }
    assert_eq!(f("NOEXISTE(1)"), "#¿NOMBRE?");
    assert_eq!(f("SUMAA(1)"), "#¿NOMBRE?");
}

#[test]
fn el_nombre_en_ingles_da_lo_mismo_que_en_espanol() {
    let pares = [
        ("SUMA(1;2;3)", "SUM(1,2,3)"),
        ("SI(1>0;\"a\";\"b\")", "IF(1>0,\"a\",\"b\")"),
        ("SI.ERROR(1/0;7)", "IFERROR(1/0,7)"),
        ("CONTAR.SI(A1:A5;\">15\")", "COUNTIF(A1:A5,\">15\")"),
        (
            "SUMAR.SI(A1:A5;\">15\";B1:B5)",
            "SUMIF(A1:A5,\">15\",B1:B5)",
        ),
        ("BUSCARV(20;H1:J4;2;0)", "VLOOKUP(20,H1:J4,2,0)"),
        ("INDICE(H1:J4;2;2)", "INDEX(H1:J4,2,2)"),
        ("COINCIDIR(30;H1:H4;0)", "MATCH(30,H1:H4,0)"),
        ("IZQUIERDA(\"hola\";2)", "LEFT(\"hola\",2)"),
        ("NOMPROPIO(\"ab cd\")", "PROPER(\"ab cd\")"),
        ("AÑO(36526)", "YEAR(36526)"),
        ("ESNUMERO(A1)", "ISNUMBER(A1)"),
        ("REDONDEAR(2.675;2)", "ROUND(2.675,2)"),
        ("MAX(A1:A3)", "MAXIMO(A1:A3)"),
        ("MAX(A1:A3)", "max(a1:a3)"),
    ];
    for (es, en) in pares {
        assert_eq!(d(es), d(en), "{es} / {en}");
    }
}

#[test]
fn los_dos_separadores_se_pueden_mezclar() {
    assert_eq!(d("SUMA(A1;A2,A3)"), "60");
    assert_eq!(d("SI(1<2,\"a\";\"b\")"), "a");
    // Una coma dentro de un texto es texto.
    assert_eq!(f("CONCATENAR(\"a,b\";\";\")"), "a,b;");
}

#[test]
fn formulas_a_medias_y_basura_siguen_siendo_error() {
    for mala in [
        "SI(",
        "SI(1;2",
        "SUMA(1;;2)",
        "SUMA(;1)",
        "1 +* 2",
        "((1)",
        "1)",
        "\"a\" \"b\"",
        "A1:",
        "A1:5",
        "{1,2}",
        "1 <> ",
        "&1",
        "TRUE FALSE",
        "NOEXISTE",
        "5%%%%%%%%%%%%%%%%%%%% 3",
    ] {
        let r = f(mala);
        assert!(
            r.starts_with('#'),
            "«={mala}» tendria que ser un error y dio «{r}»"
        );
    }
    // Los anidamientos de diez mil parentesis no revientan la pila.
    let hondo = format!("{}1{}", "(".repeat(10_000), ")".repeat(10_000));
    assert_eq!(f(&hondo), "#¡VALOR!");
    let hondo = format!("{}1{}", "SUMA(".repeat(10_000), ")".repeat(10_000));
    assert_eq!(f(&hondo), "#¡VALOR!");
    let hondo = format!("{}1", "-".repeat(10_000));
    assert_eq!(f(&hondo), "#¡VALOR!");
}

#[test]
fn un_rango_de_varias_celdas_en_una_cuenta_sigue_siendo_error() {
    assert_eq!(d("A1:A3*2"), "#¡VALOR!");
    assert_eq!(d("SI(A1:A3;1;2)"), "#¡VALOR!");
    // Un rango de una celda es esa celda.
    assert_eq!(d("A1:A1*2"), "20");
    // Y una funcion puede devolver un rango que otra consume.
    assert_eq!(d("SUMA(SI(VERDADERO;A1:A3;B1:B3))"), "60");
}

#[test]
fn las_cinco_funciones_de_siempre_se_muestran_igual() {
    // El texto de lo que ya existia no cambia.
    let t = tabla(&[
        ("A1", "10"),
        ("A2", "20.5"),
        ("A3", "x"),
        ("B1", "=SUMA(A1:A3)"),
        ("B2", "=CONTAR(A1:A3)"),
        ("B3", "=PROMEDIO(A1:A3)"),
        ("B4", "=MIN(A1:A3)"),
        ("B5", "=MAX(A1:A3)"),
        ("B6", "=SUMA()"),
        ("B7", "=MAX()"),
        ("B8", "=PROMEDIO()"),
    ]);
    let v = evaluar_todo(&t);
    let texto = |k: &str| v[k].to_string();
    assert_eq!(texto("B1"), "30.5");
    assert_eq!(texto("B2"), "2");
    assert_eq!(texto("B3"), "15.25");
    assert_eq!(texto("B4"), "10");
    assert_eq!(texto("B5"), "20.5");
    assert_eq!(texto("B6"), "0");
    assert_eq!(texto("B7"), "0");
    assert_eq!(texto("B8"), "#¡DIV/0!");
    assert_eq!(v["B1"].mostrar(','), "30,5");
}

// ---- Rendimiento ----------------------------------------------------------

/// Una hoja de 2 000 celdas con formulas de todas las familias.
fn hoja_grande() -> Tabla {
    let mut t = Tabla::default();
    let nombres = ["Ana", "Beto", "Carla", "Dario", "Elena"];
    // Datos: 100 filas de numero, nombre y codigo.
    for i in 1..=100usize {
        t.poner(ref_de(&format!("A{i}")).unwrap(), &(i * 3).to_string());
        t.poner(ref_de(&format!("B{i}")).unwrap(), nombres[i % 5]);
        t.poner(ref_de(&format!("C{i}")).unwrap(), &format!("{}", i % 7));
    }
    let mut hechas = 0;
    for i in 1..=100usize {
        // 20 formulas por fila.
        let filas: [(&str, String); 20] = [
            ("E", format!("=A{i}*2+C{i}")),
            ("F", format!("=SI(A{i}>150;\"alto\";\"bajo\")")),
            ("G", format!("=SI.ERROR(A{i}/C{i};0)")),
            ("H", format!("=REDONDEAR(E{i}/7;2)")),
            ("I", format!("=SUMA(A{i}:C{i})")),
            ("J", format!("=PROMEDIO(A1:A{i})")),
            ("K", format!("=BUSCARV(A{i};$A$1:$C$100;3;FALSO)")),
            (
                "L",
                format!("=INDICE($B$1:$B$100;COINCIDIR(A{i};$A$1:$A$100;0))"),
            ),
            ("M", format!("=CONTAR.SI($B$1:$B$100;B{i})")),
            ("N", format!("=SUMAR.SI($B$1:$B$100;B{i};$A$1:$A$100)")),
            ("O", format!("=MAYUSC(IZQUIERDA(B{i};2))&\"-\"&LARGO(B{i})")),
            ("P", format!("=TEXTO(A{i}/7;\"0.00\")")),
            ("Q", format!("=FECHA(2026;1;A{i})")),
            ("R", format!("=TEXTO(Q{i};\"dd/mm/aaaa\")")),
            ("S", format!("=Y(A{i}>10;C{i}<5)")),
            ("T", format!("=SI(S{i};E{i};-E{i})")),
            ("U", format!("=ESNUMERO(K{i})")),
            ("V", format!("=MAX(E{i};G{i};H{i})")),
            ("W", format!("=CONCATENAR(B{i};\" \";F{i})")),
            ("X", format!("=SUSTITUIR(W{i};\" \";\"_\")")),
        ];
        for (col, formula) in filas {
            t.poner(ref_de(&format!("{col}{i}")).unwrap(), &formula);
            hechas += 1;
        }
    }
    assert_eq!(hechas, 2000);
    t
}

/// El tiempo de CPU que lleva gastado este hilo, en milisegundos. Con el
/// reloj de pared, otro programa compilando al lado hacia que el mismo calculo
/// midiera el doble; el de CPU solo cuenta lo que este hilo hace.
#[cfg(windows)]
fn cpu_del_hilo_ms() -> f64 {
    #[repr(C)]
    #[derive(Default)]
    struct Tiempo {
        bajo: u32,
        alto: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> isize;
        fn GetThreadTimes(
            hilo: isize,
            creado: *mut Tiempo,
            salido: *mut Tiempo,
            nucleo: *mut Tiempo,
            usuario: *mut Tiempo,
        ) -> i32;
    }
    let (mut a, mut b, mut k, mut u) = (
        Tiempo::default(),
        Tiempo::default(),
        Tiempo::default(),
        Tiempo::default(),
    );
    // SAFETY: `GetThreadTimes` solo escribe en las cuatro estructuras que se
    // le pasan, del tamano de un FILETIME, y el manejador del hilo actual no
    // hay que cerrarlo.
    let bien = unsafe { GetThreadTimes(GetCurrentThread(), &mut a, &mut b, &mut k, &mut u) };
    assert_ne!(bien, 0);
    let cien_ns = |t: &Tiempo| (((t.alto as u64) << 32) | t.bajo as u64) as f64;
    (cien_ns(&k) + cien_ns(&u)) / 10_000.0
}

#[cfg(not(windows))]
fn cpu_del_hilo_ms() -> f64 {
    // Sin reloj de CPU a mano: el de pared.
    use std::time::Instant;
    thread_local!(static CERO: Instant = Instant::now());
    CERO.with(|c| c.elapsed().as_secs_f64() * 1000.0)
}

/// Lo que tarda en recalcular la hoja grande, en ms de CPU. El reloj de CPU
/// de un hilo avanza a saltos de unos 15 ms, asi que se mide el total de
/// varias vueltas seguidas; y como otro programa compilando al lado encarece
/// cada vuelta, se queda con la mejor de varias tandas (el ruido solo suma).
fn tiempo_de_recalcular(t: &Tabla) -> (f64, BTreeMap<String, Valor>) {
    const VUELTAS: usize = 4;
    let mut v = BTreeMap::new();
    let mut mejor = f64::MAX;
    for _ in 0..8 {
        let t0 = cpu_del_hilo_ms();
        for _ in 0..VUELTAS {
            v = evaluar_todo(t);
        }
        mejor = mejor.min((cpu_del_hilo_ms() - t0) / VUELTAS as f64);
    }
    (mejor, v)
}

#[test]
fn una_hoja_de_2000_formulas_se_recalcula_en_menos_de_50_ms() {
    let t = hoja_grande();
    let (mejor, v) = tiempo_de_recalcular(&t);
    // Que la hoja de verdad calcule: ni un error en las 2 000.
    let errores: Vec<_> = v
        .iter()
        .filter(|(_, v)| matches!(v, Valor::Error(_)))
        .collect();
    assert!(errores.is_empty(), "{errores:?}");
    assert_eq!(v["K10"], Valor::Numero(3.0), "A10=30 y su C es 10%7=3");
    assert_eq!(v["R1"].to_string(), "03/01/2026");
    eprintln!("recalcular 2000 formulas: {mejor:.1} ms de CPU");
    assert!(
        mejor < 50.0,
        "recalcular 2000 formulas gasto {mejor:.1} ms de CPU (limite: 50 ms)"
    );
}
