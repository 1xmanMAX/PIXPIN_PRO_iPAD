// Pruebas de lo nuevo de sincronizar: los textos de «Enviar a…», del envio
// con codigo y de lo recibido; el QR de CoreImage; elegir la IPv4 del QR; y
// de verdad entre dos nucleos por la red de esta maquina, con la `Puerta`
// de la app (varios hilos): la galeria en la vuelta, un archivo suelto, el
// envio con codigo, un chat compartido (invitar y entrar), una sonda
// durante una vuelta e IPv6 (`::1`) si la maquina lo tiene.

import UIKit
import Foundation
import Network
import PixPinCore
@testable import PixPinKit
import XCTest

final class SincroTextosDeEnvioTests: XCTestCase {
    func testLaFraseDeEnviarADiceDondeQuedo() {
        let uno = [EnvioSuelto(nombre: "a.png", donde: .chatAbierto, chat: "Obra", error: nil)]
        XCTAssertEqual(estadoDeEnvio(uno, a: "PC"), .hecho("«a.png» llegó a PC, en «Obra»."))
        let mezcla = [
            EnvioSuelto(nombre: "a.png", donde: .lienzo, chat: nil, error: nil),
            EnvioSuelto(nombre: "b.pdf", donde: nil, chat: nil, error: "solo acepta fotos"),
        ]
        XCTAssertEqual(estadoDeEnvio(mezcla, a: "PC"),
                       .hecho("«a.png» llegó a PC, en su lienzo.\n«b.pdf»: solo acepta fotos."))
        // Caso negativo: no llego nada.
        let nada = [EnvioSuelto(nombre: "b.pdf", donde: nil, chat: nil, error: "se cortó")]
        XCTAssertEqual(estadoDeEnvio(nada, a: "PC"), .fallo("se cortó"))
        XCTAssertEqual(EstadoEnvioSuelto.enviando(3).texto, "Enviando 3 archivos…")
    }

    func testLoQueLlegaYLoQueSeRecibeEnPalabras() {
        let l = ArchivoLlegado(nombre: "a.png", donde: .chat, chat: "g", mensaje: "m", ruta: nil)
        XCTAssertEqual(textoDeLlegadas([l], de: "PC"), "PC envió «a.png» a Mensajes guardados.")
        XCTAssertEqual(textoDeLlegadas([l, l], de: "PC"), "PC envió 2 archivos a Mensajes guardados.")
        XCTAssertEqual(textoDeLlegadas([], de: "PC"), "")
        XCTAssertEqual(textoDeFin(.enviado(nombre: "Movil")), "Enviado a Movil.")
        XCTAssertEqual(textoDeFin(.codigoMalo(intentos: 2)), "Alguien probó otro código (2 de 5).")
        let cosas = [
            RecibidoEnvio(nombre: "p", donde: "proyecto", chat: "1"),
            RecibidoEnvio(nombre: "a", donde: "guardados", chat: "2"),
        ]
        XCTAssertEqual(textoDeRecibido(cosas, de: "PC"),
                       "Recibido de PC\np: en la lista de proyectos\na: en «Mensajes guardados»")
        XCTAssertEqual(textoDeRecibido([RecibidoEnvio(nombre: "Tesis", donde: "proyecto_al_dia", chat: "1")]),
                       "Recibido\n«Tesis» puesto al día")
        XCTAssertEqual(textoDeRecibido([]), "No llegó nada.")
    }

    func testElQRSePintaYLaIPv4EsLaDeLaRedLocal() throws {
        let qr = try XCTUnwrap(imagenQR("pixpin-chat:1:K7Q2M9XP:192.168.1.5:4000", lado: 200))
        XCTAssertGreaterThanOrEqual(qr.size.width, 100)
        XCTAssertEqual(qr.size.width, qr.size.height)
        XCTAssertEqual(elegirIPv4([("utun3", "10.8.0.2"), ("en1", "192.168.1.9"), ("en0", "192.168.1.5")]),
                       "192.168.1.5")
        XCTAssertEqual(elegirIPv4([("bridge0", "10.0.0.1")]), "10.0.0.1")
        XCTAssertNil(elegirIPv4([]))
        // Lo que lee Rust de lo tecleado.
        XCTAssertEqual(leerInvitacion(texto: "k7q2-m9xp")?.codigo, "K7Q2M9XP")
        XCTAssertNil(leerInvitacion(texto: "pixpin-envio:1:123456::0"))
        XCTAssertEqual(leerCodigoDeEnvio(texto: "482 913")?.codigo, "482913")
        XCTAssertEqual(tipoDeServicioChat(), "_pixpinchat._tcp")
        XCTAssertEqual(tipoDeServicioEnvio(), "_pixpinenvio._tcp")
        XCTAssertEqual(tipoDeServicioRecibir(), "_pixpinrecibe._tcp")
    }
}

final class SincroCompletaTests: XCTestCase {
    private var raiz: URL!

    override func setUpWithError() throws {
        raiz = try carpetaTemporalSincro("sincro-completa")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: raiz)
    }

    private func nucleo(_ nombre: String) throws -> Nucleo {
        try Nucleo.abrir(raiz: raiz.appendingPathComponent(nombre).path, nombreDelAparato: nombre)
    }

    private func fichero(_ nombre: String, _ datos: Data) throws -> URL {
        let u = raiz.appendingPathComponent(nombre)
        try datos.write(to: u)
        return u
    }

    /// El PC crea un grupo y escucha con la `Puerta` de la app; el Mac se
    /// une por `host`.
    private func grupo(_ pc: Nucleo, _ mac: Nucleo, host: String = "127.0.0.1") throws -> Puerta {
        _ = try pc.crearGrupo()
        let puerta = try Puerta(nucleo: pc)
        puerta.arrancar { _ in }
        let codigo = try XCTUnwrap(try pc.aparato().grupo)
        _ = try mac.unirseAGrupo(host: host, puerto: puerta.puerto, codigo: codigo)
        return puerta
    }

    func testLaGaleriaViajaEnLaVuelta() throws {
        let pc = try nucleo("PC")
        let mac = try nucleo("Mac")
        let puerta = try grupo(pc, mac)
        defer { puerta.cerrar() }
        let capturas = URL(fileURLWithPath: pc.carpetaCapturas())
        try FileManager.default.createDirectory(at: capturas, withIntermediateDirectories: true)
        try Data("\u{89}PNG captura".utf8).write(to: capturas.appendingPathComponent("captura-20261015-101010.png"))

        let r = try mac.sincronizarCon(host: "127.0.0.1", puerto: puerta.puerto)
        XCTAssertGreaterThanOrEqual(r.capturas, 1)
        let pagina = try mac.galeria(desde: 0, cuantos: 10, dias: 7, desfaseMs: 0)
        XCTAssertEqual(pagina.capturas.map(\.nombre), ["captura-20261015-101010.png"])
        XCTAssertNotNil(pagina.capturas.first?.seVa)
    }

    func testEnviarAUnAparatoDelGrupoYUnaSondaALaVez() async throws {
        let pc = try nucleo("PC")
        let mac = try nucleo("Mac")
        let puerta = try grupo(pc, mac)
        defer { puerta.cerrar() }
        let obra = try pc.crearChat(nombre: "Obra")
        pc.ponerAlFrente(frente: .chat(id: obra.id, nombre: "Obra"))
        let foto = try fichero("foto.png", Data("\u{89}PNG foto".utf8))

        // Una vuelta y una sonda a la vez: la sonda no espera.
        let notas = try mac.crearChat(nombre: "Notas")
        for i in 0 ..< 20 { _ = try mac.escribirNota(chat: notas.id, texto: "nota \(i)", respondeA: nil) }
        let puertoPC = puerta.puerto
        async let vuelta = Task.detached { try mac.sincronizarCon(host: "127.0.0.1", puerto: puertoPC) }.value
        let punto = NWEndpoint.hostPort(host: "127.0.0.1", port: NWEndpoint.Port(rawValue: puertoPC)!)
        let d = await Vecinos.sondear(punto)
        XCTAssertEqual(d, Direccion(host: "127.0.0.1", puerto: puertoPC))
        let r = try await vuelta
        XCTAssertEqual(r.enviados, 20)

        let enviado = try mac.enviarAAparato(host: "127.0.0.1", puerto: puertoPC, rutas: [foto.path])
        XCTAssertEqual(enviado.first?.donde, .chatAbierto)
        XCTAssertEqual(enviado.first?.chat, "Obra")
        let m = try pc.ultimosMensajes(chat: obra.id, cuantos: 10)
        XCTAssertEqual(m.mensajes.last?.clase, "IMAGEN")
    }

    func testElEnvioConCodigoLlegaAMensajesGuardados() throws {
        let pc = try nucleo("PC")
        let mac = try nucleo("Mac")
        let notas = try fichero("notas.txt", Data("hola".utf8))
        let p = try pc.abrirEnvio(rutas: [notas.path])
        defer { p.cerrar() }
        p.aprobarTodos()
        XCTAssertTrue(p.textoDelQr(host: "192.168.1.4").hasPrefix("pixpin-envio:1:\(p.codigo()):192.168.1.4:"))
        let hecho = expectation(description: "enviado")
        atenderEnHilo(nombre: "prueba-envio", { try p.atender() }, esFin: { _ in true }) { r in
            if case .success(.enviado) = r { hecho.fulfill() }
        }
        let cosas = try mac.recibirEnvio(host: "127.0.0.1", puerto: p.puerto(), codigo: p.codigo())
        wait(for: [hecho], timeout: 10)
        XCTAssertEqual(cosas.map(\.donde), ["guardados"])
        let m = try mac.ultimosMensajes(chat: cosas[0].chat, cuantos: 5)
        XCTAssertEqual(m.mensajes.last?.nombre, "notas.txt")
    }

    func testUnChatCompartidoSeInvitaYSeEntra() throws {
        let ana = try nucleo("Ana")
        let beto = try nucleo("Beto")
        let tesis = try ana.crearChat(nombre: "Tesis")
        _ = try ana.escribirNota(chat: tesis.id, texto: "capitulo 1", respondeA: nil)
        let privado = try ana.crearChat(nombre: "Privado")
        _ = try ana.escribirNota(chat: privado.id, texto: "secreto", respondeA: nil)
        _ = try ana.compartirChat(ficha: tesis.id)
        let inv = try ana.invitarAChat(ficha: tesis.id)
        defer { inv.cerrar() }
        XCTAssertTrue(inv.textoDelQr(host: "fe80::1%en0").hasPrefix("pixpin-chat:1:\(inv.codigo())::"))
        let entro = expectation(description: "entro y se trajo el chat")
        entro.expectedFulfillmentCount = 2
        atenderEnHilo(nombre: "prueba-invitacion", { try inv.atender() }, esFin: { $0 == .cerrada }) { r in
            switch r {
            case .success(.entro), .success(.junto): entro.fulfill()
            default: break
            }
        }
        let dentro = try beto.entrarEnChatCompartido(codigo: inv.codigo(), host: "127.0.0.1", puerto: inv.puerto())
        wait(for: [entro], timeout: 10)
        XCTAssertTrue(dentro.estoyDentro)
        let suyos = try beto.chats()
        XCTAssertNotNil(suyos.first { $0.nombre == "Tesis" })
        XCTAssertNil(suyos.first { $0.nombre == "Privado" }, "la valla: nada de otros chats")
        XCTAssertTrue(try beto.hayChatsCompartidos())
        XCTAssertNotNil(try beto.anuncioDePresencia())
    }

    func testSeSincronizaPorIPv6SiLaMaquinaLoTiene() throws {
        // `::1` sin IPv6 (algunas maquinas de la CI): no hay nada que probar.
        let s = socket(AF_INET6, SOCK_STREAM, 0)
        guard s >= 0 else { throw XCTSkip("sin IPv6") }
        var dir = sockaddr_in6()
        dir.sin6_len = UInt8(MemoryLayout<sockaddr_in6>.size)
        dir.sin6_family = sa_family_t(AF_INET6)
        dir.sin6_addr = in6addr_loopback
        let enlazado = withUnsafePointer(to: &dir) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(s, $0, socklen_t(MemoryLayout<sockaddr_in6>.size))
            }
        }
        close(s)
        guard enlazado == 0 else { throw XCTSkip("sin IPv6 de bucle") }

        let pc = try nucleo("PC")
        let mac = try nucleo("Mac")
        let puerta = try grupo(pc, mac, host: "::1")
        defer { puerta.cerrar() }
        let obra = try mac.crearChat(nombre: "Obra")
        _ = try mac.escribirNota(chat: obra.id, texto: "por IPv6", respondeA: nil)
        let r = try mac.sincronizarCon(host: "[::1]", puerto: puerta.puerto)
        XCTAssertEqual(r.otroNombre, "PC")
        XCTAssertNotNil(try pc.chats().first { $0.nombre == "Obra" })
    }
}
