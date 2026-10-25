// Lo traido del Mac 62fe093: un chat compartido como conversacion
// (`Chat/ConversacionCompartida.swift`, Windows b89c058 y e3f1d60) y las
// cuentas de «Estudiar la clase» (`VozVentanas/EstudiarClase.swift`,
// Windows ce6c058), por el puente de verdad.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

private func carpetaTemporalConversacion(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-ipad-\(nombre)-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

@MainActor
final class ChatConversacionYClaseTests: XCTestCase {
    func testElChatCompartidoNaceConmigoDentroYLoMioVaALaDerecha() async throws {
        let raiz = try carpetaTemporalConversacion("conversacion-compartida")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "iPad de prueba")
        let nuevo = try almacen.nucleo.crearChatCompartido(nombre: "Tesis")
        XCTAssertTrue(nuevo.compartido)
        let chapas = try almacen.nucleo.chapasDeCompartidos()
        XCTAssertEqual(chapas.map(\.ficha), [nuevo.ficha.id])
        XCTAssertEqual(TextosCompartido.chapa(chapas[0].personas), "Compartido · 1 persona")
        XCTAssertEqual(TextosCompartido.chapa(3), "Compartido · 3 personas")

        let escrita = await almacen.escribir(nota: "hola", en: nuevo.ficha.id)
        let m = try XCTUnwrap(escrita)
        XCTAssertNotNil(m.aparato, "lo escrito aqui lleva el codigo de este aparato")
        let c = try XCTUnwrap(almacen.nucleo.conversacionDelChat(
            ficha: nuevo.ficha.id,
            mensajes: [AutoriaDeMensaje(id: m.id, cuando: m.cuando, aparato: m.aparato)],
            desfaseMs: desfaseLocalMs()))
        XCTAssertEqual(c.personas, 1)
        XCTAssertEqual(c.lados.first?.mio, true)
        XCTAssertNil(c.lados.first?.nombre)

        // Caso negativo: un chat normal no tiene lados, se ve como siempre.
        await almacen.crearChat(nombre: "Casa")
        let casa = try XCTUnwrap(almacen.chats.first { $0.nombre == "Casa" }).id
        XCTAssertNil(try almacen.nucleo.conversacionDelChat(ficha: casa, mensajes: [], desfaseMs: 0))
    }

    func testLosDosModosYLasCuentasDeEstudiar() {
        XCTAssertEqual(otroModoDeGrabar(modo: .pantalla), .capturas)
        XCTAssertEqual(otroModoDeGrabar(modo: .capturas), .pantalla)
        // Un toque cerca de una captura se queda en ella; lejos, donde se toco.
        XCTAssertEqual(clasePegarASenal(x: 51, ancho: 100, duracionMs: 60_000, senales: [30_000], tolerancia: 3), 30_000)
        XCTAssertEqual(clasePegarASenal(x: 60, ancho: 100, duracionMs: 60_000, senales: [30_000], tolerancia: 3), 36_000)
        XCTAssertTrue(ModeloEstudiar.esAMano(.interacting))
        // Caso negativo: el scroll que sigue al sonido no es mover a mano.
        XCTAssertFalse(ModeloEstudiar.esAMano(.animating))
        XCTAssertFalse(ModeloEstudiar.esAMano(.idle))
    }
}
