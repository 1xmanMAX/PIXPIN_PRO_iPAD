// Pruebas de lo nuevo de la captura del iPad: recortar (proporciones,
// medidas tecleadas, la ultima zona), el color de la lupa y la pila de
// capturas.

import CoreGraphics
import Foundation
@testable import PixPinKit
import XCTest

final class CapturaIPadMasTests: XCTestCase {
    private let foto = CGRect(x: 0, y: 0, width: 1000, height: 800)

    // MARK: Proporciones

    func testAtarALaProporcionMandaElLadoMasArrastrado() {
        let a = RecorteCaptura.atado(desde: CGPoint(x: 100, y: 100), hasta: CGPoint(x: 260, y: 120), proporcion: .p16x9)
        XCTAssertEqual(a, CGRect(x: 100, y: 100, width: 160, height: 90))
        // Hacia arriba a la izquierda.
        let b = RecorteCaptura.atado(desde: CGPoint(x: 300, y: 300), hasta: CGPoint(x: 290, y: 200), proporcion: .p1x1)
        XCTAssertEqual(b, CGRect(x: 200, y: 200, width: 100, height: 100))
        // Libre: el rectangulo entre los dos.
        let c = RecorteCaptura.atado(desde: CGPoint(x: 50, y: 60), hasta: CGPoint(x: 10, y: 100), proporcion: .libre)
        XCTAssertEqual(c, CGRect(x: 10, y: 60, width: 40, height: 40))
    }

    func testAjustarUnaZonaALaProporcion() {
        let z = RecorteCaptura.ajustar(CGRect(x: 100, y: 100, width: 400, height: 50), a: .p4x3, limites: foto)
        XCTAssertEqual(z, CGRect(x: 100, y: 100, width: 400, height: 300))
        // No cabe hacia abajo: manda el alto que cabe.
        let w = RecorteCaptura.ajustar(CGRect(x: 0, y: 700, width: 400, height: 50), a: .p1x1, limites: foto)
        XCTAssertEqual(w, CGRect(x: 0, y: 700, width: 100, height: 100))
        // Sin zona: la mayor centrada.
        let n = RecorteCaptura.ajustar(nil, a: .p16x9, limites: foto)
        XCTAssertEqual(n?.width, 1000)
        XCTAssertEqual(n?.height, 562.5)
        XCTAssertEqual(n?.midY, 400)
        XCTAssertNil(RecorteCaptura.ajustar(nil, a: .libre, limites: foto))
    }

    // MARK: Medidas

    func testLeerLasMedidasComoLasTecleaLaGente() {
        XCTAssertEqual(RecorteCaptura.leer("800x600")?.ancho, 800)
        XCTAssertEqual(RecorteCaptura.leer(" 800 × 600 ")?.alto, 600)
        XCTAssertEqual(RecorteCaptura.leer("1920*1080")?.ancho, 1920)
        XCTAssertNil(RecorteCaptura.leer("800"))
        XCTAssertNil(RecorteCaptura.leer("0x10"))
        XCTAssertNil(RecorteCaptura.leer("123456x10"))
        XCTAssertNil(RecorteCaptura.leer("-5x10"))
        XCTAssertEqual(RecorteCaptura.numero("42"), 42)
        XCTAssertNil(RecorteCaptura.numero("4a"))
    }

    func testLasMedidasDanLaZonaDentroDeLaFoto() {
        // Sin zona: centrada.
        XCTAssertEqual(RecorteCaptura.zona(ancho: 200, alto: 100, seleccion: nil, limites: foto),
                       CGRect(x: 400, y: 350, width: 200, height: 100))
        // Con zona: desde su esquina, sin salirse.
        XCTAssertEqual(RecorteCaptura.zona(ancho: 300, alto: 300, seleccion: CGRect(x: 900, y: 10, width: 50, height: 50),
                                           limites: foto),
                       CGRect(x: 700, y: 10, width: 300, height: 300))
        // Mas grande que la foto: la foto entera.
        XCTAssertEqual(RecorteCaptura.zona(ancho: 5000, alto: 5000, seleccion: nil, limites: foto), foto)
    }

    func testEnPixelesYDeslizar() {
        XCTAssertEqual(RecorteCaptura.enPixeles(CGRect(x: 10.4, y: 20.6, width: 99.2, height: 10), ancho: 1000, alto: 800),
                       CGRect(x: 10, y: 20, width: 100, height: 11))
        XCTAssertEqual(RecorteCaptura.enPixeles(CGRect(x: 990, y: 790, width: 50, height: 50), ancho: 1000, alto: 800),
                       CGRect(x: 990, y: 790, width: 10, height: 10))
        let r = RecorteCaptura.deslizar(CGRect(x: 900, y: 0, width: 200, height: 100), dx: 50, dy: -20, dentroDe: foto)
        XCTAssertEqual(r, CGRect(x: 800, y: 0, width: 200, height: 100))
        XCTAssertFalse(RecorteCaptura.valida(CGRect(x: 0, y: 0, width: 1, height: 100)))
        XCTAssertFalse(RecorteCaptura.valida(nil))
        XCTAssertTrue(RecorteCaptura.valida(CGRect(x: 0, y: 0, width: 2, height: 2)))
    }

    func testLasMarcasSeMuevenConElRecorte() {
        let m = MarcaCaptura(herramienta: .flecha, color: 0xFF0000, grosor: 3,
                             puntos: [CGPoint(x: 150, y: 220), CGPoint(x: 300, y: 400)])
        let movidas = RecorteCaptura.moverMarcas([m], recorte: CGRect(x: 100, y: 200, width: 10, height: 10))
        XCTAssertEqual(movidas[0].puntos, [CGPoint(x: 50, y: 20), CGPoint(x: 200, y: 200)])
        XCTAssertEqual(movidas[0].id, m.id)
    }

    func testLaUltimaZonaVuelveYSeQuedaDentro() {
        let defaults = UserDefaults(suiteName: "CapturaIPadMasTests")!
        defaults.removePersistentDomain(forName: "CapturaIPadMasTests")
        let u = UltimaZonaCaptura(defaults: defaults)
        XCTAssertNil(u.leer())
        u.guardar(CGRect(x: 600, y: 500, width: 600, height: 400))
        XCTAssertEqual(u.leer(), CGRect(x: 600, y: 500, width: 600, height: 400))
        // En una foto mas pequena, lo que cabe.
        XCTAssertEqual(u.leer(ancho: 1000, alto: 800), CGRect(x: 600, y: 500, width: 400, height: 300))
        // Fuera del todo: nada.
        XCTAssertNil(u.leer(ancho: 500, alto: 400))
        defaults.removePersistentDomain(forName: "CapturaIPadMasTests")
    }

    // MARK: Lupa

    func testElColorDeLaLupa() {
        XCTAssertEqual(RecorteCaptura.hex(255, 59, 48), "#FF3B30")
        XCTAssertEqual(RecorteCaptura.rgb(10, 132, 255), "RGB 10 132 255")
        XCTAssertNil(RecorteCaptura.pixel(en: CGPoint(x: 1000, y: 5), ancho: 1000, alto: 800))
        XCTAssertEqual(RecorteCaptura.pixel(en: CGPoint(x: 999.9, y: 0.2), ancho: 1000, alto: 800)?.x, 999)
        // En la esquina la lupa no se sale.
        XCTAssertEqual(RecorteCaptura.zonaDeLupa(en: CGPoint(x: 1, y: 799), lado: 15, ancho: 1000, alto: 800),
                       CGRect(x: 0, y: 785, width: 15, height: 15))
    }

    func testElColorSeLeeDelPixel() throws {
        // Una foto de 2 x 1: rojo y azul.
        let espacio = try XCTUnwrap(CGColorSpace(name: CGColorSpace.sRGB))
        var px: [UInt8] = [255, 0, 0, 255, 0, 0, 255, 255]
        let ctx = try XCTUnwrap(CGContext(data: &px, width: 2, height: 1, bitsPerComponent: 8, bytesPerRow: 8,
                                          space: espacio, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        let imagen = try XCTUnwrap(ctx.makeImage())
        XCTAssertEqual(ColorPixel.en(imagen, x: 0, y: 0)?.hex, "#FF0000")
        XCTAssertEqual(ColorPixel.en(imagen, x: 1, y: 0)?.hex, "#0000FF")
        XCTAssertNil(ColorPixel.en(imagen, x: 2, y: 0))
    }

    // MARK: Pila

    func testLaPilaApilaDurante10SegundosOArmada() {
        var t = TandaPila()
        let a = URL(fileURLWithPath: "/tmp/a.png"), b = URL(fileURLWithPath: "/tmp/b.png")
        XCTAssertTrue(t.meter(a, en: 0))
        XCTAssertFalse(t.meter(b, en: 5))
        XCTAssertEqual(t.cuantas, 2)
        // Pasados 10 s sin armar, empieza otra.
        XCTAssertTrue(t.meter(a, en: 20))
        XCTAssertEqual(t.cuantas, 1)
        t.armada = true
        XCTAssertFalse(t.meter(b, en: 500))
        XCTAssertEqual(t.cuantas, 2)
        t.vaciar()
        XCTAssertFalse(t.abierta(en: 500))
        for i in 0 ..< 30 { t.meter(URL(fileURLWithPath: "/tmp/\(i).png"), en: Double(i)) }
        XCTAssertEqual(t.cuantas, TandaPila.tope)
        XCTAssertEqual(t.ficheros.first?.lastPathComponent, "6.png")
        XCTAssertEqual(TandaPila.titulo(1), "1 captura")
        XCTAssertEqual(TandaPila.titulo(3), "3 capturas apiladas")
    }
}
