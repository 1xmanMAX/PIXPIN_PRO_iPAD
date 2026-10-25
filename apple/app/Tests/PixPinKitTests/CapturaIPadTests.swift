// Pruebas de la captura del iPad: el nombre de la galeria (el de Windows),
// el orden de «Al chat», las marcas del editor y que lo anotado se quema de
// verdad en la imagen (con su mosaico).

import CoreGraphics
import Foundation
@testable import PixPinKit
import XCTest

final class CapturaIPadTests: XCTestCase {
    private let utc = TimeZone(identifier: "UTC")!

    // MARK: Nombres

    func testElNombreDeLaGaleriaEsElDeWindows() {
        let fecha = Date(timeIntervalSince1970: 1_761_400_000) // 2025-10-25 13:46:40 UTC
        XCTAssertEqual(NombresCaptura.nombreDeCaptura(fecha: fecha, zona: utc), "captura-20251025-134640")
        let carpeta = URL(fileURLWithPath: "/tmp/capturas", isDirectory: true)
        let libre = NombresCaptura.rutaNueva(en: carpeta, fecha: fecha, zona: utc, existe: { _ in false })
        XCTAssertEqual(libre.lastPathComponent, "captura-20251025-134640.png")
        // En ese segundo ya hay una (aunque sea un JPEG): -2, y luego -3.
        let ocupadas: Set<String> = ["captura-20251025-134640.jpg", "captura-20251025-134640-2.png"]
        let otra = NombresCaptura.rutaNueva(en: carpeta, fecha: fecha, zona: utc,
                                            existe: { ocupadas.contains($0.lastPathComponent) })
        XCTAssertEqual(otra.lastPathComponent, "captura-20251025-134640-3.png")
    }

    func testAlChatPoneElUltimoPrimeroYBuscaSinTildes() {
        let nombres = ["Mensajes guardados", "Tesis", "Obra Miraflores", "Compras temu"]
        XCTAssertEqual(ListaDeProyectos.ordenar(nombres, id: { $0 }, ultimo: "Obra Miraflores"),
                       ["Obra Miraflores", "Mensajes guardados", "Tesis", "Compras temu"])
        XCTAssertEqual(ListaDeProyectos.ordenar(nombres, id: { $0 }, ultimo: "No existe"), nombres)
        XCTAssertEqual(ListaDeProyectos.filtrar(nombres, nombre: { $0 }, texto: "TÉSIS"), ["Tesis"])
        XCTAssertEqual(ListaDeProyectos.filtrar(nombres, nombre: { $0 }, texto: " "), nombres)
        XCTAssertEqual(ListaDeProyectos.nombreDeFichero(fecha: Date(timeIntervalSince1970: 1.5)), "captura-1500.png")
    }

    // MARK: Marcas

    func testUnaMarcaSinCuerpoNoSeGuarda() {
        let toque = MarcaCaptura(herramienta: .flecha, color: 0, grosor: 3, puntos: [CGPoint(x: 5, y: 5), CGPoint(x: 6, y: 6)])
        XCTAssertFalse(toque.tieneCuerpo)
        let flecha = MarcaCaptura(herramienta: .flecha, color: 0, grosor: 3, puntos: [.zero, CGPoint(x: 30, y: 0)])
        XCTAssertTrue(flecha.tieneCuerpo)
        let caja = MarcaCaptura(herramienta: .rectangulo, color: 0, grosor: 3, puntos: [CGPoint(x: 40, y: 30), CGPoint(x: 10, y: 10)])
        XCTAssertEqual(caja.caja, CGRect(x: 10, y: 10, width: 30, height: 20), "da igual hacia donde se arrastre")
        XCTAssertFalse(MarcaCaptura(herramienta: .texto, color: 0, grosor: 3, puntos: [.zero], texto: "  ").tieneCuerpo)
        XCTAssertTrue(MarcaCaptura(herramienta: .lapiz, color: 0, grosor: 3, puntos: [.zero]).tieneCuerpo)
    }

    func testLasHerramientasVanEnElOrdenDeWindows() {
        XCTAssertEqual(HerramientaCaptura.allCases.map(\.tecla), ["1", "2", "3", "4", "5"])
        XCTAssertEqual(HerramientaCaptura.allCases.map(\.nombre), ["Lápiz", "Flecha", "Rectángulo", "Texto", "Mosaico"])
    }

    // MARK: Quemar

    func testLoAnotadoSeQuemaEnLaImagen() throws {
        let blanca = try XCTUnwrap(PinesIPadTests.foto(ancho: 100, alto: 80))
        let rojo = ColoresCaptura.todos[0]
        let marca = MarcaCaptura(herramienta: .rectangulo, color: rojo, grosor: 6,
                                 puntos: [CGPoint(x: 20, y: 20), CGPoint(x: 80, y: 60)])
        let quemada = PintorMarcas.quemar(blanca, marcas: [marca], pixelada: nil)
        XCTAssertEqual(quemada.width, 100, "a los pixeles de la foto")
        XCTAssertEqual(quemada.height, 80)
        let borde = try XCTUnwrap(Self.pixel(quemada, x: 20, y: 40))
        XCTAssertGreaterThan(borde.r, 200)
        XCTAssertLessThan(borde.g, 120, "el borde es rojo")
        let dentro = try XCTUnwrap(Self.pixel(quemada, x: 50, y: 40))
        XCTAssertGreaterThan(dentro.g, 240, "dentro sigue blanco")
        XCTAssertTrue(PintorMarcas.quemar(blanca, marcas: [], pixelada: nil) === blanca, "sin marcas, la misma")
    }

    func testElMosaicoTapaLoDeDebajo() throws {
        // Medio negro y medio blanco: el mosaico de en medio queda gris.
        let foto = try XCTUnwrap(Self.mitades(ancho: 120, alto: 120))
        let pixelada = try XCTUnwrap(PintorMarcas.pixelar(foto))
        XCTAssertEqual(pixelada.width, 120)
        XCTAssertEqual(pixelada.height, 120)
        XCTAssertEqual(PintorMarcas.ladoMosaico(CGSize(width: 120, height: 120)), 8)
        XCTAssertEqual(PintorMarcas.ladoMosaico(CGSize(width: 4032, height: 3024)), 67)
        let marca = MarcaCaptura(herramienta: .mosaico, color: 0, grosor: 3,
                                 puntos: [CGPoint(x: 0, y: 0), CGPoint(x: 120, y: 120)])
        let quemada = PintorMarcas.quemar(foto, marcas: [marca], pixelada: pixelada)
        XCTAssertEqual(quemada.width, 120)
        // Fuera del mosaico nada cambia.
        let fuera = PintorMarcas.quemar(foto, marcas: [MarcaCaptura(herramienta: .mosaico, color: 0, grosor: 3,
                                                                     puntos: [CGPoint(x: 0, y: 0), CGPoint(x: 10, y: 10)])],
                                        pixelada: pixelada)
        let lejos = try XCTUnwrap(Self.pixel(fuera, x: 100, y: 60))
        XCTAssertGreaterThan(lejos.r, 240)
    }

    // MARK: Ayudas

    static func pixel(_ imagen: CGImage, x: Int, y: Int) -> (r: UInt8, g: UInt8, b: UInt8)? {
        let ancho = imagen.width, alto = imagen.height
        guard let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: ancho * 4,
                                  space: espacio, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let datos = ctx.data else { return nil }
        ctx.draw(imagen, in: CGRect(x: 0, y: 0, width: ancho, height: alto))
        let p = datos.bindMemory(to: UInt8.self, capacity: ancho * alto * 4)
        // En memoria la primera fila es la de arriba de la foto.
        let i = (y * ancho + x) * 4
        return (p[i], p[i + 1], p[i + 2])
    }

    static func mitades(ancho: Int, alto: Int) -> CGImage? {
        guard let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: espacio, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: ancho, height: alto))
        ctx.setFillColor(CGColor(red: 0, green: 0, blue: 0, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: ancho / 2, height: alto))
        return ctx.makeImage()
    }
}
