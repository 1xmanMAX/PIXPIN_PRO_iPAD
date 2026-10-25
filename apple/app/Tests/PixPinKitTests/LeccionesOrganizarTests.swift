// Lecciones en el iPad por su modelo (el del Mac): guardar una y
// encontrarla por paginas.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

@MainActor
final class LeccionesOrganizarTests: XCTestCase {
    func testUnaLeccionSeGuardaYSeEncuentra() async throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("lecciones")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let modelo = ModeloLecciones(nucleo: almacen.nucleo)
        await modelo.recargar()
        XCTAssertEqual(modelo.total, 0)

        var b = BorradorLeccion()
        b.titulo = "Revisar el replanteo antes de hormigonar"
        b.quePaso = "El muro salió torcido"
        let f = await modelo.guardar(b.editada)
        XCTAssertNotNil(f)
        await modelo.recargar()
        XCTAssertEqual(modelo.total, 1)
        XCTAssertEqual(modelo.lecciones.first?.titulo, "Revisar el replanteo antes de hormigonar")

        // Caso negativo: borrada, ya no sale.
        await modelo.borrar(try XCTUnwrap(f?.id))
        await modelo.recargar()
        XCTAssertEqual(modelo.total, 0)
    }
}
