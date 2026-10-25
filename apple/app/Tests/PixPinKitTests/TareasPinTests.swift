// El pin de una tarea y lo que se ve mientras se escribe una: la vista de
// como quedara («Urgente · #casa · ⏰ …»), la nota de «¿Por qué no ahora?»
// dentro de la tarea y lo que se guarda de los pines abiertos.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

@MainActor
final class TareasPinTests: XCTestCase {
    func testComoQuedaraJuntaLasMarcas() {
        XCTAssertEqual(ComoQuedaraTarea.vista(titulo: "comprar yeso", urgencia: "urgente", etiquetas: ["casa"],
                                              hora: "18:00", escrito: "comprar yeso urgente #casa a las 6"),
                       "comprar yeso · Urgente · #casa · ⏰ 18:00")
        XCTAssertEqual(ComoQuedaraTarea.vista(titulo: "x", urgencia: "muy_urgente", etiquetas: [], hora: nil, escrito: "x"),
                       "x · Muy urgente")
        // Sin marcas no se dice nada (se ve en la caja), ni vacio.
        XCTAssertNil(ComoQuedaraTarea.vista(titulo: "pan", urgencia: nil, etiquetas: [], hora: nil, escrito: "pan"))
        XCTAssertNil(ComoQuedaraTarea.vista("   "))
    }

    func testComoQuedaraConElNucleo() throws {
        let v = try XCTUnwrap(ComoQuedaraTarea.vista("llamar al fontanero #casa"))
        XCTAssertTrue(v.contains("#casa"), v)
        XCTAssertTrue(v.hasPrefix("llamar al fontanero"), v)
    }

    func testLaNotaDelPinVaDentroDeLaTarea() async throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("pin-tarea")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let modelo = ModeloTareas(nucleo: almacen.nucleo)
        await modelo.apuntar("revisar planos")
        let lista = try XCTUnwrap(modelo.listas.first)
        let n = almacen.nucleo
        let nuevo = try n.ponerNotaTarea(chat: lista.chat, lista: lista.mensaje, titulo: "revisar planos", nota: "falta el plotter")
        XCTAssertNotNil(nuevo)
        let t = try XCTUnwrap(n.tareaPorTitulo(chat: lista.chat, lista: lista.mensaje, titulo: "revisar planos"))
        XCTAssertEqual(marcasDeTarea(texto: t.crudo, desfaseMs: 0).nota, "falta el plotter")
        // Caso negativo: una tarea que no esta.
        XCTAssertNil(try n.tareaPorTitulo(chat: lista.chat, lista: lista.mensaje, titulo: "no existe"))
    }

    func testLoQueSeGuardaDeUnPin() throws {
        let g = PinesDeTareasIPad.Guardado(chat: "c", lista: "l", titulo: "t", x: 10, y: 20)
        let d = try JSONEncoder().encode([g])
        XCTAssertEqual(try JSONDecoder().decode([PinesDeTareasIPad.Guardado].self, from: d), [g])
        XCTAssertEqual(ModeloPinTarea.clave(chat: "c", lista: "l", titulo: "t"), "c/l/t")
    }
}
