// Los planos en papel y los marcadores del iPad: las cuentas de la lamina
// (escala 1:n, unidades, la hoja del PDF), el aviso de Civil 3D, los
// marcadores y marcos 3D en los ficheros de Windows, y, si hay tarjeta, el
// PDF y la imagen de un DXF de verdad.

import Foundation
import Metal
import PixPinCore
@testable import PixPinKit
import XCTest

/// Dos rayas y un circulo, en metros ($INSUNITS = 6).
private let dxfEnMetros = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n10\n21\n0\n31\n0\n0\nLINE\n8\n0\n10\n10\n20\n0\n30\n0\n11\n10\n21\n6\n31\n0\n0\nCIRCLE\n8\n0\n10\n5\n20\n3\n30\n0\n40\n2\n0\nENDSEC\n0\nEOF\n"

private func carpeta(_ nombre: String) throws -> URL {
    let c = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-papel-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: c, withIntermediateDirectories: true)
    return c
}

final class PlanosLaminaTests: XCTestCase {
    func testLaEscalaEsLaDeSiempreQueCabe() {
        // 10 m x 6 m en 400 x 250 mm de papel: 1:25 justo no, 1:50 si.
        XCTAssertEqual(CuentasLamina.escalaQueCabe(anchoMm: 10_000, altoMm: 6000, papelAncho: 400, papelAlto: 250), 25)
        XCTAssertEqual(CuentasLamina.escalaQueCabe(anchoMm: 12_000, altoMm: 6000, papelAncho: 400, papelAlto: 250), 50)
        XCTAssertEqual(CuentasLamina.escalaQueCabe(anchoMm: 100, altoMm: 50, papelAncho: 400, papelAlto: 250), 1)
        // Casos negativos: sin papel o mas grande que la 1:100000.
        XCTAssertNil(CuentasLamina.escalaQueCabe(anchoMm: 100, altoMm: 50, papelAncho: 0, papelAlto: 250))
        XCTAssertNil(CuentasLamina.escalaQueCabe(anchoMm: 1e12, altoMm: 1, papelAncho: 400, papelAlto: 250))
    }

    func testLasUnidadesDelPlano() {
        XCTAssertEqual(CuentasLamina.mmPorUnidad(6), 1000)
        XCTAssertEqual(CuentasLamina.mmPorUnidad(4), 1)
        XCTAssertEqual(CuentasLamina.unidad(5), "cm")
        // Sin decir: sin escala y sin unidad.
        XCTAssertNil(CuentasLamina.mmPorUnidad(0))
        XCTAssertEqual(CuentasLamina.unidad(0), "")
        XCTAssertNil(CuentasLamina.escala(unidades: 0, marco: CajaPlano(x0: 0, y0: 0, x1: 1, y1: 1), aw: 100, ah: 100))
        let e = CuentasLamina.escala(unidades: 6, marco: CajaPlano(x0: 0, y0: 0, x1: 10, y1: 6), aw: 400 * CuentasLamina.mm,
                                     ah: 250 * CuentasLamina.mm)
        XCTAssertEqual(e?.n, 25)
        // 1 m a 1:25 son 40 mm de papel.
        XCTAssertEqual(e?.puntos ?? 0, 40 * CuentasLamina.mm, accuracy: 1e-9)
    }

    func testLaFechaYElProyecto() throws {
        var c = DateComponents()
        c.year = 2026; c.month = 10; c.day = 9; c.hour = 12
        let f = try XCTUnwrap(Calendar(identifier: .gregorian).date(from: c))
        XCTAssertEqual(CuentasLamina.hoy(f), "9/10/2026")
        let raiz = try carpeta("proyecto")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let archivos = raiz.appendingPathComponent("p1/archivos", isDirectory: true)
        try FileManager.default.createDirectory(at: archivos, withIntermediateDirectories: true)
        try Data(#"{"nombre":"Casa Lopez"}"#.utf8).write(to: raiz.appendingPathComponent("p1/proyecto.json"))
        XCTAssertEqual(CuentasLamina.proyecto(de: archivos.appendingPathComponent("planta.dwg").path), "Casa Lopez")
        // Caso negativo: fuera de un proyecto, el nombre de su carpeta.
        XCTAssertEqual(CuentasLamina.proyecto(de: raiz.appendingPathComponent("suelto/planta.dwg").path), "suelto")
    }

    func testLaHojaConLaminaDejaSitioAlMembrete() {
        let papel = CGRect(x: 0, y: 0, width: 1190, height: 842)
        let c = CuentasLamina.cajas(papel: papel, imprimible: papel.insetBy(dx: 12, dy: 12))
        XCTAssertTrue(papel.contains(c.recuadro))
        XCTAssertTrue(c.recuadro.contains(c.membrete))
        XCTAssertLessThan(c.plano.maxY, c.membrete.minY)
        XCTAssertEqual(c.membrete.maxX, c.recuadro.maxX, accuracy: 1e-6)
        // Lo que la impresora no alcanza manda sobre el margen de la norma.
        let estrecho = CuentasLamina.cajas(papel: papel, imprimible: papel.insetBy(dx: 100, dy: 100))
        XCTAssertGreaterThanOrEqual(estrecho.recuadro.minX, 100)
    }

    func testLaHojaDelPdfTieneElLadoLargoDeUnA3() {
        let t = ExportarPlanoIPad.tamanoDeHoja(CajaPlano(x0: 0, y0: 0, x1: 200, y1: 100), lado: 1190.55, margen: 28)
        XCTAssertEqual(t.width, 1190.55, accuracy: 1e-6)
        XCTAssertEqual(t.height, (1190.55 - 56) / 2 + 56, accuracy: 1e-6)
        XCTAssertEqual(ExportarPlanoIPad.base("planta/baja:1.dwg"), "planta-baja-1")
        XCTAssertEqual(ExportarPlanoIPad.base(""), "Plano")
    }

    func testElAvisoDeCivil3DCuentaSoloLoGuardadoSinDibujo() {
        let l = [NoDibujado(tipo: "sin dibujo: AeccDbSurfaceTin (AeccLand130)", cuantos: 3),
                 NoDibujado(tipo: "sin dibujo: AeccDbAlignment", cuantos: 2),
                 NoDibujado(tipo: "fuente que falta: romans.shx", cuantos: 7)]
        XCTAssertEqual(VisorPlano.cuantosSinDibujo(l), 5)
        // Caso negativo: solo letras que faltan.
        XCTAssertEqual(VisorPlano.cuantosSinDibujo([l[2]]), 0)
    }
}

final class PlanosMarcadoresTests: XCTestCase {
    @MainActor
    func testLosMarcadoresVanEnElFicheroDeWindowsYConservanLoDemas() throws {
        let c = try carpeta("vistas")
        defer { try? FileManager.default.removeItem(at: c) }
        let f = c.appendingPathComponent("anot-x.vistas.json")
        try Data(#"{"version":1,"plano":[],"modelo":[],"otro":"se queda"}"#.utf8).write(to: f)
        let m = MarcadoresDeVista(fichero: f)
        m.poner(x: 1, y: 2, ancho: 30, alto: 20)
        let e: [String: Any] = ["objetivo": [1.0, 2.0, 3.0], "rumbo": 0.5, "altura": 0.3, "dist": 10.0]
        let id = m.poner3d(estado: e)
        let otra = MarcadoresDeVista(fichero: f)
        XCTAssertEqual(otra.lista.count, 1)
        XCTAssertEqual(otra.lista.first?.ancho, 30)
        XCTAssertEqual(otra.del3d.map(\.id), [id])
        let o = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: f)) as? [String: Any])
        XCTAssertEqual(o["otro"] as? String, "se queda")
        // Quitar deja la lista vacia escrita (no borra el fichero).
        otra.quitar3d(id)
        otra.quitar(try XCTUnwrap(otra.lista.first))
        let vacio = MarcadoresDeVista(fichero: f)
        XCTAssertTrue(vacio.lista.isEmpty)
        XCTAssertTrue(vacio.del3d.isEmpty)
        XCTAssertTrue(FileManager.default.fileExists(atPath: f.path))
    }

    func testLaCamaraDeUnEstado3dYSiEsLaMisma() throws {
        let e: [String: Any] = ["objetivo": [1.0, 2.0, 3.0], "rumbo": 0.5, "altura": 0.3, "dist": 10.0]
        let c = try XCTUnwrap(camaraDeEstado3d(e))
        XCTAssertEqual(c.objetivo.z, 3)
        XCTAssertTrue(mismaCamara3d(c, c))
        var otra = c
        otra.rumbo += 0.01
        XCTAssertFalse(mismaCamara3d(c, otra))
        // Caso negativo: sin objetivo no hay camara.
        XCTAssertNil(camaraDeEstado3d(["rumbo": 1.0]))
    }

    @MainActor
    func testElRecuadroDeUnMarco3DVaYVuelve() {
        let t = CGSize(width: 1000, height: 500)
        let caja = Marcos3D.caja(CGPoint(x: 600, y: 100), CGPoint(x: 400, y: 300), en: t)
        XCTAssertEqual(caja[0], -0.2, accuracy: 1e-9)
        XCTAssertEqual(caja[3], -0.1, accuracy: 1e-9)
        let r = Marcos3D.rect(caja, en: t)
        XCTAssertEqual(r.minX, 400, accuracy: 1e-9)
        XCTAssertEqual(r.height, 200, accuracy: 1e-9)
        // Caso negativo: una caja mal formada no tiene recuadro.
        XCTAssertEqual(Marcos3D.rect([1, 2], en: t), .zero)
    }
}

final class PlanosPapelMetalTests: XCTestCase {
    @MainActor
    func testElPdfYLaImagenDeUnPlano() throws {
        guard let m = MotorMetal(dispositivo: MTLCreateSystemDefaultDevice()) else { throw XCTSkip("Sin tarjeta con Metal") }
        let c = try carpeta("pdf")
        defer { try? FileManager.default.removeItem(at: c) }
        let ruta = c.appendingPathComponent("planta.dxf")
        try dxfEnMetros.write(to: ruta, atomically: true, encoding: .utf8)
        let nucleo = try Nucleo.abrir(raiz: c.appendingPathComponent("datos").path, nombreDelAparato: "iPad")
        let d = try PlanoEnTarjeta.cargar(ruta: ruta.path, nucleo: nucleo, motor: m)
        let visibles = Set(d.plano.gruposVisibles())
        let marcos = [CajaPlano(x0: 0, y0: 0, x1: 5, y1: 6), CajaPlano(x0: 5, y0: 0, x1: 10, y1: 6)]
        let pdf = try XCTUnwrap(ExportarPlanoIPad.pdf(d, visibles: visibles, marcos: marcos, util: d.resumen.cajaUtil,
                                                      encima: EncimaPapel()))
        XCTAssertEqual(String(decoding: pdf.prefix(4), as: UTF8.self), "%PDF")
        let doc = try XCTUnwrap(CGPDFDocument(CGDataProvider(data: pdf as CFData)!))
        XCTAssertEqual(doc.numberOfPages, 2)
        let img = try XCTUnwrap(ExportarPlanoIPad.imagen(d, visibles: visibles, region: d.resumen.cajaUtil, encima: nil))
        XCTAssertEqual(max(img.width, img.height), 3200)
        // Casos negativos: una region vacia no da nada; un DXF no es un DWG.
        XCTAssertNil(ExportarPlanoIPad.pdf(d, visibles: visibles, marcos: [CajaPlano(x0: 1, y0: 1, x1: 1, y1: 1)],
                                           util: CajaPlano(x0: 1, y0: 1, x1: 1, y1: 1), encima: nil))
        XCTAssertFalse(d.plano.esDwg())
        XCTAssertTrue(try d.plano.buscarTextos(consulta: "no hay textos", max: 5).isEmpty)
    }
}
