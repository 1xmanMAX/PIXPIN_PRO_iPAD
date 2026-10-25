use super::*;
use std::io::Write;

fn zip(entradas: &[(&str, &str)]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, t) in entradas {
        z.start_file(*n, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(t.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn crudo<'a>(t: &'a Tabla, dir: &str) -> &'a str {
    t.celda(ref_de(dir).unwrap())
}

fn ver(t: &Tabla, dir: &str) -> String {
    formula::evaluar(t, ref_de(dir).unwrap()).to_string()
}

const LIBRO: &str = r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><workbookPr/><sheets><sheet name="Gastos" sheetId="1" r:id="rId1"/><sheet name="Oculta" sheetId="2" state="hidden" r:id="rId2"/><sheet name="Resumen" sheetId="3" r:id="rId3"/></sheets></workbook>"#;
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="w" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="w" Target="worksheets/sheet2.xml"/><Relationship Id="rId3" Type="w" Target="/xl/worksheets/sheet3.xml"/></Relationships>"#;
const TEXTOS: &str = r#"<sst><si><t>Concepto</t></si><si><r><t>Tot</t></r><r><t>al</t></r></si><si><t>00123</t></si><si><t>=no es fórmula</t></si></sst>"#;
const ESTILOS: &str = r##"<styleSheet><numFmts><numFmt numFmtId="164" formatCode="dd/mm/yyyy;@"/></numFmts><fonts><font><sz val="11"/></font><font><b/><sz val="11"/></font></fonts><fills><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill></fills><cellStyleXfs><xf numFmtId="0" fontId="1"/></cellStyleXfs><cellXfs><xf numFmtId="0" fontId="0" fillId="0"/><xf numFmtId="0" fontId="1" fillId="2" applyAlignment="1"><alignment horizontal="center"/></xf><xf numFmtId="164" fontId="0" fillId="0"/><xf numFmtId="9" fontId="0" fillId="0"/></cellXfs></styleSheet>"##;
const HOJA1: &str = r#"<worksheet><cols><col min="1" max="1" width="20" customWidth="1"/></cols><sheetData>
    <row r="1"><c r="A1" t="s" s="1"><v>0</v></c><c r="B1" t="s"><v>2</v></c><c r="C1" t="s"><v>3</v></c></row>
    <row r="2"><c r="A2"><v>0.30000000000000004</v></c><c r="B2" s="2"><v>46275</v></c><c r="C2" s="3"><v>0.15</v></c></row>
    <row r="3"><c r="A3"><v>10</v></c><c r="B3"><f t="shared" ref="B3:B4" si="0">A3*2</f><v>20</v></c></row>
    <row r="4"><c r="A4"><v>7</v></c><c r="B4"><f t="shared" si="0"/><v>14</v></c></row>
    <row r="5"><c r="A5" t="s"><v>1</v></c><c r="B5"><f>SUM(B3:B4)</f><v>34</v></c><c r="C5"><f>_xlfn.XLOOKUP(7,A3:A4,B3:B4)</f><v>14</v></c><c r="D5"><f>CUBEVALUE("x")</f><v>99</v></c><c r="E5"><f>D5+1</f><v>100</v></c><c r="F5" t="str"><f>"a"&amp;"b"</f><v>ab</v></c><c r="G5" t="b"><v>1</v></c></row>
    </sheetData></worksheet>"#;

pub(crate) fn libro_de_prueba() -> Vec<u8> {
    zip(&[
        ("xl/workbook.xml", LIBRO),
        ("xl/_rels/workbook.xml.rels", RELS),
        ("xl/sharedStrings.xml", TEXTOS),
        ("xl/styles.xml", ESTILOS),
        ("xl/worksheets/sheet1.xml", HOJA1),
        (
            "xl/worksheets/sheet2.xml",
            r#"<worksheet><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData></worksheet>"#,
        ),
        (
            "xl/worksheets/sheet3.xml",
            r#"<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Hola</t></is></c></row></sheetData></worksheet>"#,
        ),
    ])
}

#[test]
fn un_xlsx_con_dos_hojas_visibles_estilos_fechas_y_formulas() {
    // La prueba del movil (`ImportarHojasTest`), con lo que aqui cambia
    // dicho donde cambia.
    let hojas = leer_bytes(&libro_de_prueba(), "Cuentas 2026.xlsx", "", 7).unwrap();
    let nombres: Vec<&str> = hojas.iter().map(|h| h.nombre.as_str()).collect();
    assert_eq!(
        nombres,
        ["Cuentas 2026 · Gastos", "Cuentas 2026 · Resumen"],
        "la oculta no entra"
    );
    let t = &hojas[0].tabla;
    assert_eq!(t.nombre, "Cuentas 2026 · Gastos");
    assert_eq!(t.tocado, 7);
    assert_eq!(ver(t, "A1"), "Concepto");
    let e = &t.estilos["A1"];
    assert!(e.n);
    assert_eq!(e.a.as_deref(), Some("c"));
    assert_eq!(e.f.as_deref(), Some("#ffff00"));
    assert_eq!(
        crudo(t, "B1"),
        "'00123",
        "un texto que parece numero sigue siendo texto"
    );
    assert_eq!(ver(t, "B1"), "00123");
    assert_eq!(ver(t, "C1"), "=no es fórmula");
    assert_eq!(crudo(t, "A2"), "0.3", "diecisiete cifras, a quince");
    assert_eq!(ver(t, "B2"), "10/09/2026");
    assert_eq!(crudo(t, "C2"), "15%");
    // El calculo del PC no guarda el formato del numero (el movil si, y
    // ensena «15%»): vale lo mismo, y suma lo mismo.
    assert_eq!(ver(t, "C2"), "0.15");
    assert_eq!(crudo(t, "B4"), "=A4*2", "formula compartida");
    assert_eq!(ver(t, "B4"), "14");
    assert_eq!(crudo(t, "B5"), "=SUM(B3:B4)");
    assert_eq!(ver(t, "B5"), "34");
    // XLOOKUP ya se calcula aqui y da lo que dio Excel: sigue siendo formula.
    assert_eq!(crudo(t, "C5"), "=XLOOKUP(7,A3:A4,B3:B4)");
    assert_eq!(ver(t, "C5"), "14");
    assert_eq!(
        crudo(t, "D5"),
        "99",
        "la funcion que aqui no hay se queda con su valor"
    );
    assert_eq!(
        crudo(t, "E5"),
        "=D5+1",
        "y la que depende de ella sigue siendo formula"
    );
    assert_eq!(ver(t, "E5"), "100");
    assert_eq!(ver(t, "F5"), "ab");
    assert_eq!(ver(t, "G5"), "VERDADERO");
    assert_eq!(t.anchos.get("A"), Some(&145));
    // Solo CUBEVALUE se queda con su valor: XLOOKUP y el «&» (que antes
    // tampoco habia) ya se calculan aqui y dan lo que dio Excel.
    assert_eq!(hojas[0].formulas_como_valor, 1);
    assert_eq!(ver(&hojas[1].tabla, "A1"), "Hola");
}

#[test]
fn un_csv_de_la_excel_espanola() {
    let texto = "\u{feff}Producto;Precio\n\"Pan; grande\";1.234,50\nLeche;3,20\n";
    let hojas = leer_bytes(texto.as_bytes(), "precios.csv", "", 0).unwrap();
    assert_eq!(hojas.len(), 1);
    assert_eq!(hojas[0].nombre, "precios");
    let mut t = hojas[0].tabla.clone();
    assert_eq!(ver(&t, "A1"), "Producto", "sin la marca de orden de bytes");
    assert_eq!(ver(&t, "A2"), "Pan; grande");
    t.poner(ref_de("B4").unwrap(), "=SUMA(B2:B3)");
    assert_eq!(ver(&t, "B4"), "1237.7");
}

#[test]
fn un_csv_de_windows_en_latin1_no_sale_roto() {
    // «Año;Señal» en Latin-1: no es UTF-8 valido.
    let bytes = [
        b'A', 0xF1, b'o', b';', b'S', b'e', 0xF1, b'a', b'l', b'\r', b'\n', b'1', b';', b'2',
    ];
    let hojas = leer_bytes(&bytes, "raro.csv", "", 0).unwrap();
    assert_eq!(ver(&hojas[0].tabla, "A1"), "Año");
    assert_eq!(ver(&hojas[0].tabla, "B1"), "Señal");
    assert_eq!(ver(&hojas[0].tabla, "B2"), "2");
}

#[test]
fn un_ods_con_filas_repetidas_y_formulas() {
    let contenido = r#"<?xml version="1.0" encoding="UTF-8"?>
        <office:document-content xmlns:office="o" xmlns:table="t" xmlns:text="x"><office:body><office:spreadsheet>
        <table:table table:name="Hoja1">
          <table:table-column table:number-columns-repeated="16384"/>
          <table:table-row>
            <table:table-cell office:value-type="float" office:value="2"><text:p>2</text:p></table:table-cell>
            <table:table-cell office:value-type="float" office:value="3"><text:p>3</text:p></table:table-cell>
            <table:table-cell table:formula="of:=SUM([.A1:.B1])" office:value-type="float" office:value="5"><text:p>5</text:p></table:table-cell>
            <table:table-cell table:formula="of:=[Hoja2.A1]" office:value-type="float" office:value="42"><text:p>42</text:p></table:table-cell>
            <table:table-cell table:number-columns-repeated="16380"/>
          </table:table-row>
          <table:table-row table:number-rows-repeated="2">
            <table:table-cell office:value-type="string"><text:p>x<text:s text:c="2"/>y</text:p></table:table-cell>
            <table:table-cell office:value-type="date" office:date-value="2026-09-10"><text:p>10/09/26</text:p></table:table-cell>
          </table:table-row>
          <table:table-row table:number-rows-repeated="1048570"><table:table-cell table:number-columns-repeated="16384"/></table:table-row>
        </table:table>
        </office:spreadsheet></office:body></office:document-content>"#;
    let hojas = leer_bytes(&zip(&[("content.xml", contenido)]), "libro.ods", "", 0).unwrap();
    let t = &hojas[0].tabla;
    assert_eq!(crudo(t, "C1"), "=SUM(A1:B1)");
    assert_eq!(ver(t, "C1"), "5");
    assert_eq!(crudo(t, "D1"), "42", "otra hoja: se queda con su valor");
    assert_eq!(ver(t, "A2"), "x  y");
    assert_eq!(ver(t, "A3"), "x  y", "la fila repetida se copia");
    assert_eq!(ver(t, "B3"), "10/09/2026");
    assert_eq!(t.tamano().1, 3, "las filas vacias no ocupan");
}

#[test]
fn un_xls_antiguo_se_reconoce_y_se_dice_que_no_se_lee() {
    assert!(es_libro("viejo.xls"));
    assert!(es_libro("Cuentas.XLSX"));
    assert!(!es_libro("foto.png"));
    assert!(!es_libro("sin-extension"));
    let firma = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1];
    assert_eq!(
        leer_bytes(&firma, "viejo.xls", "", 0),
        Err(NoSeLee::XlsAntiguo)
    );
    // Y sin extension, por su firma.
    assert_eq!(leer_bytes(&firma, "", "blob", 0), Err(NoSeLee::XlsAntiguo));
    assert!(NoSeLee::XlsAntiguo.to_string().contains(".xlsx"));
}

#[test]
fn un_zip_que_no_es_un_libro_o_un_libro_vacio_se_dicen() {
    let otro = zip(&[("hola.txt", "no")]);
    assert_eq!(
        leer_bytes(&otro, "x.xlsx", "", 0),
        Err(NoSeLee::NoEsUnLibro)
    );
    assert_eq!(
        leer_bytes(b"no soy zip", "x.xlsx", "", 0),
        Err(NoSeLee::NoEsUnLibro)
    );
    let vacio = zip(&[
        ("xl/workbook.xml", LIBRO),
        ("xl/_rels/workbook.xml.rels", RELS),
        (
            "xl/worksheets/sheet1.xml",
            "<worksheet><sheetData/></worksheet>",
        ),
    ]);
    assert_eq!(leer_bytes(&vacio, "x.xlsx", "", 0), Err(NoSeLee::SinHojas));
    assert_eq!(leer_bytes(b"", "x.csv", "", 0), Err(NoSeLee::SinHojas));
}

#[test]
fn sin_extension_se_mira_por_dentro() {
    let hojas = leer_bytes(&libro_de_prueba(), "", "a1b2c3", 0).unwrap();
    assert_eq!(hojas.len(), 2, "un ZIP sin content.xml es un xlsx");
    let hojas = leer_bytes(b"a,b\n1,2", "", "a1b2c3", 0).unwrap();
    assert_eq!(ver(&hojas[0].tabla, "B2"), "2", "y lo demas, CSV");
}

#[test]
fn copiar_una_formula_mueve_lo_que_no_lleva_dolar() {
    assert_eq!(desplazar("=A3*2", 1, 0), "=A4*2");
    assert_eq!(desplazar("=$A$3+A$3+$A3+B3", 2, 1), "=$A$3+B$3+$A5+C5");
    assert_eq!(desplazar("=SUM(A1:B2)", 1, 1), "=SUM(B2:C3)");
    // Lo que va entre comillas y los nombres de funcion no son celdas.
    assert_eq!(desplazar("=LOG10(A1)&\"B2\"", 1, 0), "=LOG10(A2)&\"B2\"");
    // Caso negativo: lo que se sale de la hoja queda en #¡REF!.
    assert_eq!(desplazar("=A1", -1, 0), "=#¡REF!");
    assert_eq!(desplazar("=A1", 0, 0), "=A1");
}

#[test]
fn las_referencias_de_una_formula_se_leen_con_sus_rangos() {
    assert_eq!(
        referencias("=SUM(B3:B4)+D5"),
        vec![(1, 2, 1, 3), (3, 4, 3, 4)]
    );
    assert_eq!(referencias("=LOG10(2)"), vec![]);
    assert!(referencias("no es formula").is_empty());
}

#[test]
fn el_xml_no_pide_ficheros_por_entidades() {
    // Un DOCTYPE con entidad externa: se salta entero y la entidad no existe
    // (se queda escrita tal cual).
    let mut textos = Vec::new();
    sax(
        r#"<?xml version="1.0"?><!DOCTYPE x [<!ENTITY e SYSTEM "file:///c:/windows/win.ini">]><x>&e;&amp;&#65;&#x42;</x>"#,
        &mut |ev| {
            if let Ev::Texto(t) = ev {
                textos.push(t.to_string());
            }
        },
    );
    assert_eq!(textos.concat(), "&e;&AB");
}

#[test]
fn las_fechas_de_excel_se_escriben_como_en_el_movil() {
    assert_eq!(texto_de_fecha(46275.0), "10/09/2026");
    assert_eq!(texto_de_fecha(46275.5), "10/09/2026 12:00");
    assert_eq!(texto_de_fecha(1.0), "31/12/1899");
    assert_eq!(texto_de_fecha(f64::NAN), "#¡NUM!");
}

#[test]
fn el_separado_entiende_comillas_de_verdad_y_comillas_de_texto() {
    assert_eq!(
        de_separado("a;\"b;c\";\"di \"\"x\"\"\"\r\n\"no\" cierra;e", ';'),
        vec![vec!["a", "b;c", "di \"x\""], vec!["\"no\" cierra", "e"]]
    );
    assert!(de_separado("", ',').is_empty());
}

/// Cuanto tarda un libro grande: 10 000 filas con numeros, una formula por
/// fila y un saldo acumulado. `cargo test --release -- --ignored medir`.
#[test]
#[ignore]
fn medir_un_libro_de_diez_mil_filas() {
    let filas = 10_000;
    let mut hoja = String::from("<worksheet><sheetData>");
    let mut saldo = 0.0;
    for f in 1..=filas {
        let (a, b) = (f as f64, (f % 7) as f64);
        saldo += a + b;
        hoja.push_str(&format!(
            "<row r=\"{f}\"><c r=\"A{f}\"><v>{a}</v></c><c r=\"B{f}\"><v>{b}</v></c><c r=\"C{f}\"><f>A{f}+B{f}</f><v>{}</v></c><c r=\"D{f}\"><f>{}</f><v>{saldo}</v></c></row>",
            a + b,
            if f == 1 { "C1".to_string() } else { format!("D{}+C{f}", f - 1) }
        ));
    }
    hoja.push_str("</sheetData></worksheet>");
    let libro = zip(&[
        ("xl/workbook.xml", LIBRO),
        ("xl/_rels/workbook.xml.rels", RELS),
        ("xl/worksheets/sheet1.xml", &hoja),
    ]);
    let t0 = std::time::Instant::now();
    let hojas = leer_bytes(&libro, "grande.xlsx", "", 0).unwrap();
    let ms = t0.elapsed().as_millis();
    let t = &hojas[0].tabla;
    eprintln!(
        "{} celdas, {} formulas como valor, {ms} ms, xml {} kB",
        t.celdas.len(),
        hojas[0].formulas_como_valor,
        hoja.len() / 1024
    );
    assert_eq!(t.celdas.len(), filas * 4);
}
