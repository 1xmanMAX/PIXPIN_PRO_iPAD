// La pestana Funciones de la seccion Proyectos (`FuncionesDeLaLista.swift`,
// Windows 2e0da60 y 768b73f; la del Mac): los mosaicos en el orden de
// Windows, la rejilla de la lista de siempre y el teclado, por el puente de
// verdad.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

@MainActor
final class FuncionesTests: XCTestCase {
    func testLosMosaicosVanEnElOrdenDeWindowsConSusTextos() {
        let es = textosDeFunciones(locale: "es-ES")
        XCTAssertEqual(es.mosaicos.map(\.funcion),
                       [.grabar, .lienzo, .tareas, .timeline, .horario, .galeria, .figuras])
        XCTAssertEqual(es.interruptorFunciones, "Funciones")
        XCTAssertEqual(es.mosaicos[1].titulo, "Nuevo lienzo")
        // Cada icono es uno de los trazados de Windows.
        for m in es.mosaicos {
            XCTAssertNotNil(TrazadosMaterial.todos[m.icono], m.icono)
        }
        // Caso negativo: en ingles no sale el espanol.
        XCTAssertEqual(textosDeFunciones(locale: "en-US").mosaicos[4].titulo, "Timetable")
    }

    func testLaRejillaYElTeclado() {
        let g = rejillaDeFunciones(ancho: 320, alto: 700)
        XCTAssertEqual(g.columnas, 2)
        XCTAssertTrue(g.conDetalle)
        XCTAssertEqual(g.mosaicos.count, 7)
        let r = teclaEnFunciones(elegida: nil, tecla: .abajo, columnas: g.columnas)
        XCTAssertEqual(r.elegida, 0)
        XCTAssertEqual(teclaEnFunciones(elegida: 4, tecla: .hacer, columnas: 2).hacer, .horario)
        XCTAssertTrue(teclaEnFunciones(elegida: 4, tecla: .escape, columnas: 2).volver)
    }

    /// El mosaico Horario (y el pedido `ventana horario`) lleva a Tareas y
    /// deja pedida su vista Horario.
    func testElHorarioAbreTareasEnSuVista() {
        let nav = Navegacion.compartida
        let antes = nav.seccion
        defer {
            nav.seccion = antes
            VistaTareas.vistaPedida = nil
        }
        nav.seccion = .proyectos
        VistaTareas.abrir(vista: .horario)
        XCTAssertEqual(nav.seccion, .tareas)
        XCTAssertEqual(VistaTareas.vistaPedida, .horario)
        XCTAssertEqual(vistaTareasDeTexto(texto: VistaPedidaDeTareas.horario.rawValue), .horario)
    }
}
