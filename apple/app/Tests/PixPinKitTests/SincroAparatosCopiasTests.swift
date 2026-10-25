// Pruebas de «Tus aparatos» y «Copias de seguridad» (A3 y A5): lo que se lee
// de cada aparato, y la pantalla de copias sin pantalla (`copias_ui.rs` de
// Windows: preguntar no restaura nada, un toque tarde no hace nada,
// recuperar de la papelera).

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

final class SincroAparatosTests: XCTestCase {
    private func fila(_ estado: Sincronizador.Estado, direccion: Direccion? = nil,
                      ultimaVez: Int64 = 0) -> Sincronizador.Fila {
        Sincronizador.Fila(id: "pc", nombre: "PC", letra: "a", direccion: direccion, estado: estado,
                           ultimaVez: ultimaVez)
    }

    func testElAparatoDiceSiEstaYCuandoSeSincronizo() {
        let min: Int64 = 60_000
        let d = Direccion(host: "192.168.1.5", puerto: 47474)
        XCTAssertEqual(textoDeAparato(fila(.contesta, direccion: d, ultimaVez: 1), ahora: 10 * min + 1),
                       "Disponible (192.168.1.5) · sincronizado hace 10 min")
        XCTAssertEqual(textoDeAparato(fila(.noEncontrado, ultimaVez: 1), ahora: 3 * 24 * 60 * min + 1),
                       "No encontrado · sincronizado hace 3 días")
        // Caso negativo: sin vuelta nunca no se inventa un «hace».
        XCTAssertEqual(textoDeAparato(fila(.noEncontrado), ahora: 5), "No encontrado")
        // Lo que acaba de pasar manda sobre la fecha.
        XCTAssertEqual(textoDeAparato(fila(.fallo("No contesta"), ultimaVez: 1), ahora: 5), "No contesta")
    }

    func testDondeEstaUsaLaDeSiempreSiContestaYSiNoLaDelAnuncio() {
        let vistos = [VistoEnRed(id: "tel", nombre: "Telefono", host: "10.0.0.9", puerto: 47475)]
        XCTAssertEqual(dondeEstaAparato(id: "tel", nombre: "Telefono", host: "10.0.0.2", puerto: 47474,
                                        contesta: true, vistos: vistos), .laDeSiempre)
        XCTAssertEqual(dondeEstaAparato(id: "tel", nombre: "Telefono", host: "10.0.0.2", puerto: 47474,
                                        contesta: false, vistos: vistos),
                       .nueva(host: "10.0.0.9", puerto: 47475))
        XCTAssertEqual(dondeEstaAparato(id: "otro", nombre: "Telefono", host: "10.0.0.2", puerto: 47474,
                                        contesta: false, vistos: vistos), .noEsta)
    }

    func testRenombrarYSalirDelGrupoDesdeLaApp() throws {
        let raiz = try carpetaTemporalSincro("aparatos")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let mac = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "Mac")
        _ = try mac.crearGrupo()
        XCTAssertEqual(try mac.renombrarAparato(nombre: " Mac de Max ").nombre, "Mac de Max")
        XCTAssertThrowsError(try mac.renombrarAparato(nombre: "  "))
        let fuera = try mac.salirDelGrupo()
        XCTAssertNil(fuera.grupo)
        XCTAssertEqual(fuera.nombre, "Mac de Max")
        XCTAssertTrue(try mac.miembrosDelGrupo().isEmpty)
    }
}

@MainActor
final class SincroCopiasTests: XCTestCase {
    private func copia(_ id: String) -> CopiaDeSeguridad {
        CopiaDeSeguridad(id: id, chat: "p1", cuando: 1000, motivo: "Antes de sincronizar con Tableta",
                         nombre: "Tesis", hojas: 3, mensajes: 2, sinGrandes: false)
    }

    func testPreguntarNoRestauraNadaHastaQueSeConfirma() async throws {
        let raiz = try carpetaTemporalSincro("copias-vacia")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let m = ModeloCopias(nucleo: try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "Mac"))
        m.preguntar(copia("1"))
        XCTAssertNotNil(m.preguntando, "se pregunta antes de tocar nada")
        // Caso negativo: cancelar deja todo como estaba y no apunta nada.
        m.cancelar()
        XCTAssertNil(m.preguntando)
        XCTAssertNil(m.hecho, "cancelar no es haber restaurado")
        // Y confirmar sin copia en pregunta (un toque tarde) no hace nada.
        await m.confirmar()
        XCTAssertNil(m.hecho)
        // Una copia que ya no esta dice que no se pudo, no restaura otra.
        m.preguntar(copia("no-esta"))
        await m.confirmar()
        if case .mal = m.hecho {} else { XCTFail("tenia que fallar: \(String(describing: m.hecho))") }
    }

    func testRecuperarDeLaPapeleraLoDevuelveALaListaYLoQuitaDeLaPapelera() async throws {
        let raiz = try carpetaTemporalSincro("papelera")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let n = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "Mac")
        let tesis = try n.crearChat(nombre: "Tesis")
        _ = try n.escribirNota(chat: tesis.id, texto: "capitulo 1", respondeA: nil)
        XCTAssertTrue(try n.borrarProyecto(ficha: tesis.id))

        let m = ModeloCopias(nucleo: n)
        var avisos = 0
        m.alCambiar = { avisos += 1 }
        await m.cargar()
        XCTAssertEqual(m.borrados.map(\.nombre), ["Tesis"], "la papelera se lee al entrar")
        // Caso negativo: uno que no esta en la lista no se recupera.
        var otro = m.borrados[0]
        otro.carpeta = "otra-cosa"
        await m.recuperar(otro)
        XCTAssertNil(m.hecho)
        XCTAssertEqual(m.borrados.count, 1, "sigue en la papelera")

        await m.recuperar(m.borrados[0])
        XCTAssertEqual(m.hecho, .recuperado("Tesis"))
        XCTAssertTrue(m.borrados.isEmpty, "ya no esta en la papelera")
        XCTAssertEqual(avisos, 1, "la lista de proyectos se relee")
        XCTAssertTrue(try n.chats().contains { $0.nombre == "Tesis" })
    }

    func testLaHoraSePintaConFechaYMinutosYLaQueNoHayNoSeInventa() {
        XCTAssertTrue(ModeloCopias.cuando(0).isEmpty, "sin hora no se pinta una falsa")
        let t = ModeloCopias.cuando(1_757_939_357_123)
        XCTAssertTrue(t.contains("·") && t.contains(":"), t)
        let c = copia("1")
        XCTAssertEqual(ModeloCopias.detalle(c), "«Tesis» · Antes de sincronizar con Tableta\n3 hojas · 2 mensajes")
        XCTAssertTrue(ModeloCopias.textoDeVolver(c).contains("(antes de sincronizar con Tableta)"))
        XCTAssertEqual(ModeloCopias.texto(.recuperados(n: 1, total: 2)).1, false)
    }
}

/// Envio por codigo completo (A6): un proyecto entero, lo que vuelve y la
/// oferta que se decide antes de aceptar.
final class SincroEnvioCompletoTests: XCTestCase {
    private var raiz: URL!

    override func setUpWithError() throws {
        raiz = try carpetaTemporalSincro("envio-completo")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: raiz)
    }

    private func nucleo(_ nombre: String) throws -> Nucleo {
        try Nucleo.abrir(raiz: raiz.appendingPathComponent(nombre).path, nombreDelAparato: nombre)
    }

    /// Quien contesta la oferta en la prueba: apunta lo que vio.
    final class Contesta: DecidirOferta, @unchecked Sendable {
        var vista: OfertaRecibida?
        let respuesta: (OfertaRecibida) -> [String]?
        init(_ respuesta: @escaping (OfertaRecibida) -> [String]?) { self.respuesta = respuesta }
        func decidir(oferta: OfertaRecibida) -> [String]? {
            vista = oferta
            return respuesta(oferta)
        }
    }

    /// Manda el proyecto `ficha` del PC al Mac, que decide con `decide`.
    private func mandar(_ pc: Nucleo, _ ficha: String, a mac: Nucleo, _ decide: Contesta) throws -> [RecibidoEnvio]? {
        let p = try pc.abrirEnvioDeProyecto(ficha: ficha)
        defer { p.cerrar() }
        p.aprobarTodos()
        let hecho = expectation(description: "atendido")
        atenderEnHilo(nombre: "prueba-envio", { try p.atender() }, esFin: { _ in true }) { _ in hecho.fulfill() }
        let r = try mac.recibirEnvioPreguntando(host: "127.0.0.1", puerto: p.puerto(), codigo: p.codigo(),
                                                decidir: decide)
        wait(for: [hecho], timeout: 10)
        return r
    }

    func testUnProyectoEnteroLlegaALaListaYAlVolverSeOfreceActualizarlo() throws {
        let pc = try nucleo("PC")
        let mac = try nucleo("Mac")
        let tesis = try pc.crearChat(nombre: "Tesis")
        _ = try pc.escribirNota(chat: tesis.id, texto: "capitulo 1", respondeA: nil)

        let primera = Contesta { _ in [] }
        let r = try XCTUnwrap(try mandar(pc, tesis.id, a: mac, primera))
        XCTAssertEqual(r.map(\.donde), ["proyecto"])
        XCTAssertEqual(primera.vista?.cosas.first?.tipo, "proyecto")
        XCTAssertNil(primera.vista?.cosas.first?.yaTienes, "la primera vez no se tiene")
        XCTAssertTrue(try mac.chats().contains { $0.nombre == "Tesis" })

        // Otra vez: ya lo tiene, se ofrece ponerlo al dia y se pone al dia.
        let segunda = Contesta { _ in [] }
        let r2 = try XCTUnwrap(try mandar(pc, tesis.id, a: mac, segunda))
        XCTAssertEqual(segunda.vista?.cosas.first?.yaTienes, "Tesis")
        XCTAssertEqual(r2.map(\.donde), ["proyecto_al_dia"])
        XCTAssertEqual(try mac.chats().filter { $0.nombre == "Tesis" }.count, 1, "no duplica")
        XCTAssertFalse(mac.copiasDeSeguridad().isEmpty, "copia de antes de ponerlo al dia")

        // «Crear como nuevo»: entra aparte.
        let tercera = Contesta { o in o.cosas.map(\.identidad) }
        let r3 = try XCTUnwrap(try mandar(pc, tesis.id, a: mac, tercera))
        XCTAssertEqual(r3.map(\.donde), ["proyecto"])
        XCTAssertEqual(try mac.chats().filter { $0.nombre == "Tesis" }.count, 2)

        // Caso negativo: «No, gracias» no trae nada.
        let no = Contesta { _ in nil }
        XCTAssertNil(try mandar(pc, tesis.id, a: mac, no))
        XCTAssertEqual(try mac.chats().filter { $0.nombre == "Tesis" }.count, 2)
    }

    func testLaOfertaSoloDejaElegirEnLoQueYaSeTiene() {
        let o = OfertaRecibida(de: "Movil", deCodigo: "K7Q2", cosas: [
            CosaOfrecida(identidad: "proyecto:a", tipo: "proyecto", nombre: "Tesis", bytes: 10,
                         proyectoNombre: nil, yaTienes: "Tesis"),
            CosaOfrecida(identidad: "lienzo:a:h", tipo: "lienzo", nombre: "Plano", bytes: 5,
                         proyectoNombre: "Tesis", yaTienes: nil),
        ])
        var el = EligiendoOferta(o)
        XCTAssertFalse(el.nadaCoincide)
        XCTAssertEqual(el.respuesta, [], "por defecto se actualiza, como en el movil")
        el.elegir("proyecto:a", nuevo: true)
        // Caso negativo: lo que no se tiene no se elige.
        el.elegir("lienzo:a:h", nuevo: true)
        XCTAssertEqual(el.respuesta, ["proyecto:a"])
        el.elegir("proyecto:a", nuevo: false)
        XCTAssertEqual(el.respuesta, [])
        XCTAssertEqual(EligiendoOferta.nombre(o.cosas[1]), "Lienzo «Plano» de «Tesis»")
        XCTAssertTrue(EligiendoOferta.explicacion(nuevo: false).contains("Copias de seguridad"))
    }
}
