// El timeline en el iPad por su modelo (el del Mac): los dias y sus
// momentos por paginas, apuntar, mandar a «Momentos», borrar y deshacer.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

@MainActor
final class TimelineOrganizarTests: XCTestCase {
    func testElTimelineDaLosDiasYSusMomentos() async throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("timeline")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let carpeta = raiz.appendingPathComponent("datos/timeline", isDirectory: true)
        try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        let ahora = Int64(Date().timeIntervalSince1970 * 1000)
        let hace10Dias = ahora - 10 * 86_400_000
        let lineas = [
            #"{"id":"tl-1-0","cuando":\#(ahora - 60_000),"titulo":"Llegó el yeso","descripcion":"diez sacos"}"#,
            #"{"id":"tl-2-0","cuando":\#(ahora - 30_000),"titulo":"Reunión","enMomentos":true}"#,
            #"{"id":"tl-3-0","cuando":\#(hace10Dias),"titulo":"Antiguo"}"#,
            #"{"id":"roto"#,
        ]
        try (lineas.joined(separator: "\n") + "\n").write(to: carpeta.appendingPathComponent("momentos.jsonl"),
                                                          atomically: true, encoding: .utf8)

        let modelo = ModeloTimeline(nucleo: almacen.nucleo)
        await modelo.recargar()
        XCTAssertNil(modelo.error)
        XCTAssertEqual(modelo.modo, .pestana(.hoy))
        XCTAssertEqual(modelo.momentos.map(\.titulo), ["Llegó el yeso", "Reunión"])
        XCTAssertEqual(modelo.dias.count, 2)
        let viejo = try XCTUnwrap(modelo.dias.last)
        await modelo.elegir(.dia(viejo.iso))
        XCTAssertEqual(modelo.modo, .dia(viejo.iso))
        XCTAssertEqual(modelo.momentos.map(\.titulo), ["Antiguo"])
        await modelo.volverDelDia()
        XCTAssertEqual(modelo.modo, .pestana(.hoy))
        // Solo «Momentos».
        modelo.filtro = .momentos
        await modelo.recargar()
        XCTAssertEqual(modelo.dias.count, 1)
        // Caso negativo: una fecha que no es una fecha.
        XCTAssertNil(ModeloTimeline.partes("ayer"))
        XCTAssertEqual(ModeloTimeline.partes("2026-10-15")?.2, 15)
    }

    func testApuntarMandarABorrarYDeshacer() async throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("timeline-apuntar")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let modelo = ModeloTimeline(nucleo: almacen.nucleo)
        await modelo.recargar()
        XCTAssertTrue(modelo.momentos.isEmpty)

        modelo.texto = "Hormigonado el forjado\ncon la bomba grande"
        let hecho = await modelo.enviar()
        XCTAssertTrue(hecho)
        XCTAssertEqual(modelo.texto, "")
        let m = try XCTUnwrap(modelo.momentos.last)
        XCTAssertEqual(m.titulo, "Hormigonado el forjado")
        XCTAssertFalse(m.enMomentos)

        await modelo.alternarMomentos(m)
        XCTAssertTrue(try XCTUnwrap(modelo.momentos.last).enMomentos)

        await modelo.borrar(try XCTUnwrap(modelo.momentos.last))
        XCTAssertTrue(modelo.momentos.isEmpty)
        XCTAssertNotNil(modelo.deshacer)
        await modelo.deshacerBorrado()
        XCTAssertEqual(modelo.momentos.map(\.titulo), ["Hormigonado el forjado"])

        // Caso negativo: la caja vacia no apunta nada.
        modelo.texto = "   "
        let nada = await modelo.enviar()
        XCTAssertFalse(nada)
        XCTAssertEqual(modelo.momentos.count, 1)
    }

    func testLaDuracionDeUnAudio() {
        XCTAssertEqual(TarjetaMomento.duracion(42_000), "0:42")
        XCTAssertEqual(TarjetaMomento.duracion(125_000), "2:05")
        XCTAssertEqual(TarjetaMomento.duracion(-5), "0:00")
    }
}
