// Pruebas del asistente del iPad: la nube por URLSession con un
// `URLProtocol` falso (nunca una API de pago) y la clave en el Llavero, que
// sin clave lo dice, las manos de la app (tareas, «¿seguro?»), el historial,
// el asistente en el chat (con `turnoDePrueba`, sin modelo) y los pedidos.

import Foundation
import PixPinCore
import UIKit
import XCTest
@testable import PixPinKit

/// Claude de mentira: contesta a `api.anthropic.com` sin salir a la red.
final class ClaudeDeMentira: URLProtocol {
    /// Lo que se le pidio (cabeceras en minusculas y el cuerpo).
    nonisolated(unsafe) static var pedidos: [(cabeceras: [String: String], cuerpo: [String: Any])] = []
    nonisolated(unsafe) static var codigo = 200

    static func sse(_ eventos: [[String: Any]]) -> Data {
        var s = ""
        for e in eventos {
            let d = try! JSONSerialization.data(withJSONObject: e)
            s += "event: \(e["type"] as! String)\ndata: \(String(decoding: d, as: UTF8.self))\n\n"
        }
        return Data(s.utf8)
    }

    static func respuesta(_ texto: String) -> Data {
        sse([
            ["type": "message_start", "message": ["id": "msg_1", "type": "message", "role": "assistant",
                                                  "usage": ["input_tokens": 10, "output_tokens": 1]]],
            ["type": "content_block_start", "index": 0, "content_block": ["type": "text", "text": ""]],
            ["type": "content_block_delta", "index": 0, "delta": ["type": "text_delta", "text": texto]],
            ["type": "content_block_stop", "index": 0],
            ["type": "message_delta", "delta": ["stop_reason": "end_turn"], "usage": ["output_tokens": 5]],
            ["type": "message_stop"],
        ])
    }

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.host == "api.anthropic.com"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        var cuerpo = Data()
        if let s = request.httpBodyStream {
            s.open()
            var buf = [UInt8](repeating: 0, count: 65536)
            while s.hasBytesAvailable {
                let n = s.read(&buf, maxLength: buf.count)
                if n <= 0 { break }
                cuerpo.append(buf, count: n)
            }
            s.close()
        } else if let b = request.httpBody {
            cuerpo = b
        }
        let json = (try? JSONSerialization.jsonObject(with: cuerpo)) as? [String: Any] ?? [:]
        var cab: [String: String] = [:]
        for (k, v) in request.allHTTPHeaderFields ?? [:] { cab[k.lowercased()] = v }
        ClaudeDeMentira.pedidos.append((cab, json))
        let codigo = ClaudeDeMentira.codigo
        let datos: Data
        if codigo != 200 {
            datos = Data(#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.utf8)
        } else if json["stream"] as? Bool == true {
            datos = ClaudeDeMentira.respuesta("Hola desde la nube.")
        } else {
            datos = Data(#"{"id":"msg_1","type":"message","model":"claude-haiku-5-5","content":[{"type":"text","text":"ok"}]}"#.utf8)
        }
        let r = HTTPURLResponse(url: request.url!, statusCode: codigo, httpVersion: "HTTP/1.1",
                                headerFields: ["Content-Type": codigo == 200 && json["stream"] as? Bool == true
                                    ? "text/event-stream" : "application/json"])!
        client?.urlProtocol(self, didReceive: r, cacheStoragePolicy: .notAllowed)
        // A trozos, como la red.
        var i = 0
        while i < datos.count {
            let fin = min(i + 97, datos.count)
            client?.urlProtocol(self, didLoad: datos.subdata(in: i ..< fin))
            i = fin
        }
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

/// Lo que llega en tandas (de otro hilo).
final class TandasDelAsistente: @unchecked Sendable {
    private let cerrojo = NSLock()
    private var lista: [String] = []

    func meter(_ t: String) {
        cerrojo.lock()
        lista.append(t)
        cerrojo.unlock()
    }

    var todas: [String] {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return lista
    }
}

private func carpetaDePrueba(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-ipad-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

final class AsistenteIPadNubeTests: XCTestCase {
    override func setUp() {
        let c = URLSessionConfiguration.ephemeral
        c.protocolClasses = [ClaudeDeMentira.self]
        TransporteHttps.configuracion = c
        TransporteHttps.instalar()
        ClaudeDeMentira.pedidos = []
        ClaudeDeMentira.codigo = 200
    }

    /// La clave en el Llavero de pruebas (un servicio propio: no toca las
    /// de verdad). Sin Llavero utilizable (simulador sin app), se salta.
    private func conClave() throws {
        XCTAssertTrue(LlaveroDelSistema.instalar(LlaveroDelSistema.dePruebas))
        do {
            try iaGuardarClave(proveedorId: "claude", clave: "sk-ant-api03-PRUEBA1234567890abcd")
        } catch {
            throw XCTSkip("No hay Llavero utilizable aquí: \(error)")
        }
    }

    private func ajustes(_ motor: String) -> AjustesIa {
        AjustesIa(motor: motor, modelo: "", pensar: false, respaldoLocal: false, mejorVista: false,
                  documentosALaNube: "", carpetaModelos: "", puertoUsuario: 9)
    }

    func testHablarConClaudeConLaClaveDelLlavero() throws {
        try conClave()
        defer { try? iaBorrarClave(proveedorId: "claude") }
        XCTAssertTrue(iaHayClave(proveedorId: "claude"))
        let enmascarada = try XCTUnwrap(iaClaveEnmascarada(proveedorId: "claude"))
        XCTAssertFalse(enmascarada.contains("PRUEBA1234567890"), "nunca se ensena entera: \(enmascarada)")
        guard case let .nube(proveedor, _, modelo, conClave) = iaQueMotor(ajustes: ajustes("claude")) else {
            return XCTFail("la nube")
        }
        XCTAssertEqual(proveedor, "claude")
        XCTAssertTrue(conClave)
        let tandas = TandasDelAsistente()
        let oyente = OyenteDelAsistente(alTexto: { tandas.meter($0) })
        let corte = CorteIa.nuevo()
        let manos = ManosDelAsistente(nucleo: nil, corte: corte) { _, r in r(false) }
        let peticion = PeticionIa(historial: [], pedido: "saluda", foto: nil, adjuntos: [], hoja: "", desfaseMs: 0)
        let r = try iaTurno(ajustes: ajustes("claude"), peticion: peticion, nucleo: nil, app: manos,
                            oyente: oyente, corte: corte)
        XCTAssertEqual(r.texto, "Hola desde la nube.")
        XCTAssertEqual(tandas.todas.joined(), r.texto, "llega en vivo")
        let pedido = try XCTUnwrap(ClaudeDeMentira.pedidos.last)
        XCTAssertEqual(pedido.cabeceras["x-api-key"], "sk-ant-api03-PRUEBA1234567890abcd")
        XCTAssertEqual(pedido.cuerpo["model"] as? String, modelo)
        XCTAssertEqual(pedido.cuerpo["stream"] as? Bool, true)
    }

    func testProbarLaClaveYUnaClaveMala() throws {
        try conClave()
        defer { try? iaBorrarClave(proveedorId: "claude") }
        XCTAssertNoThrow(try iaProbarClave(proveedorId: "claude"))
        ClaudeDeMentira.codigo = 401
        XCTAssertThrowsError(try iaProbarClave(proveedorId: "claude"))
        // Sin clave guardada, ni se llama.
        try iaBorrarClave(proveedorId: "claude")
        XCTAssertFalse(iaHayClave(proveedorId: "claude"))
        let antes = ClaudeDeMentira.pedidos.count
        XCTAssertThrowsError(try iaProbarClave(proveedorId: "claude"))
        XCTAssertEqual(ClaudeDeMentira.pedidos.count, antes)
    }

    @MainActor
    func testElModeloContestaConClaudeYSinClaveLoDice() async throws {
        let dir = try carpetaDePrueba("asistente-modelo")
        let almacen = try Almacen(raiz: dir, nombreDelAparato: "iPad de prueba")
        var a = try almacen.nucleo.ajustes()
        a.asistenteMotor = "claude"
        try almacen.nucleo.guardarAjustes(nuevos: a)
        let modelo = ModeloAsistente(almacen: almacen, puertoLlama: 9, usarApple: false)
        // Sin clave: la burbuja lo dice y la vista ofrece ponerla.
        XCTAssertTrue(LlaveroDelSistema.instalar(LlaveroDelSistema.dePruebas))
        try? iaBorrarClave(proveedorId: "claude")
        let sin = await modelo.comprobarMotor()
        XCTAssertTrue(sin.faltaClave)
        await modelo.enviar("hola")
        XCTAssertEqual(modelo.burbujas.count, 2)
        XCTAssertTrue(modelo.burbujas[1].esAviso)
        // Con clave: contesta Claude (el falso), en vivo y con su motor.
        try conClave()
        defer { try? iaBorrarClave(proveedorId: "claude") }
        let con = await modelo.comprobarMotor()
        XCTAssertFalse(con.faltaClave)
        await modelo.enviar("saluda")
        let ultima = try XCTUnwrap(modelo.burbujas.last)
        XCTAssertFalse(ultima.delUsuario)
        XCTAssertFalse(ultima.esAviso, ultima.texto)
        XCTAssertEqual(ultima.texto, "Hola desde la nube.")
        XCTAssertFalse(modelo.ocupado)
        XCTAssertFalse(modelo.sePuedeParar)
    }

    @MainActor
    func testElegirElMotorCambiaLosAjustes() async throws {
        let dir = try carpetaDePrueba("asistente-motor")
        let almacen = try Almacen(raiz: dir, nombreDelAparato: "iPad de prueba")
        let modelo = ModeloAsistente(almacen: almacen, puertoLlama: 9, usarApple: false)
        _ = await modelo.elegirMotor("deepseek")
        XCTAssertEqual(try almacen.nucleo.ajustes().asistenteMotor, "deepseek")
        XCTAssertEqual(modelo.motorElegido, "deepseek")
        // Local sin Apple Intelligence: no hay nadie y se dice como tenerlo.
        let nadie = await modelo.elegirMotor("")
        XCTAssertEqual(nadie, .ninguno)
        XCTAssertTrue(nadie.faltaClave)
        XCTAssertTrue(ModeloAsistente.textoSinMotor.contains("clave"))
    }
}

final class AsistenteIPadManosTests: XCTestCase {
    @MainActor
    func testLasTareasSeHacenAquiYLoDemasSeDice() async throws {
        let dir = try carpetaDePrueba("asistente-tareas")
        let nucleo = try Nucleo.abrir(raiz: dir.path, nombreDelAparato: "iPad de prueba")
        let antes = PedidosDelAsistente.hacer
        PedidosDelAsistente.hacer = nil
        defer { PedidosDelAsistente.hacer = antes }
        let r = await ManosDelAsistente.hacerAqui(#"{"pixpin":1,"accion":"anadir_tarea","texto":"comprar pan"}"#, nucleo: nucleo)
        XCTAssertNil(r)
        let lista = try XCTUnwrap(nucleo.tareas(desfaseMs: 0).first)
        XCTAssertEqual(lista.tareas.first?.texto, "comprar pan")
        let marcar = #"{"pixpin":1,"accion":"marcar_tarea","codigo":"\#(lista.mensaje)","indice":0,"hecha":true}"#
        let marcada = await ManosDelAsistente.hacerAqui(marcar, nucleo: nucleo)
        XCTAssertNil(marcada)
        XCTAssertEqual(try nucleo.tareas(desfaseMs: 0).first?.tareas.first?.hecha, true)
        // Casos negativos.
        let otra = await ManosDelAsistente.hacerAqui(#"{"pixpin":1,"accion":"capturar"}"#, nucleo: nucleo)
        XCTAssertTrue(otra?.contains("iPad") ?? false)
        let rota = await ManosDelAsistente.hacerAqui("roto", nucleo: nucleo)
        XCTAssertNotNil(rota)
        XCTAssertFalse(ManosDelAsistente.esDeTareas(#"{"pixpin":1,"accion":"nota_nueva"}"#))
        XCTAssertNotNil(ManosDelAsistente.aLaApp(#"{"accion":"nota_nueva"}"#), "sin version no se entiende")
    }

    func testConfirmarEsperaElSiYParadoEsUnNo() async throws {
        let corte = CorteIa.nuevo()
        let si = ManosDelAsistente(nucleo: nil, corte: corte) { _, responder in responder(true) }
        let r = await Task.detached { si.confirmar(pregunta: "¿Borro la lista?") }.value
        XCTAssertTrue(r)
        let nadie = ManosDelAsistente(nucleo: nil, corte: corte) { _, _ in }
        corte.cancelar()
        let r2 = await Task.detached { nadie.confirmar(pregunta: "¿Seguro?") }.value
        XCTAssertFalse(r2)
    }

    func testExportarSinQuienSepaLoDice() async throws {
        let dir = try carpetaDePrueba("asistente-exportar")
        let f = dir.appendingPathComponent("a.txt")
        try Data("hola".utf8).write(to: f)
        let manos = ManosDelAsistente(nucleo: nil, corte: CorteIa.nuevo()) { _, r in r(false) }
        let carpeta = dir.path
        let e = await Task.detached { () -> String? in
            do {
                _ = try manos.exportar(ruta: f.path, formato: "pdf", carpeta: carpeta)
                return nil
            } catch {
                return ModeloAsistente.texto(de: error)
            }
        }.value
        XCTAssertTrue(e?.contains("iPad") ?? false, e ?? "-")
    }

    @MainActor
    func testLasConversacionesSeGuardanYSeLeen() throws {
        let dir = try carpetaDePrueba("asistente-historial")
        let h = HistorialAsistente(carpeta: dir)
        var c = Conversacion()
        h.guardar(c)
        XCTAssertTrue(h.todas().isEmpty, "una vacia no se guarda")
        var b = ModeloAsistente.Burbuja(delUsuario: false, texto: "Listo.")
        b.pensamiento = "pienso"
        b.llamando = "Dibujando…"
        c.burbujas = [ModeloAsistente.Burbuja(delUsuario: true, texto: "dibuja una casa\ncon jardin"), b]
        h.guardar(c)
        let leida = try XCTUnwrap(h.todas().first)
        XCTAssertEqual(leida.rotulo, "dibuja una casa")
        XCTAssertEqual(leida.burbujas[1].pensamiento, "pienso")
        XCTAssertNil(leida.burbujas[1].llamando, "lo de en vivo no se guarda")
        h.borrar(c.id)
        XCTAssertTrue(h.todas().isEmpty)
    }

    @MainActor
    func testLaFotoSeReduceYLoDeFueraSeCopia() throws {
        let dir = try carpetaDePrueba("asistente-foto")
        let png = dir.appendingPathComponent("grande.png")
        let img = UIGraphicsImageRenderer(size: CGSize(width: 2000, height: 1000),
                                          format: { let f = UIGraphicsImageRendererFormat(); f.scale = 1; return f }())
            .image { c in UIColor.red.setFill(); c.fill(CGRect(x: 0, y: 0, width: 2000, height: 1000)) }
        try XCTUnwrap(img.pngData()).write(to: png)
        let foto = try XCTUnwrap(ModeloAsistente.foto(png.path))
        XCTAssertEqual(foto.mime, "image/jpeg")
        let leida = try XCTUnwrap(UIImage(data: foto.bytes)?.cgImage)
        XCTAssertEqual(max(leida.width, leida.height), 768)
        XCTAssertNil(ModeloAsistente.foto(dir.appendingPathComponent("no.png").path))
        let modelo = ModeloAsistente(almacen: nil, usarApple: false, historial: HistorialAsistente(carpeta: dir.appendingPathComponent("conv")))
        modelo.adjuntar([png])
        let a = try XCTUnwrap(modelo.pendientes.first)
        XCTAssertTrue(a.esImagen)
        XCTAssertNotEqual(a.ruta, png.path, "lo de fuera de la carpeta de datos se copia")
        XCTAssertTrue(FileManager.default.fileExists(atPath: a.ruta))
    }
}

@MainActor
final class AsistenteIPadEnElChatTests: XCTestCase {
    override func tearDown() {
        MainActor.assumeIsolated { AsistenteEnElChat.turnoDePrueba = nil }
        super.tearDown()
    }

    func testEnSuChatContestaYEnOtroSoloConLaMencion() async throws {
        let dir = try carpetaDePrueba("e1")
        let a = try Almacen(raiz: dir, nombreDelAparato: "iPad de prueba")
        await a.cargarChats()
        await a.asegurarAsistente()
        let chat = try XCTUnwrap(a.chats.first { $0.esAsistente }).id
        await a.abrir(chat: chat)
        AsistenteEnElChat.turnoDePrueba = { nucleo, chat, inicio, _, _, _ in
            let r = try nucleo.iaResponder(chat: chat, nota: inicio.respuesta.id, texto: "Hola, soy el asistente.", lienzo: nil)
            return FinChatIa(respuesta: r, abrirLienzo: nil)
        }
        let mE = await a.escribir(nota: "hola", en: chat)
        let m = try XCTUnwrap(mE)
        let turnos = AsistenteEnElChat.compartido.alEnviar([m], chat: chat, almacen: a)
        XCTAssertEqual(turnos.count, 1)
        XCTAssertTrue(AsistenteEnElChat.compartido.ocupado(chat))
        for t in turnos { await t.value }
        let ultimo = try XCTUnwrap(a.mensajes[chat]?.last)
        XCTAssertEqual(ultimo.texto, "Hola, soy el asistente.")
        XCTAssertTrue(ultimo.esDelAsistente)
        XCTAssertTrue(AsistenteEnElChat.compartido.alEnviar([ultimo], chat: chat, almacen: a).isEmpty,
                      "su respuesta no se contesta")
        // En otro chat, solo con @asistente.
        await a.crearChat(nombre: "obra")
        let obra = try XCTUnwrap(a.chats.first { $0.nombre == "obra" }).id
        await a.abrir(chat: obra)
        let holaE = await a.escribir(nota: "hola a todos", en: obra)
        let hola = try XCTUnwrap(holaE)
        XCTAssertTrue(AsistenteEnElChat.compartido.alEnviar([hola], chat: obra, almacen: a).isEmpty)
        let preguntaE = await a.escribir(nota: "@asistente resume esto", en: obra)
        let pregunta = try XCTUnwrap(preguntaE)
        for t in AsistenteEnElChat.compartido.alEnviar([pregunta], chat: obra, almacen: a) { await t.value }
        XCTAssertEqual(a.mensajes[obra]?.last?.respondeA, pregunta.id)
    }
}

@MainActor
final class AsistenteIPadPedidosTests: XCTestCase {
    func testLosNombresDeProyectoSeCambianPorSuId() {
        let proyectos = [(id: "c1", nombre: "Obra", guardados: false), (id: "g", nombre: "Mensajes guardados", guardados: true)]
        let p = HacerPedidos.ponerIds([Pedido("nota_nueva", ["proyecto": .texto("obra")]),
                                       Pedido("nota_nueva", ["proyecto": .texto("guardados")]),
                                       Pedido("nota_nueva", ["proyecto": .texto("no-esta")])], proyectos: proyectos)
        XCTAssertEqual(p[0]["proyecto"], .texto("c1"))
        XCTAssertEqual(p[1]["proyecto"], .nulo)
        XCTAssertEqual(p[2]["proyecto"], .texto("no-esta"), "lo que no se encuentra se deja (y se dira)")
        XCTAssertEqual(HacerPedidos.sinFichas("mira [img 01]  esto [archivo 2]"), "mira esto")
    }

    func testUnEnlaceDeNotaNuevaLaEscribeYLaAbre() async throws {
        let dir = try carpetaDePrueba("pedidos")
        let a = try Almacen(raiz: dir, nombreDelAparato: "iPad de prueba")
        await a.cargarChats()
        let h = HacerPedidos()
        var avisos: [String] = []
        h.avisar = { avisos.append($0) }
        h.confirmar = { _, responder in responder(true) }
        h.registrar(almacen: a, sincro: nil)
        let nav = Navegacion.compartida
        nav.cerrarVisor()
        XCTAssertTrue(h.abrir(enlace: URL(string: "pixpinmax://nota_nueva?texto=Hola%20iPad")!))
        XCTAssertFalse(h.abrir(enlace: URL(string: "https://example.com")!), "no es nuestro")
        // El pedido se hace en una tarea: se espera a que abra el visor.
        for _ in 0 ..< 100 where nav.visor == nil { try await Task.sleep(for: .milliseconds(20)) }
        guard case let .nota(_, m)? = nav.visor else { return XCTFail("abre la nota: \(avisos)") }
        XCTAssertEqual(m.texto, "Hola iPad")
        nav.cerrarVisor()
    }
}
