// Las tareas de todos los chats en el iPad: apuntar en el Inbox, marcar en
// su mensaje, la urgencia y las etiquetas (que se escriben en el texto) y
// el recordatorio `⏰`.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

@MainActor
final class TareasOrganizarTests: XCTestCase {
    func testLasTareasSeApuntanYSeMarcanEnSuMensaje() async throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("tareas")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let modelo = ModeloTareas(nucleo: almacen.nucleo)
        await modelo.cargar()
        XCTAssertTrue(modelo.listas.isEmpty)

        await modelo.apuntar("comprar yeso")
        await modelo.apuntar("llamar al fontanero")
        XCTAssertNil(modelo.error)
        let inbox = try XCTUnwrap(modelo.listas.first)
        XCTAssertTrue(inbox.esInbox)
        XCTAssertEqual(inbox.tareas.map(\.texto), ["comprar yeso", "llamar al fontanero"])
        XCTAssertEqual(modelo.pendientesPorGrupo.map(\.grupo), [.hoy])

        let yeso = TareaConLista(lista: inbox, tarea: inbox.tareas[0])
        await modelo.marcar(yeso, hecha: true)
        XCTAssertEqual(modelo.hechas.map(\.tarea.texto), ["comprar yeso"])

        // Urgente y con etiqueta: se corrige el texto y lo entiende el nucleo.
        let fontanero = try XCTUnwrap(modelo.listas.first.map { TareaConLista(lista: $0, tarea: $0.tareas[1]) })
        await modelo.editar(fontanero, a: ModeloTareas.conEtiqueta(
            ModeloTareas.conUrgencia(fontanero.tarea.texto, .urgente), "casa"))
        let corregida = try XCTUnwrap(modelo.listas.first?.tareas[1])
        let marcas = marcasDeTarea(texto: corregida.texto, desfaseMs: desfaseLocalMs())
        XCTAssertEqual(marcas.urgencia, "urgente")
        XCTAssertEqual(marcas.etiquetas, ["casa"])

        // Recordarla: lleva su ⏰ y sale en lo que vigila el vigia.
        let cuando = Date().addingTimeInterval(3600)
        await modelo.recordar(TareaConLista(lista: try XCTUnwrap(modelo.listas.first), tarea: corregida), en: cuando)
        XCTAssertNotNil(modelo.listas.first?.tareas[1].recuerda)
        let puestos = try almacen.nucleo.recordatorios(desfaseLocalMs: desfaseLocalMs())
        XCTAssertEqual(puestos.filter { $0.tarea != nil }.count, 1)

        // Borrar y deshacer.
        let borrar = TareaConLista(lista: try XCTUnwrap(modelo.listas.first), tarea: try XCTUnwrap(modelo.listas.first?.tareas[1]))
        await modelo.borrar(borrar)
        XCTAssertEqual(modelo.listas.first?.tareas.count, 1)
        await modelo.deshacer()
        XCTAssertEqual(modelo.listas.first?.tareas.count, 2)
    }

    func testLaUrgenciaYLasEtiquetasSeEscribenEnElTexto() {
        XCTAssertEqual(ModeloTareas.conUrgencia("pan", .urgente), "pan 🔼")
        XCTAssertEqual(ModeloTareas.conUrgencia("pan 🔼", .muyUrgente), "pan ⏫")
        XCTAssertEqual(ModeloTareas.conUrgencia("pan ⏫ #casa", .normal), "pan #casa")
        XCTAssertEqual(ModeloTareas.conEtiqueta("pan", "#super mercado"), "pan #super_mercado")
        // Casos negativos: sin repetir y sin etiquetas vacias.
        XCTAssertEqual(ModeloTareas.conEtiqueta("pan #casa", "casa"), "pan #casa")
        XCTAssertEqual(ModeloTareas.conEtiqueta("pan", "  "), "pan")
        XCTAssertEqual(ModeloTareas.sinEtiqueta("pan #casa #obra", "casa"), "pan #obra")
    }

    func testBuscarEncuentraSinTildesYPorEtiqueta() {
        XCTAssertTrue(ModeloTareas.encaja("llamar al banco ⏫ #casa", "BANCO #casa"))
        XCTAssertTrue(ModeloTareas.encaja("pagar la calefacción", "calefaccion"))
        XCTAssertFalse(ModeloTareas.encaja("llamar al banco", "banco pan"))
    }
}
