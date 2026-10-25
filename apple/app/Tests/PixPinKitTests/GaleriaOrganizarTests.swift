// La galeria de capturas y su caducidad en el iPad (de las pruebas del
// Mac): conservar, «7 dias mas», borrar a la papelera de PixPin y
// deshacer, las paginas, los dias de la rejilla y el barrendero de lo
// caducado (que en el iPad va a la papelera de PixPin, no a la del sistema).
// Cruzan el puente de verdad sobre carpetas temporales.

import CoreGraphics
import Foundation
import ImageIO
import PixPinCore
@testable import PixPinKit
import UniformTypeIdentifiers
import XCTest

func carpetaTemporalOrganizar(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-ipad-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

@MainActor
func almacenTemporalOrganizar(_ nombre: String) throws -> (Almacen, URL) {
    let raiz = try carpetaTemporalOrganizar(nombre)
    let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "iPad de prueba")
    return (almacen, raiz)
}

private func escribirPNGOrganizar(_ url: URL, ancho: Int, alto: Int) throws {
    let ctx = try XCTUnwrap(CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8,
                                      bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    ctx.setFillColor(CGColor(srgbRed: 0.9, green: 0.3, blue: 0.2, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: ancho, height: alto))
    let imagen = try XCTUnwrap(ctx.makeImage())
    let destino = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil))
    CGImageDestinationAddImage(destino, imagen, nil)
    XCTAssertTrue(CGImageDestinationFinalize(destino))
}

@MainActor
final class GaleriaOrganizarTests: XCTestCase {
    func testLaGaleriaConservaAlargaBorraYDeshace() async throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("galeria")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let capturas = URL(fileURLWithPath: almacen.nucleo.carpetaCapturas(), isDirectory: true)
        try FileManager.default.createDirectory(at: capturas, withIntermediateDirectories: true)
        for n in ["captura-a.png", "captura-b.png", "captura-c.png"] {
            try escribirPNGOrganizar(capturas.appendingPathComponent(n), ancho: 64, alto: 48)
        }
        try Data("no".utf8).write(to: capturas.appendingPathComponent("notas.txt"))

        let modelo = ModeloGaleria(almacen: almacen)
        modelo.dias = { 7 }
        await modelo.recargar()
        XCTAssertNil(modelo.error)
        XCTAssertEqual(modelo.total, 3, "el txt no es una captura")
        XCTAssertTrue(modelo.capturas.allSatisfy { $0.diasQueQuedan == 7 && !$0.conservada })
        // La miniatura de la rejilla sale de la cache comun, reducida.
        let primera = try XCTUnwrap(modelo.capturas.first)
        let mini = await Miniaturas.compartidas.cargar(modelo.url(de: primera), lado: MiniaturaCaptura.lado)
        XCTAssertNotNil(mini)
        // Un dia, un grupo.
        XCTAssertEqual(modelo.grupos.count, 1)
        XCTAssertTrue(ModeloGaleria.rotulo(Date()).hasPrefix("Hoy · "))

        await modelo.conservar(["captura-a.png"])
        XCTAssertNil(modelo.error)
        XCTAssertTrue(try XCTUnwrap(modelo.capturas.first { $0.nombre == "captura-a.png" }).conservada)
        await modelo.conservar(["captura-a.png"])
        XCTAssertEqual(modelo.aviso, "Ya estaban conservadas")

        let antes = try XCTUnwrap(modelo.capturas.first { $0.nombre == "captura-b.png" }?.seVa)
        await modelo.prorrogar(["captura-b.png"])
        let despues = try XCTUnwrap(modelo.capturas.first { $0.nombre == "captura-b.png" }?.seVa)
        XCTAssertEqual(despues, antes + 7 * 86_400_000)

        await modelo.borrar(["captura-b.png", "captura-c.png"])
        XCTAssertEqual(modelo.total, 1)
        let papelera = raiz.appendingPathComponent("datos/papelera/capturas/captura-b.png")
        XCTAssertTrue(FileManager.default.fileExists(atPath: papelera.path))
        await modelo.deshacer()
        XCTAssertEqual(modelo.total, 3)
        XCTAssertTrue(modelo.ultimoBorrado.isEmpty)
        // Caso negativo: deshacer sin nada borrado no hace nada.
        await modelo.deshacer()
        XCTAssertEqual(modelo.total, 3)
    }

    func testLaGaleriaLlegaPorPaginas() async throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("galeria-paginas")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let capturas = URL(fileURLWithPath: almacen.nucleo.carpetaCapturas(), isDirectory: true)
        try FileManager.default.createDirectory(at: capturas, withIntermediateDirectories: true)
        let cuantas = Int(ModeloGaleria.tamPagina) + 5
        for i in 0..<cuantas {
            try Data([0x89, 0x50, 0x4e, 0x47]).write(to: capturas.appendingPathComponent(String(format: "c-%04d.png", i)))
        }
        let modelo = ModeloGaleria(almacen: almacen)
        await modelo.recargar()
        XCTAssertEqual(modelo.capturas.count, Int(ModeloGaleria.tamPagina))
        XCTAssertTrue(modelo.quedanMas)
        await modelo.cargarMas()
        XCTAssertEqual(modelo.capturas.count, cuantas)
        XCTAssertFalse(modelo.quedanMas)
    }

    func testLaInsigniaCambiaDeColorAlAcercarseElDia() {
        XCTAssertEqual(InsigniaCaducidad.color(dias: 0), Paleta.v2.rojo)
        XCTAssertEqual(InsigniaCaducidad.color(dias: 2), Paleta.v2.naranja)
        XCTAssertNotEqual(InsigniaCaducidad.color(dias: 6), Paleta.v2.rojo)
        XCTAssertTrue(InsigniaCaducidad.pista(dias: 1).contains("mañana"))
    }
}

// MARK: - Lo caducado, a la papelera de PixPin

final class GaleriaCaducidadTests: XCTestCase {
    private func viejo(_ url: URL, dias: Double) throws {
        let fecha = Date().addingTimeInterval(-dias * 86_400)
        try FileManager.default.setAttributes([.modificationDate: fecha], ofItemAtPath: url.path)
    }

    /// Tres capturas: una vieja, una vieja pero conservada y una de hoy.
    private func preparar(_ nombre: String) throws -> (Nucleo, URL, URL) {
        let raiz = try carpetaTemporalOrganizar(nombre)
        let datos = raiz.appendingPathComponent("datos", isDirectory: true)
        let nucleo = try Nucleo.abrir(raiz: datos.path, nombreDelAparato: "iPad de prueba")
        let capturas = URL(fileURLWithPath: nucleo.carpetaCapturas(), isDirectory: true)
        try FileManager.default.createDirectory(at: capturas, withIntermediateDirectories: true)
        for n in ["vieja.png", "guardada.png", "nueva.png"] {
            try Data("png".utf8).write(to: capturas.appendingPathComponent(n))
        }
        try viejo(capturas.appendingPathComponent("vieja.png"), dias: 8)
        try viejo(capturas.appendingPathComponent("guardada.png"), dias: 30)
        let registro = #"{"desde": 0, "conservadas": ["guardada.png", "borrada-a-mano.png"]}"#
        try Data(registro.utf8).write(to: datos.appendingPathComponent("capturas-caducidad.json"))
        return (nucleo, raiz, capturas)
    }

    func testLasCaducadasVanALaPapeleraDePixPinYLasConservadasSeQuedan() throws {
        let (nucleo, raiz, capturas) = try preparar("barrer")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let barrendero = BarrenderoCapturas(nucleo: nucleo)
        barrendero.dias = { 7 }
        barrendero.avisar = { _ in }
        let hecho = barrendero.barrer()
        XCTAssertEqual(hecho.idas, ["vieja.png"])
        XCTAssertTrue(hecho.fallos.isEmpty)
        XCTAssertFalse(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("vieja.png").path))
        let papelera = raiz.appendingPathComponent("datos/papelera/capturas/vieja.png")
        XCTAssertTrue(FileManager.default.fileExists(atPath: papelera.path), "a la papelera de PixPin")
        XCTAssertTrue(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("guardada.png").path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("nueva.png").path))
        let pagina = try nucleo.galeria(desde: 0, cuantos: 10, dias: 7, desfaseMs: 0)
        XCTAssertEqual(Set(pagina.capturas.map(\.nombre)), ["guardada.png", "nueva.png"])
        // Otra vez: no queda nada que barrer.
        XCTAssertTrue(barrendero.barrer().idas.isEmpty)
    }

    func testConElAjusteACeroNoSeVaNinguna() throws {
        let (nucleo, raiz, capturas) = try preparar("barrer-cero")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let barrendero = BarrenderoCapturas(nucleo: nucleo)
        barrendero.dias = { 0 }
        barrendero.tirar = { _ in
            XCTFail("no tenia que tirar nada")
            return []
        }
        XCTAssertTrue(barrendero.barrer().idas.isEmpty)
        XCTAssertTrue(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("vieja.png").path))
        barrendero.dias = { 30 }
        XCTAssertTrue(barrendero.barrer().idas.isEmpty)
    }

    func testElPlazoSaleDePixpinmaxToml() throws {
        let (nucleo, raiz, _) = try preparar("barrer-toml")
        defer { try? FileManager.default.removeItem(at: raiz) }
        var tres = nucleo.ajustesDeFabrica()
        tres.diasCaducidad = 3
        try nucleo.guardarAjustes(nuevos: tres)
        XCTAssertEqual(BarrenderoCapturas(nucleo: nucleo).dias(), 3)
        XCTAssertEqual(AjusteCaducidad.dias, 3, "lo leido se queda para la galeria")
    }

    func testElAvisoDeLasQueSeVan() throws {
        XCTAssertNil(AvisoCaducidad.aviso(de: [], ahora: Date()))
        let una = CapturaCaducada(nombre: "a.png", ruta: "/tmp/a.png", seVa: Int64(Date().timeIntervalSince1970 * 1000))
        let a = try XCTUnwrap(AvisoCaducidad.aviso(de: [una], ahora: Date()))
        XCTAssertEqual(a.categoria, .captura)
        XCTAssertEqual(a.datos[AvisoCaducidad.claveNombre], "a.png")
        let varias = try XCTUnwrap(AvisoCaducidad.aviso(de: [una, una], ahora: Date()))
        XCTAssertEqual(varias.categoria, .capturas)
        XCTAssertEqual(CategoriaAviso.captura.acciones, [.conservar, .abrir])
    }
}
