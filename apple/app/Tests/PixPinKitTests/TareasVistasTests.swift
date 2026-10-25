@testable import PixPinKit
import PixPinCore
import XCTest

/// Las vistas de Tareas (Tarjetas, Kanban, Horario) y «clase» y «carpeta»
/// en Buscar (Windows 5139e5d y 22c3ce2, 25-oct-2026; las del Mac). En el
/// iPad la eleccion del nucleo se llama `VistaDeTareas` (`VistaTareas` es la
/// seccion).
@MainActor
final class TareasVistasTests: XCTestCase {
    func testLasVistasSinListaYSusAtajos() {
        XCTAssertEqual(VistaDeTareas.todas.map(\.rotulo), ["Tarjetas", "Kanban", "Horario"])
        XCTAssertEqual(VistaDeTareas.todas.map(\.letraAtajo), ["t", "k", "h"])
        XCTAssertEqual(VistaDeTareas.horario.pista, "Horario · ⌃⌘H")
        // Lo que el iPad guardaba antes abre en las tarjetas.
        XCTAssertEqual(vistaTareasDeTexto(texto: "Por fecha"), .tarjetas)
        XCTAssertEqual(vistaTareasDeTexto(texto: "Por lista"), .tarjetas)
        XCTAssertEqual(vistaTareasDeTexto(texto: VistaPedidaDeTareas.horario.rawValue), .horario)
        XCTAssertEqual(vistaTareasDeTexto(texto: vistaTareasComoTexto(vista: .kanban)), .kanban)
    }

    func testClaseYCarpetasEnBuscar() {
        XCTAssertTrue(accionesBuscar(para: "clase").contains(.grabarClase))
        XCTAssertTrue(accionesBuscar(para: "reunión").contains(.grabarClase))
        XCTAssertEqual(AccionBuscar.consultaDeCarpetas("carpeta Diario de obra"), "Diario de obra")
        XCTAssertEqual(AccionBuscar.consultaDeCarpetas("carp"), "")
        XCTAssertTrue(AccionBuscar.soloCarpetas("Cuaderno x"))
        // Sin carpetas leidas, crearla; con una que se llama asi, no.
        XCTAssertEqual(accionesBuscar(para: "carpeta diario"), [.nuevaCarpeta("diario")])
        let c = CarpetaDeFlow(chat: "1", nombreChat: "Obra", id: "2", nombre: "Diario", paginas: 2,
                              grabacion: false, cuando: 0)
        let g = CarpetaDeFlow(chat: "1", nombreChat: "Obra", id: "3", nombre: "Diario clase", paginas: 1,
                              grabacion: true, cuando: 0)
        let v = accionesBuscar(para: "carpeta diario", carpetas: CarpetasDeFlow(carpetas: [c, g], exacta: true))
        XCTAssertEqual(v, [.carpeta(c), .paginaEnCarpeta(c), .carpeta(g)], "la grabada sin «Nueva página»")
        // Casos negativos: otra palabra no es de las carpetas.
        XCTAssertNil(AccionBuscar.consultaDeCarpetas("car"))
        XCTAssertNil(AccionBuscar.consultaDeCarpetas("galeria carpeta"))
        XCTAssertFalse(AccionBuscar.soloCarpetas("carp"))
    }

    /// Los pedidos de Flow aceptan `carpeta_paginas` sin perder `al_lienzo`.
    func testLosPedidosConocenLasCarpetas() {
        XCTAssertTrue(Pedido.acciones.contains("carpeta_paginas"))
        XCTAssertTrue(Pedido.acciones.contains("al_lienzo"))
        XCTAssertEqual(Set(Pedido.acciones).count, Pedido.acciones.count, "sin repetidas")
    }

    /// El numero del dia de la cabecera del Horario (dias desde 1970).
    func testElNumeroDelDiaDelHorario() {
        XCTAssertEqual(VistaHorarioIPad.numeroDelDia(0), 1, "1-ene-1970")
        XCTAssertEqual(VistaHorarioIPad.numeroDelDia(31), 1, "1-feb-1970")
        XCTAssertEqual(VistaHorarioIPad.numeroDelDia(30), 31)
    }

    /// Lo pendiente con hora va a las chapitas; lo hecho o sin hora, no.
    func testLasChapitasSoloLlevanLoPendienteConHora() throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("chapitas")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let m = ModeloTareas(nucleo: almacen.nucleo)
        XCTAssertTrue(m.conHora.isEmpty)
        XCTAssertNil(m.listaDeAsignatura("Geometría"), "sin listas, ninguna")
    }
}
