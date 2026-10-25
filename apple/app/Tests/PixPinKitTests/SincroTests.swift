// Pruebas de sincronizar: lo puro (TXT de Bonjour, quien es del grupo, la
// espera de «quieto» con un reloj de mentira) y una vuelta de verdad entre
// dos nucleos en carpetas temporales por 127.0.0.1, como hace Windows.

import Foundation
import Network
import PixPinCore
@testable import PixPinKit
import XCTest

final class SincroTXTBonjourTests: XCTestCase {
    func testElTXTSeEscribeEnOrdenYSeLeeIgual() {
        let pares = [("g", "0123456789abcdef"), ("id", "abc"), ("l", "c"), ("n", "Mac de Max ñ")]
        let datos = codificarTXT(pares)
        // Cada entrada lleva delante su largo en un byte.
        XCTAssertEqual(Int(datos[0]), "g=0123456789abcdef".utf8.count)
        XCTAssertEqual(String(decoding: datos[1 ... 2], as: UTF8.self), "g=")
        let leido = leerTXT(datos)
        XCTAssertEqual(leido, ["g": "0123456789abcdef", "id": "abc", "l": "c", "n": "Mac de Max ñ"])
    }

    func testUnTXTVacioEsUnByteCeroYLoQueNoCabeSeDejaFuera() {
        XCTAssertEqual(codificarTXT([]), Data([0]))
        let largo = String(repeating: "x", count: 300)
        XCTAssertEqual(leerTXT(codificarTXT([("n", largo), ("id", "a")])), ["id": "a"])
    }

    func testLasClavesNoDistinguenMayusculas() {
        let d = DatosVecino(txt: ["G": "etq", "ID": "otro", "L": "b", "N": "PC"])
        XCTAssertEqual(d, DatosVecino(etiqueta: "etq", id: "otro", letra: "b", nombre: "PC"))
    }

    func testSoloEsDelGrupoQuienAnunciaMiEtiquetaYNoSoyYo() {
        let otro = DatosVecino(txt: ["g": "etq", "id": "pc"])
        XCTAssertTrue(otro.esDelGrupo(etiqueta: "etq", yo: "mac"))
        XCTAssertFalse(otro.esDelGrupo(etiqueta: "otra", yo: "mac"))
        // Mi propio anuncio tambien sale al buscar.
        XCTAssertFalse(otro.esDelGrupo(etiqueta: "etq", yo: "pc"))
        // Sin `g` (sin grupo) no es de ninguno, ni con una etiqueta vacia.
        let suelto = DatosVecino(txt: ["g": "", "id": "x"])
        XCTAssertNil(suelto.etiqueta)
        XCTAssertFalse(suelto.esDelGrupo(etiqueta: "", yo: "mac"))
    }

    func testElNombreSaleDelTXTODeLaInstancia() {
        XCTAssertEqual(DatosVecino(txt: ["n": "Movil"]).nombreParaVer(instancia: "PixPin Telefono"), "Movil")
        XCTAssertEqual(DatosVecino(txt: [:]).nombreParaVer(instancia: "PixPin Telefono"), "Telefono")
        XCTAssertEqual(DatosVecino(txt: [:]).nombreParaVer(instancia: "Otro"), "Otro")
    }

    func testLasDireccionesSeEscribenComoEnElMovil() {
        XCTAssertEqual(Direccion(texto: "192.168.1.20:47474"), Direccion(host: "192.168.1.20", puerto: 47474))
        XCTAssertEqual(Direccion(texto: " 10.0.0.8 "), Direccion(host: "10.0.0.8", puerto: puertoPreferido()))
        XCTAssertEqual(Direccion(texto: "[fe80::1%en0]:5000"), Direccion(host: "fe80::1%en0", puerto: 5000))
        XCTAssertEqual(Direccion(texto: "fe80::1"), Direccion(host: "fe80::1", puerto: puertoPreferido()))
        XCTAssertNil(Direccion(texto: ""))
        XCTAssertNil(Direccion(texto: "pc:nope"))
        XCTAssertNil(Direccion(texto: "pc:0"))
    }

    func testLosHostsDeNetworkSeEscribenParaRust() {
        XCTAssertEqual(textoIPv4(Data([192, 168, 1, 5])), "192.168.1.5")
        XCTAssertNil(textoIPv4(Data([1, 2, 3])))
        XCTAssertEqual(Vecinos.textoDeHost(.ipv4(IPv4Address("10.0.0.9")!)), "10.0.0.9")
        XCTAssertEqual(Vecinos.textoDeHost(.ipv6(IPv6Address("2001:db8::1")!)), "2001:db8::1")
        XCTAssertEqual(Vecinos.textoDeHost(.name("pc.local", nil)), "pc.local")
    }

    func testElTipoYElPuertoSonLosDeWindows() {
        XCTAssertEqual(tipoDeServicio(), "_pixpin._tcp")
        XCTAssertEqual(puertoPreferido(), 47474)
    }
}

final class SincroAutomaticaTests: XCTestCase {
    /// Un reloj que solo avanza cuando la prueba quiere.
    final class Reloj {
        var ahora = Date(timeIntervalSince1970: 1_000_000)
        func avanzar(_ s: TimeInterval) { ahora = ahora.addingTimeInterval(s) }
    }

    /// El reloj de la automatica (`automatica.rs` de Windows), por la API
    /// que usa el `Sincronizador`.
    func testElRelojEsperaTreintaSegundosQuietoYCadaDiezMinutos() {
        let t = tiemposAuto()
        XCTAssertEqual(t.quietoMs, 30_000)
        XCTAssertEqual(t.periodoMs, 600_000)
        let r = RelojSincro()
        XCTAssertEqual(r.siguiente(ahora: 0, ocupado: false), .nada, "sin nadie que conteste, nada")
        r.responde(id: "tel", si: true)
        r.verCambios(cambios: 0, ahora: 0)
        XCTAssertEqual(r.siguiente(ahora: 0, ocupado: false), .entera(motivo: .aparecio))
        r.empezo(ahora: 0)
        XCTAssertFalse(r.acabo(bien: true, cambios: 0, ahora: 0))
        r.verCambios(cambios: 1, ahora: 1_000)
        XCTAssertEqual(r.proximoMs(ahora: 1_000, corriendo: false), t.quietoMs)
        XCTAssertEqual(r.siguiente(ahora: 1_000 + t.quietoMs, ocupado: false), .entera(motivo: .cambio))
        XCTAssertEqual(r.siguiente(ahora: 1_000 + t.quietoMs, ocupado: true), .nada, "nunca a la par")
    }

    func testLasTareasSalenAlInstanteYAntesQueLaEntera() {
        let t = tiemposAuto()
        let r = RelojSincro()
        r.responde(id: "tel", si: true)
        r.empezo(ahora: 0)
        _ = r.acabo(bien: true, cambios: 0, ahora: 0)
        r.avisanTareas(ficha: "obra", ahora: 100)
        XCTAssertEqual(r.siguiente(ahora: 100 + t.calmaTareasMs - 1, ocupado: false), .nada)
        XCTAssertEqual(r.siguiente(ahora: 100 + t.calmaTareasMs, ocupado: false), .tareas(fichas: ["obra"]))
        XCTAssertEqual(r.tomarTareas(), ["obra"])
        XCTAssertEqual(rondasConTodos(aparatos: 2, soloTareas: false), 2, "tres aparatos: dos rondas")
        XCTAssertEqual(rondasConTodos(aparatos: 1, soloTareas: false), 1)
        XCTAssertEqual(rondasConTodos(aparatos: 3, soloTareas: true), 1)
    }

    func testElTercerFalloSeguidoAvisaUnaVez() {
        let r = RelojSincro()
        XCTAssertFalse(r.acabo(bien: false, cambios: 0, ahora: 0))
        XCTAssertFalse(r.acabo(bien: false, cambios: 0, ahora: 0))
        XCTAssertTrue(r.acabo(bien: false, cambios: 0, ahora: 0))
        XCTAssertFalse(r.acabo(bien: false, cambios: 0, ahora: 0), "caso negativo: el cuarto ya no")
    }

    func testLoAutomaticoRespetaElBajoConsumo() {
        XCTAssertTrue(sePuedeSincronizar(.manual, automatica: false, bajoConsumo: true))
        XCTAssertTrue(sePuedeSincronizar(.quieto, automatica: true, bajoConsumo: false))
        XCTAssertFalse(sePuedeSincronizar(.quieto, automatica: true, bajoConsumo: true))
        XCTAssertFalse(sePuedeSincronizar(.aparecio, automatica: true, bajoConsumo: true))
        XCTAssertFalse(sePuedeSincronizar(.aparecio, automatica: false, bajoConsumo: false))
        // `[sincro] presencia = false`: a mano si, solo no.
        XCTAssertTrue(sePuedeSincronizar(.manual, automatica: true, bajoConsumo: false, presencia: false))
        XCTAssertFalse(sePuedeSincronizar(.periodo, automatica: true, bajoConsumo: false, presencia: false))
        XCTAssertFalse(sePuedeSincronizar(.tareas, automatica: true, bajoConsumo: false, presencia: false))
        XCTAssertTrue(sePuedeSincronizar(.tareas, automatica: true, bajoConsumo: false, presencia: true))
    }

    func testOcupadoNoEsUnFallo() {
        XCTAssertTrue(esOcupado("Está sincronizando con otro aparato. Prueba otra vez en un momento."))
        XCTAssertFalse(esOcupado("PC no aceptó la conexión: ¿tiene el mismo código de grupo?"))
    }

    func testUnAparatoQueVaYVieneNoLanzaUnaVueltaCadaVez() {
        let reloj = Reloj()
        XCTAssertTrue(tocaPorAparecer(ultimaVuelta: nil, ahora: reloj.ahora))
        let vuelta = reloj.ahora
        reloj.avanzar(10)
        XCTAssertFalse(tocaPorAparecer(ultimaVuelta: vuelta, ahora: reloj.ahora))
        reloj.avanzar(entreVueltasPorAparecer)
        XCTAssertTrue(tocaPorAparecer(ultimaVuelta: vuelta, ahora: reloj.ahora))
    }
}

final class SincroDeVerdadTests: XCTestCase {
    /// Dos aparatos en dos carpetas: el «PC» crea el grupo y escucha en su
    /// `Puerta` (el hilo de la app), el «Mac» se une y sincroniza una nota,
    /// todo por la API de Rust y 127.0.0.1.
    func testDosNucleosSeUnenYSincronizanPorLocalhost() throws {
        let raiz = try carpetaTemporalSincro("sincro")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let pc = try Nucleo.abrir(raiz: raiz.appendingPathComponent("pc").path, nombreDelAparato: "PC")
        let mac = try Nucleo.abrir(raiz: raiz.appendingPathComponent("mac").path, nombreDelAparato: "Mac")

        // Sin grupo, no hay anuncio.
        XCTAssertNil(try pc.anuncio())
        let codigo = try pc.crearGrupo()
        XCTAssertTrue(codigoValido(codigo: codigo))
        let anuncio = try XCTUnwrap(try pc.anuncio())
        XCTAssertEqual(anuncio.tipo, tipoDeServicio())
        XCTAssertEqual(anuncio.instancia, "PixPin PC")

        // Lo que anuncia el PC, leido como lo leeria otro Mac por Bonjour,
        // es del grupo de quien tenga su codigo.
        let visto = DatosVecino(txt: leerTXT(codificarTXT(anuncio.txt.map { ($0.clave, $0.valor) })))
        XCTAssertTrue(visto.esDelGrupo(etiqueta: etiquetaDeGrupo(codigo: codigo), yo: try mac.aparato().id))
        XCTAssertEqual(visto.nombre, "PC")
        XCTAssertEqual(visto.letra, "a")

        let puerta = try Puerta(nucleo: pc)
        defer { puerta.cerrar() }
        let llegoElMac = expectation(description: "el Mac saludo dos veces (union y vuelta)")
        llegoElMac.expectedFulfillmentCount = 2
        puerta.arrancar { resultado in
            if case let .success(.aparato(_, nombre)) = resultado, nombre == "Mac" {
                llegoElMac.fulfill()
            }
        }

        let yo = try mac.unirseAGrupo(host: "127.0.0.1", puerto: puerta.puerto,
                                      codigo: codigoLegible(codigo: codigo).lowercased())
        XCTAssertEqual(yo.grupo, codigo)
        XCTAssertNotNil(yo.letra)
        // Ya en el grupo, el Mac anuncia la misma etiqueta que el PC.
        XCTAssertEqual(try mac.anuncio()?.etiqueta, anuncio.etiqueta)

        let obra = try mac.crearChat(nombre: "Obra")
        _ = try mac.escribirNota(chat: obra.id, texto: "desde el Mac", respondeA: nil)
        let r = try mac.sincronizarCon(host: "127.0.0.1", puerto: puerta.puerto)
        XCTAssertEqual(r.otroNombre, "PC")
        XCTAssertGreaterThanOrEqual(r.enviados, 1)
        XCTAssertFalse(r.texto.isEmpty)
        wait(for: [llegoElMac], timeout: 10)

        let suya = try XCTUnwrap(try pc.chats().first { $0.nombre == "Obra" })
        let pagina = try pc.ultimosMensajes(chat: suya.id, cuantos: 10)
        XCTAssertEqual(pagina.mensajes.map(\.texto), ["desde el Mac"])
    }

    /// La sonda de la app (`NWConnection`, PING → PONG) contra la puerta de
    /// Rust: contesta y da la direccion que necesita `sincronizarCon`.
    func testLaSondaDeNetworkContestaYDaLaDireccion() async throws {
        let raiz = try carpetaTemporalSincro("sonda")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let pc = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "PC")
        let puerta = try Puerta(nucleo: pc)
        defer { puerta.cerrar() }
        puerta.arrancar { _ in }
        let punto = NWEndpoint.hostPort(host: "127.0.0.1", port: NWEndpoint.Port(rawValue: puerta.puerto)!)
        let d = await Vecinos.sondear(punto)
        XCTAssertEqual(d, Direccion(host: "127.0.0.1", puerto: puerta.puerto))
    }
}

/// «¿Qué sincronizar?» sin pantalla (`elegir.rs` de Windows).
final class SincroElegirTests: XCTestCase {
    private func fila(_ id: String, aqui: Bool = true, alli: Bool = true, marcado: Bool = true,
                      seBorraAqui: Bool = false) -> FilaElegir {
        FilaElegir(id: id, nombre: id.capitalized, aqui: aqui, alli: alli,
                   estadoAqui: "Más reciente", recienteAqui: true,
                   estadoAlli: "Anterior", recienteAlli: false,
                   marcado: marcado, seBorraAqui: seBorraAqui)
    }

    private func pregunta(_ filas: [FilaElegir], seBorranAlli: [String] = []) -> PreguntaSincro {
        PreguntaSincro(otro: "PC", yo: "Mac", filas: filas, seBorranAlli: seBorranAlli, loMioManda: false)
    }

    func testLoBorradoEnElOtroAvisaSoloSiSeMarcaYLoMioMandaLoVuelveAMandar() {
        var el = Eligiendo(pregunta([fila("obra", alli: false, marcado: false, seBorraAqui: true),
                                     fila("casa")]))
        XCTAssertEqual(el.cuantos, 1)
        XCTAssertNil(el.avisoDeBorrados, "caso negativo: sin marcar no se borra")
        XCTAssertEqual(el.estado(0, esMio: true).texto, "Borrado en PC · sin marcar: se queda aquí")
        el.marcados[0] = true
        XCTAssertEqual(el.estado(0, esMio: true).color, .rojo)
        XCTAssertTrue(el.avisoDeBorrados?.contains("Se borrará AQUÍ, porque se borró en PC: «Obra»") ?? false)
        el.loMioManda = true
        XCTAssertNil(el.avisoDeBorrados, "con lo mío manda aquí no se borra")
        XCTAssertEqual(el.estado(0, esMio: false).color, .verde)
        XCTAssertEqual(el.boton, "Mandar 2")
        XCTAssertEqual(el.eleccion, EleccionSincro(elegidos: ["obra", "casa"], loMioManda: true))
    }

    func testTodoNadaYLoBorradoAqui() {
        var el = Eligiendo(pregunta([fila("obra"), fila("casa", marcado: false)], seBorranAlli: ["Viejo"]))
        XCTAssertEqual(el.avisoDeBorrados, "Se borrará en PC, porque lo borraste aquí: «Viejo».")
        XCTAssertEqual(el.estado(0, esMio: true).texto, "Más reciente")
        XCTAssertEqual(el.estado(0, esMio: false).color, .gris)
        el.nada()
        XCTAssertNil(el.eleccion, "caso negativo: con nada marcado no se sincroniza")
        el.todo()
        XCTAssertEqual(el.boton, "Sincronizar 2")
    }
}
