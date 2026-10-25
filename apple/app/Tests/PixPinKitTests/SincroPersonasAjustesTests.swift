// Pruebas de lo puro de la zona Sincro y Ajustes nuevo: con quien de los
// chats compartidos se sincroniza solo (`personasALlamar`, el de Windows),
// el resumen de una vuelta con una persona, las direcciones recordadas con
// su nombre, y las filas de Ajustes (buscar sin tildes, «Restablecer
// <seccion>» en un paso de «Deshacer»).

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

final class SincroPersonasTests: XCTestCase {
    private func persona(_ id: String, dentro: Bool = true, soyYo: Bool = false) -> PersonaCompartida {
        PersonaCompartida(id: id, nombre: id.uppercased(), codigo: "K7Q2", dentro: dentro, desde: 0,
                          soyYo: soyYo, puedoQuitar: false)
    }

    private func chat(_ id: String, dentro: Bool = true, _ personas: [PersonaCompartida]) -> ChatCompartidoMac {
        ChatCompartidoMac(chat: id, ficha: nil, nombre: id, creador: "yo", creado: 0, personas: personas,
                          estoyDentro: dentro)
    }

    func testSoloSeLlamaALasPersonasDeChatsDondeEstamosDentro() {
        let chats = [
            chat("obra", [persona("yo", soyYo: true), persona("ana"), persona("mi-pc"), persona("luis", dentro: false)]),
            chat("casa", [persona("yo", soyYo: true), persona("ana"), persona("eva")]),
            // Caso negativo: de un chat del que sali no se llama a nadie.
            chat("viejo", dentro: false, [persona("yo", soyYo: true), persona("pepe")]),
        ]
        let v = personasALlamar(chats, yo: "yo", mios: ["mi-pc"])
        // Una vez cada una, en orden; ni yo, ni mis aparatos, ni quien salio.
        XCTAssertEqual(v.map(\.id), ["ana", "eva"])
        XCTAssertEqual(v.first?.nombre, "ANA")
        XCTAssertTrue(personasALlamar([], yo: "yo", mios: []).isEmpty)
    }

    func testElResumenDeUnaVueltaConUnaPersona() {
        let r = ResumenPersona(otroId: "ana", otroNombre: "Ana", puertoDelOtro: 47474, chats: ["obra"],
                               traidos: 3, enviados: 1, bytes: 10, trajoAlgo: true)
        XCTAssertEqual(resumenDePersona(r), "3 traídos · 1 enviados")
        var nada = r
        nada.traidos = 0
        nada.enviados = 0
        XCTAssertEqual(resumenDePersona(nada), "al día")
    }

    func testLasDireccionesRecordadasSalenConSuNombreYLasDelGrupoPrimero() {
        let dirs = [
            DireccionRecordada(id: "x", host: "10.0.0.9", puerto: 47474),
            DireccionRecordada(id: "ana", host: "10.0.0.5", puerto: 47474),
            DireccionRecordada(id: "pc", host: "10.0.0.2", puerto: 47475),
        ]
        let miembros = [MiembroGrupo(id: "pc", nombre: "PC-Casa", letra: "a", direccion: nil, ultimaVez: 0)]
        let v = direccionesConNombre(dirs, miembros: miembros, personas: ["ana": "Ana"])
        XCTAssertEqual(v.map(\.id), ["pc", "ana", "x"])
        XCTAssertEqual(v[0].nombre, "PC-Casa")
        XCTAssertTrue(v[0].esDelGrupo)
        XCTAssertEqual(v[0].direccion, Direccion(host: "10.0.0.2", puerto: 47475))
        XCTAssertFalse(v[1].esDelGrupo)
        // Caso negativo: de quien no se sabe nada, sin nombre y al final.
        XCTAssertNil(v[2].nombre)
    }
}

@MainActor
final class AjustesFilasIPadTests: XCTestCase {
    private var raiz: URL!
    private var suite = ""
    private var defaults: UserDefaults!

    override func setUp() async throws {
        raiz = FileManager.default.temporaryDirectory.appendingPathComponent("pixpin-ajustes-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: raiz, withIntermediateDirectories: true)
        suite = "pixpin-pruebas-\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: raiz)
        defaults.removePersistentDomain(forName: suite)
    }

    func testElBuscadorEncuentraSinTildesEnTodasLasSecciones() {
        let ids = FilasAjustes.buscar("sincronízar SOLA").map(\.id)
        XCTAssertTrue(ids.contains("automatica"), "\(ids)")
        XCTAssertTrue(FilasAjustes.buscar("PDF").contains { $0.id == "pdf-nivel" })
        XCTAssertTrue(FilasAjustes.buscar("microsoft").contains { $0.id == "voces-microsoft" })
        // Todas las palabras tienen que estar; vacio no encuentra nada.
        XCTAssertTrue(FilasAjustes.buscar("pdf zanahoria").isEmpty)
        XCTAssertTrue(FilasAjustes.buscar("   ").isEmpty)
        // Cada fila tiene un id unico (el buscador y las vistas los usan).
        let todos = FilasAjustes.todas.map(\.id)
        XCTAssertEqual(Set(todos).count, todos.count)
    }

    func testRestablecerUnaSeccionSoloTocaLaSuyaYSeDeshaceDeUnaVez() async throws {
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        let m = ModeloAjustes(nucleo: nucleo, defaults: defaults)
        await m.cargar()
        m.cambiar { $0.sincroPresencia = false; $0.sincroLoMioManda = !m.fabrica.sincroLoMioManda }
        m.fijar(diasCaducidad: 7)
        XCTAssertTrue(FilasAjustes.hayCambios(en: .sincro, m))
        XCTAssertTrue(FilasAjustes.hayCambios(en: .general, m))
        let pasos = m.historial.count
        FilasAjustes.restablecer(.sincro, m)
        XCTAssertFalse(FilasAjustes.hayCambios(en: .sincro, m))
        // Caso negativo: lo de General sigue como estaba.
        XCTAssertEqual(m.diasCaducidad, 7)
        XCTAssertEqual(m.historial.count, pasos + 1, "un solo paso de «Deshacer»")
        m.deshacer()
        XCTAssertFalse(m.valores.sincroPresencia)
        await m.guardar()
        XCTAssertEqual(try nucleo.ajustes().diasCaducidad, 7)
    }
}
