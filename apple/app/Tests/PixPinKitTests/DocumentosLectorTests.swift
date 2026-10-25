import PixPinCore
import XCTest
@testable import PixPinKit

/// Lo puro de los lectores: tablas sueltas, el PDF roto, los marcos, la
/// pagina que sube sola, las diapositivas, el indice y la voz de la nota.
@MainActor
final class DocumentosLectorTests: XCTestCase {
    func testUnCsvConComillasYSaltos() {
        let t = "nombre;nota\n\"Pérez; Ana\";7,5\r\n\"Dijo \"\"hola\"\"\";\"dos\nrenglones\"\n"
        XCTAssertEqual(TablaCsv.separador(de: Substring(t), ext: "csv"), ";")
        let f = TablaCsv.leer(t, separador: ";")
        XCTAssertEqual(f.count, 3)
        XCTAssertEqual(f[0], ["nombre", "nota"])
        XCTAssertEqual(f[1], ["Pérez; Ana", "7,5"])
        XCTAssertEqual(f[2], ["Dijo \"hola\"", "dos\nrenglones"])
        XCTAssertEqual(TablaCsv.separador(de: "a,b,c\n1,2,3", ext: "csv"), ",")
        XCTAssertEqual(TablaCsv.separador(de: "a;b", ext: "tsv"), "\t")
        // Sin salto al final, la ultima fila tambien cuenta.
        XCTAssertEqual(TablaCsv.leer("a,b\n1,2", separador: ","), [["a", "b"], ["1", "2"]])
    }

    func testLaCopiaLimpiaDelProyecto() throws {
        let carpeta = FileManager.default.temporaryDirectory.appendingPathComponent("sano-\(UUID().uuidString)")
        let archivos = carpeta.appendingPathComponent("archivos")
        try FileManager.default.createDirectory(at: archivos, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let doc = archivos.appendingPathComponent("doc-7.pdf")
        let limpio = archivos.appendingPathComponent("limpio-7.pdf")
        try Data("roto".utf8).write(to: doc)
        try Data("%PDF".utf8).write(to: limpio)
        let ficha = Data(#"{"id":"p","pdfOrigen":"archivos/doc-7.pdf","pdfLimpio":"archivos/limpio-7.pdf"}"#.utf8)
        XCTAssertEqual(PdfSano.copiaLimpia(de: doc, carpeta: carpeta, proyecto: ficha)?.lastPathComponent, "limpio-7.pdf")
        // Otro PDF de la misma carpeta no es el documento del proyecto.
        let otro = archivos.appendingPathComponent("otro.pdf")
        XCTAssertNil(PdfSano.copiaLimpia(de: otro, carpeta: carpeta, proyecto: ficha))
        // Sin copia limpia no hay nada que hacer.
        XCTAssertNil(PdfSano.copiaLimpia(de: doc, carpeta: carpeta, proyecto: Data(#"{"pdfOrigen":"archivos/doc-7.pdf"}"#.utf8)))
    }

    func testLasUnidadesDeLosMarcos() {
        // Dos hojas A4 (595 x 842): 1400 de ancho, 18 de hueco.
        let h = HojasDelLector(tamanos: [CGSize(width: 595, height: 842), CGSize(width: 595, height: 842)])
        let alto = 1400 * 842 / 595.0
        XCTAssertEqual(h.arriba[1], CGFloat(alto + 18), accuracy: 0.01)
        let b = CGRect(x: 0, y: 0, width: 595, height: 842)
        // La esquina de arriba a la izquierda de la segunda hoja.
        let u = h.unidades(CGPoint(x: 0, y: 842), hoja: 1, bounds: b)
        XCTAssertEqual(u.x, 0, accuracy: 0.01)
        XCTAssertEqual(u.y, h.arriba[1], accuracy: 0.01)
        XCTAssertEqual(h.hoja(de: u.y + 10), 1)
        XCTAssertEqual(h.hoja(de: 10), 0)
        // Ida y vuelta.
        let p = h.punto(CGPoint(x: 700, y: h.arriba[1] + 100), bounds: b, hoja: 1)
        let v = h.unidades(p, hoja: 1, bounds: b)
        XCTAssertEqual(v.x, 700, accuracy: 0.01)
        XCTAssertEqual(v.y, h.arriba[1] + 100, accuracy: 0.01)
        // Un marco en la vista de su hoja (a la mitad de tamano).
        let m = MarcoDoc(caja: [0, Double(h.arriba[1]), 700, Double(h.arriba[1]) + 100])
        let r = h.rect(m, enHoja: 1, vista: CGRect(x: 10, y: 0, width: 700, height: CGFloat(alto) / 2))
        XCTAssertEqual(r?.minX ?? -1, 10, accuracy: 0.01)
        XCTAssertEqual(r?.width ?? -1, 350, accuracy: 0.01)
        XCTAssertNil(h.rect(m, enHoja: 0, vista: b))
    }

    func testLosMarcosComoLosGuardaWindows() {
        let json = Data(#"[{"caja":[1,2,3,4],"parte":0},{"caja":[1,2]},{"caja":[5,6,7,8],"parte":2,"vista":"1 0 0"}]"#.utf8)
        let l = MarcosDoc.leer(json)
        XCTAssertEqual(l.count, 2)
        XCTAssertEqual(l[1].parte, 2)
        XCTAssertEqual(l[0].rect, CGRect(x: 1, y: 2, width: 2, height: 2))
    }

    func testLaPaginaQueSubeSola() {
        let a = AvanceLectorIPad()
        a.activo = true
        // La primera vuelta solo apunta la hora.
        XCTAssertEqual(a.paso(ahora: 10, porSegundo: 100, quieta: false, pixel: 0.5), 0)
        // Una decima: 10 puntos.
        XCTAssertEqual(a.paso(ahora: 10.1, porSegundo: 100, quieta: false, pixel: 0.5), 10, accuracy: 0.001)
        // Mas de una decima cuenta como una decima.
        XCTAssertEqual(a.paso(ahora: 12, porSegundo: 100, quieta: false, pixel: 0.5), 10, accuracy: 0.001)
        // Lo que no llega a un pixel se guarda para la vuelta siguiente.
        XCTAssertEqual(a.paso(ahora: 12.002, porSegundo: 100, quieta: false, pixel: 0.5), 0)
        XCTAssertEqual(a.paso(ahora: 12.004, porSegundo: 100, quieta: false, pixel: 0.5), 0.0, accuracy: 0.5)
        // Quieta (anotando) no corre y empieza de nuevo.
        XCTAssertEqual(a.paso(ahora: 13, porSegundo: 100, quieta: true, pixel: 0.5), 0)
        XCTAssertNil(a.antes)
        a.alFinal()
        XCTAssertFalse(a.enMarcha)
    }

    func testElPaseDeDiapositivas() {
        var p = PaseDiapositivas(actual: 0, total: 3)
        XCTAssertTrue(p.aplicar(.siguiente))
        XCTAssertTrue(p.aplicar(.siguiente))
        XCTAssertTrue(p.aplicar(.siguiente))
        XCTAssertEqual(p.actual, 2) // no da la vuelta
        XCTAssertFalse(p.puedeSiguiente)
        XCTAssertTrue(p.aplicar(.negro))
        XCTAssertTrue(p.negro)
        // Con la pantalla negra, pasar primero la enciende.
        XCTAssertTrue(p.aplicar(.anterior))
        XCTAssertFalse(p.negro)
        XCTAssertEqual(p.actual, 1)
        XCTAssertFalse(p.aplicar(.salir))
        XCTAssertEqual(CuentasDiapositivas.mando(toqueEn: 10, ancho: 300), .anterior)
        XCTAssertEqual(CuentasDiapositivas.mando(toqueEn: 150, ancho: 300), .pastilla)
        XCTAssertEqual(CuentasDiapositivas.mando(toqueEn: 290, ancho: 300), .siguiente)
        XCTAssertEqual(CuentasDiapositivas.mando(tecla: 0x4F, caracteres: "", mayusculas: false), .siguiente)
        XCTAssertEqual(CuentasDiapositivas.mando(tecla: 0x2C, caracteres: " ", mayusculas: true), .anterior)
        XCTAssertEqual(CuentasDiapositivas.mando(tecla: 0, caracteres: "B", mayusculas: true), .negro)
        XCTAssertEqual(CuentasDiapositivas.encajar(CGSize(width: 16, height: 9), en: CGSize(width: 1600, height: 1200)),
                       CGRect(x: 0, y: 150, width: 1600, height: 900))
        XCTAssertTrue(CuentasDiapositivas.tocaTransicion(antes: 1, ahora: 2, encendidas: true, ms: 500))
        XCTAssertFalse(CuentasDiapositivas.tocaTransicion(antes: 2, ahora: 1, encendidas: true, ms: 500))
        XCTAssertFalse(CuentasDiapositivas.tocaTransicion(antes: 1, ahora: 2, encendidas: false, ms: 500))
        XCTAssertEqual(CuentasDiapositivas.progreso(transcurrido: 0.25, ms: 500), 0.5, accuracy: 0.001)
        XCTAssertTrue(CuentasDiapositivas.sePresenta(URL(fileURLWithPath: "/tmp/clase.PPTX")))
        XCTAssertFalse(CuentasDiapositivas.sePresenta(URL(fileURLWithPath: "/tmp/viejo.ppt")))
    }

    func testElCapituloEnElQueSeEsta() {
        let l = [EntradaIndice(id: 0, titulo: "Uno", nivel: 0, destino: 0),
                 EntradaIndice(id: 1, titulo: "Dos", nivel: 0, destino: 5),
                 EntradaIndice(id: 2, titulo: "Dos.1", nivel: 1, destino: 7)]
        XCTAssertEqual(EntradaIndice.actual(l, en: 6), 1)
        XCTAssertEqual(EntradaIndice.actual(l, en: 7), 2)
        XCTAssertEqual(EntradaIndice.actual(l, en: 0), 0)
    }

    func testLaNotaSeLeeSinSusMarcas() {
        let s = "## Tema **uno**"
        let limpio = VozDeLaNotaIPad.sinMarcas(s)
        XCTAssertEqual((limpio as NSString).length, (s as NSString).length)
        XCTAssertEqual(limpio.split(separator: " ").joined(separator: " "), "Tema uno")
        XCTAssertEqual(VozDeLaNotaIPad.sinMarcas("- [x] hecho").trimmingCharacters(in: .whitespaces), "hecho")
    }

    func testLoQueDiceAlAligerar() {
        XCTAssertEqual(textoDeAligerado(.sinMejora), "Ya estaba ligero.")
        XCTAssertTrue(textoDeAligerado(.aligerado(antes: 1000, despues: 500)).contains("−50 %"))
    }
}
