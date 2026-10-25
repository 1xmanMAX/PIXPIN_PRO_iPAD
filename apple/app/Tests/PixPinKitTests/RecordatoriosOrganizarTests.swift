// Los recordatorios y el centro de avisos en el iPad: la agenda del vigia
// (las pruebas de `Agenda` de Windows), las categorias con sus botones y
// las horas que se ofrecen. Nada pide permiso: fuera de una `.app` de
// verdad no hay centro del sistema.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

final class RecordatoriosOrganizarTests: XCTestCase {
    private func rec(_ id: String, _ segundos: TimeInterval, chat: String = "c1") -> RecordatorioPuesto {
        RecordatorioPuesto(chat: chat, id: id, cuando: Date(timeIntervalSince1970: segundos), texto: "lo de \(id)")
    }

    func testLoVencidoSaleYLoDemasSigue() {
        var a = AgendaRecordatorios()
        a.programar(rec("viejo", 1))
        a.programar(rec("futuro", 9))
        XCTAssertEqual(a.vencidos(a: Date(timeIntervalSince1970: 5)).map(\.id), ["viejo"])
        XCTAssertEqual(a.pendientes.map(\.id), ["futuro"])
    }

    func testVolverAPonerloCambiaLaHora() {
        var a = AgendaRecordatorios()
        a.programar(rec("m1", 9))
        a.programar(rec("m1", 2))
        XCTAssertEqual(a.pendientes.count, 1)
        XCTAssertEqual(a.proximo?.cuando, Date(timeIntervalSince1970: 2))
        a.programar(rec("m1", 3, chat: "c2"))
        XCTAssertEqual(a.pendientes.count, 2, "el mismo id en otro chat es otro")
        // Caso negativo: cancelar lo que no esta.
        XCTAssertFalse(a.cancelar("recuerdaEn:c9:nada"))
    }

    func testLasCategoriasLlevanLosBotonesDeWindows() {
        XCTAssertEqual(CategoriaAviso.recordatorioTarea.acciones, [.hecha, .posponer, .abrir])
        XCTAssertEqual(CategoriaAviso.recordatorio.acciones, [.posponer, .abrir])
        XCTAssertEqual(AccionAviso.posponer.rotulo, "+10 min")
        XCTAssertEqual(CategoriaAviso.recordatorio.alPulsar, .abrir)
        let sistema = CentroAvisos.categoriaDelSistema(.recordatorioTarea)
        XCTAssertEqual(sistema.actions.map(\.title), ["Hecha", "+10 min", "Abrir"])
    }

    func testElAvisoDeUnaTareaLlevaSuListaYSuIndice() throws {
        let r = RecordatorioPuesto(chat: "c1", id: "tarea:l1:1", cuando: Date(timeIntervalSince1970: 100),
                                   texto: "  pan  ", tarea: TareaRecordada(lista: "l1", indice: 2))
        let a = AvisosRecordatorio.aviso(de: r)
        XCTAssertEqual(a.categoria, .recordatorioTarea)
        XCTAssertEqual(a.texto, "pan")
        XCTAssertEqual(AvisosRecordatorio.tarea(de: a.datos), TareaRecordada(lista: "l1", indice: 2))
        XCTAssertNil(AvisosRecordatorio.tarea(de: [:]))
    }

    func testLasHorasQueSeOfrecen() throws {
        let cal = Calendar(identifier: .gregorian)
        let ahora = try XCTUnwrap(cal.date(from: DateComponents(year: 2026, month: 10, day: 25, hour: 20)))
        let atajos = AtajosDeHora.atajos(ahora: ahora, calendario: cal)
        XCTAssertEqual(atajos.count, 5)
        // «Esta tarde» a las 20:00 ya paso: es la de manana.
        let tarde = atajos[3].cuando
        XCTAssertGreaterThan(tarde, ahora)
        XCTAssertEqual(cal.component(.hour, from: tarde), 18)
    }
}
