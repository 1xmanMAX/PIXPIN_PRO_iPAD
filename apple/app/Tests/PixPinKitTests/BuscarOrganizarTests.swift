// Buscar en todo en el iPad: espera, cancela lo de antes, por paginas, y
// cada «ventana» de Windows es una seccion del iPad.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

@MainActor
final class BuscarOrganizarTests: XCTestCase {
    func testBuscarEsperaCancelaLoAnteriorYVaPorPaginas() async throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("buscar")
        defer { try? FileManager.default.removeItem(at: raiz) }
        await almacen.crearChat(nombre: "Obra")
        let chat = try XCTUnwrap(almacen.chats.first { $0.nombre == "Obra" }).id
        for i in 0..<60 {
            _ = try almacen.nucleo.escribirNota(chat: chat, texto: "nota \(i)", respondeA: nil)
        }
        let nota = try almacen.nucleo.escribirNota(chat: chat, texto: "Falta el yeso de la cocina", respondeA: nil)

        let modelo = ModeloBuscar(almacen: almacen)
        modelo.cambiar("cocinero")
        let primera = try XCTUnwrap(modelo.tarea)
        modelo.cambiar("yeso cocina")
        XCTAssertTrue(primera.isCancelled)
        _ = await modelo.tarea?.value
        let r = try XCTUnwrap(modelo.resultados.first { $0.clase == .mensaje })
        XCTAssertEqual(r.mensaje, nota.id)
        XCTAssertEqual(r.chat, chat)
        XCTAssertTrue(tieneVerEnChat(.resultado(r)), "«Abrir en Proyectos»")

        await modelo.buscarAhora("nota")
        XCTAssertEqual(modelo.resultados.count, Int(ModeloBuscar.tamPagina))
        XCTAssertTrue(modelo.hayMas)
        await modelo.cargarMas()
        XCTAssertEqual(modelo.resultados.count, 60)
        XCTAssertFalse(modelo.hayMas)
    }

    func testCadaVentanaEsUnaSeccion() {
        XCTAssertEqual(VistaBuscar.seccion(de: IdVentana.galeria), .galeria)
        XCTAssertEqual(VistaBuscar.seccion(de: IdVentana.tareas), .tareas)
        XCTAssertEqual(VistaBuscar.seccion(de: "lecciones"), .lecciones)
        XCTAssertNil(VistaBuscar.seccion(de: "ventana-que-no-hay"))
    }

    func testLaFechaCortaDiceHoyYAyer() {
        let ahora = Int64(Date().timeIntervalSince1970 * 1000)
        XCTAssertTrue(fechaCorta(ahora).hasPrefix("hoy "))
        XCTAssertEqual(fechaCorta(ahora - 86_400_000), "ayer")
    }
}
