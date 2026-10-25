// Pruebas de los pedidos del iPad (§7 del inventario): `al_lienzo` (verbo,
// enlace, buscar el lienzo por su nombre y apilar), `mover_tarea`,
// `posponer`, adonde llevar un fichero al abrirlo (`IrEnElLector`), la
// ficha de una leccion nueva, el aviso flotante y vaciar el chat del
// asistente.

import Foundation
import PixPinCore
import UIKit
import XCTest
@testable import PixPinKit

private func carpeta(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-ipad-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

final class PedidosIPadPurasTests: XCTestCase {
    func testAlLienzoSeArmaComoEnWindows() throws {
        guard case let .success(l)? = Verbos.deArgumentos(["al-lienzo", "Plano casa", "la", "puerta"], esFichero: { _ in false })
        else { return XCTFail("al-lienzo se entiende") }
        guard case let .pedidos(p) = l.orden else { return XCTFail("da pedidos") }
        XCTAssertEqual(p.first?.accion, "al_lienzo")
        XCTAssertEqual(p.first?["lienzo"], .texto("Plano casa"))
        XCTAssertEqual(p.first?["texto"], .texto("la puerta"))
        // Con --lienzo, todo lo suelto es texto.
        guard case let .success(l2)? = Verbos.deArgumentos(["al-lienzo", "--lienzo", "Obra", "hola"], esFichero: { _ in false }),
              case let .pedidos(p2) = l2.orden else { return XCTFail("con --lienzo") }
        XCTAssertEqual(p2.first?["lienzo"], .texto("Obra"))
        XCTAssertEqual(p2.first?["texto"], .texto("hola"))
        // Sin nada que mandar, no.
        guard case .failure? = Verbos.deArgumentos(["al-lienzo", "Obra"], esFichero: { _ in false }) else {
            return XCTFail("sin texto ni imagenes no se arma")
        }
        // El pedido escrito a mano tambien vale (la accion ya existe).
        XCTAssertNoThrow(try Pedido.de(json: #"{"pixpin":1,"accion":"al_lienzo","lienzo":"Plano","texto":"la puerta"}"#))
        // Por enlace pregunta antes (cambia un lienzo).
        let e = try Enlaces.leer("pixpinmax://al-lienzo?lienzo=Plano&texto=hola")
        guard case let .pedidos(pe) = e.orden, let uno = pe.first else { return XCTFail("enlace") }
        XCTAssertEqual(Enlaces.seguridad(uno), .confirmar("cambia"))
    }

    @MainActor
    func testLaPeticionDelLectorSeLeeYSeTomaUnaVez() {
        let p = IrEnElLector.peticion(de: ["tipo": .texto("fichero"), "pagina": .numero(12),
                                           "buscar": .texto("  JR. ANTISUYO "),
                                           "zona": .lista([.numero(10), .numero(20), .numero(300)])])
        XCTAssertEqual(p.pagina, 12)
        XCTAssertEqual(p.indice, 11, "PDFKit cuenta desde 0")
        XCTAssertEqual(p.buscar, "JR. ANTISUYO")
        XCTAssertEqual(p.zona?.ancho, 300)
        XCTAssertFalse(IrEnElLector.Peticion(pagina: 0, buscar: "  ").algo, "pagina 0 y un texto en blanco no piden nada")
        XCTAssertNil(IrEnElLector.peticion(de: ["zona": .lista([.numero(1), .numero(2)])]).zona)

        let url = URL(fileURLWithPath: "/tmp/a/../plano.pdf")
        var avisado = false
        let vigia = NotificationCenter.default.addObserver(forName: IrEnElLector.cambio, object: nil, queue: nil) { n in
            if IrEnElLector.esPara(n, URL(fileURLWithPath: "/tmp/plano.pdf")) { avisado = true }
        }
        IrEnElLector.pedir(p, para: url)
        NotificationCenter.default.removeObserver(vigia)
        XCTAssertTrue(avisado, "el visor abierto se entera (con la ruta ya normalizada)")
        XCTAssertEqual(IrEnElLector.tomar(URL(fileURLWithPath: "/tmp/plano.pdf")), p)
        XCTAssertNil(IrEnElLector.tomar(url), "una vez")
    }

    func testLaPilaDelLienzo() {
        XCTAssertEqual(HacerPedidos.tamanoEnLaPila(ancho: 960, alto: 480).0, 480)
        XCTAssertEqual(HacerPedidos.tamanoEnLaPila(ancho: 960, alto: 480).1, 240)
        XCTAssertEqual(HacerPedidos.tamanoEnLaPila(ancho: 100, alto: 50).0, 100, "lo pequeno no se agranda")
        let (x, y) = HacerPedidos.sitioDeLaPila(caja: CajaLienzo(x0: -10, y0: 5, x1: 200, y1: 300), pila: nil)
        XCTAssertEqual(x, 280, "a la derecha de todo, con aire")
        XCTAssertEqual(y, 5)
        let (x2, y2) = HacerPedidos.sitioDeLaPila(caja: nil, pila: (x: 7, y: 9))
        XCTAssertEqual(x2, 7)
        XCTAssertEqual(y2, 9)
        XCTAssertEqual(HacerPedidos.sitioDeLaPila(caja: nil, pila: nil).0, 0)
        XCTAssertEqual(HacerPedidos.nombreNormal("  Plano_Cása "), "plano casa")
    }

    func testPosponerLeccionYTitulo() {
        XCTAssertEqual(HacerPedidos.minutosDePosponer(nil), CentroAvisos.posponerMinutos)
        XCTAssertEqual(HacerPedidos.minutosDePosponer(0), 1)
        XCTAssertEqual(HacerPedidos.minutosDePosponer(1_000_000), 7 * 24 * 60)
        let b = HacerPedidos.borradorLeccion("Revisar el encofrado\nSe abrió la junta\ny hubo que repetir")
        XCTAssertEqual(b.titulo, "Revisar el encofrado")
        XCTAssertEqual(b.quePaso, "Se abrió la junta\ny hubo que repetir")
        XCTAssertEqual(HacerPedidos.borradorLeccion("").titulo, "")
        XCTAssertEqual(HacerPedidos.tituloDelEncargo(""), "🤖 Asistente")
        XCTAssertEqual(HacerPedidos.tituloDelEncargo(String(repeating: "a", count: 60)).count, 2 + 48)
        XCTAssertEqual(CabeceraDelAsistente.sinLosDelSistema(["auto-bienvenida", "m1", "m2"]), ["m1", "m2"])
        let proyectos = [(id: "c1", nombre: "Obra", guardados: false)]
        let p = HacerPedidos.ponerIds([Pedido("mover_tarea", ["a_proyecto": .texto("obra")])], proyectos: proyectos)
        XCTAssertEqual(p[0]["a_proyecto"], .texto("c1"), "el destino de mover tambien va por nombre")
    }

    @MainActor
    func testElAvisoFlotante() {
        var dichos: [String] = []
        AvisoFlotante.alMostrar = { titulo, texto in dichos.append(titulo + ": " + texto) }
        defer { AvisoFlotante.alMostrar = nil }
        let t = AvisoFlotante.compartido.mostrar("💭 Pensando…", titulo: "🤖 Hola", acabado: false)
        XCTAssertEqual(dichos, ["🤖 Hola: 💭 Pensando…"])
        XCTAssertFalse(t.acabado)
        var dijoNo = false
        t.cambiar("Necesito tu permiso: ¿borro?", botones: [BotonAviso("Sí") {}], alCerrar: { dijoNo = true })
        XCTAssertFalse(t.acabado, "esperando un «Sí» no se va sola")
        t.aspa()
        XCTAssertTrue(dijoNo, "el aspa con la pregunta a la vista es un «No»")
        XCTAssertTrue(t.cerrada)
        t.poner("ya no cambia")
        XCTAssertNotEqual(t.texto, "ya no cambia")
        XCTAssertEqual(AvisoFlotante.recortar(String(repeating: "x", count: 3000)).count, AvisoFlotante.letras + 1)
        XCTAssertLessThanOrEqual(AvisoFlotante.duraDe(String(repeating: "x", count: 5000)), 10)
    }
}

@MainActor
final class PedidosIPadHechosTests: XCTestCase {
    private func preparar(_ nombre: String) async throws -> (Almacen, HacerPedidos, Avisos) {
        let a = try Almacen(raiz: try carpeta(nombre), nombreDelAparato: "iPad de prueba")
        await a.cargarChats()
        let h = HacerPedidos()
        let avisos = Avisos()
        h.avisar = { avisos.lista.append($0) }
        h.confirmar = { _, responder in responder(true) }
        h.registrar(almacen: a, sincro: nil)
        CuadrosIPad.alAvisar = { avisos.lista.append($0) }
        return (a, h, avisos)
    }

    final class Avisos { var lista: [String] = [] }

    func testMoverUnaTareaAOtraLista() async throws {
        let (a, h, avisos) = try await preparar("mover")
        defer { CuadrosIPad.alAvisar = nil }
        let n = a.nucleo
        let g = try n.chatGuardados().id
        let desfase = desfaseLocalMs()
        let origen = try n.crearMiniapp(chat: g, miniapp: "tareas", titulo: "Hoy", moneda: "EUR")
        let destino = try n.crearMiniapp(chat: g, miniapp: "tareas", titulo: "Compras", moneda: "EUR")
        let doc = try n.documentoMiniapp(chat: g, mensaje: origen.id)
        XCTAssertTrue(try n.guardarMiniapp(chat: g, mensaje: origen.id, anterior: doc,
                                           documento: miniTareasAnadir(documento: doc, texto: "comprar pan", desfaseMs: desfase)))
        await h.hacerYa(Pedido("mover_tarea", ["proyecto": .nulo, "codigo": .texto(origen.id), "indice": .numero(0),
                                               "a_proyecto": .nulo, "a_codigo": .texto(destino.id)]))
        let quedan = miniTareasLeer(documento: try n.documentoMiniapp(chat: g, mensaje: origen.id), desfaseMs: desfase)
        let llegan = miniTareasLeer(documento: try n.documentoMiniapp(chat: g, mensaje: destino.id), desfaseMs: desfase)
        XCTAssertTrue(quedan.isEmpty, "\(avisos.lista)")
        XCTAssertEqual(llegan.map(\.texto), ["comprar pan"])
        XCTAssertEqual(avisos.lista.last, "«comprar pan» movida a «Compras».")
        // Una que no esta, se dice.
        await h.hacerYa(Pedido("mover_tarea", ["proyecto": .nulo, "codigo": .texto(origen.id), "indice": .numero(4),
                                               "a_proyecto": .nulo, "a_codigo": .texto(destino.id)]))
        XCTAssertEqual(avisos.lista.last, "La lista no tiene la tarea número 4.")
    }

    func testPosponerUnaTareaLePoneLaHora() async throws {
        let (a, h, avisos) = try await preparar("posponer")
        defer { CuadrosIPad.alAvisar = nil }
        let n = a.nucleo
        let g = try n.chatGuardados().id
        let desfase = desfaseLocalMs()
        let lista = try n.crearMiniapp(chat: g, miniapp: "tareas", titulo: "Hoy", moneda: "EUR")
        let doc = try n.documentoMiniapp(chat: g, mensaje: lista.id)
        XCTAssertTrue(try n.guardarMiniapp(chat: g, mensaje: lista.id, anterior: doc,
                                           documento: miniTareasAnadir(documento: doc, texto: "llamar al yesero", desfaseMs: desfase)))
        await h.hacerYa(Pedido("posponer", ["proyecto": .nulo, "mensaje": .texto(lista.id), "indice": .numero(0),
                                            "minutos": .numero(30)]))
        let t = miniTareasLeer(documento: try n.documentoMiniapp(chat: g, mensaje: lista.id), desfaseMs: desfase).first
        XCTAssertNotNil(t?.recuerda, "la tarea lleva su ⏰: \(avisos.lista)")
        XCTAssertTrue(avisos.lista.last?.hasPrefix("⏰ Te lo recordaré") == true)
    }

    func testBuscarElLienzoPorSuNombre() async throws {
        let (a, _, _) = try await preparar("lienzos")
        defer { CuadrosIPad.alAvisar = nil }
        let n = a.nucleo
        let g = try n.chatGuardados().id
        let casa = try n.crearLienzo(chat: g, nombre: "Plano casa")
        let plano = try n.crearLienzo(chat: g, nombre: "Plano")
        let todos = try n.mensajes(chat: g, desde: 0, cuantos: 100).mensajes
        XCTAssertEqual(HacerPedidos.lienzoPorNombre("plano", en: todos)?.0.id, plano.id, "mejor el que se llama justo asi")
        XCTAssertEqual(HacerPedidos.lienzoPorNombre("PLANO_CASA", en: todos)?.0.id, casa.id)
        XCTAssertEqual(HacerPedidos.lienzoPorNombre("casa", en: todos)?.1, false, "el que lo contiene, si no hay otro")
        XCTAssertNil(HacerPedidos.lienzoPorNombre("garaje", en: todos))
        XCTAssertEqual(HacerPedidos.buscarLienzo("Plano casa", en: [g], nucleo: n)?.1.id, casa.id)
    }

    func testAlLienzoCreaElLienzoYApilaLaFoto() async throws {
        let (a, h, avisos) = try await preparar("al-lienzo")
        defer { CuadrosIPad.alAvisar = nil }
        let n = a.nucleo
        let foto = FileManager.default.temporaryDirectory.appendingPathComponent("pixpin-pila-\(UUID().uuidString).png")
        let img = UIGraphicsImageRenderer(size: CGSize(width: 40, height: 30)).image { c in
            UIColor.red.setFill()
            c.fill(CGRect(x: 0, y: 0, width: 40, height: 30))
        }
        try XCTUnwrap(img.pngData()).write(to: foto)
        await h.hacerYa(Pedido("al_lienzo", ["lienzo": .texto("Obra nueva"), "proyecto": .nulo,
                                             "imagenes": .textos([foto.path])]))
        let g = try n.chatGuardados().id
        let todos = try n.mensajes(chat: g, desde: 0, cuantos: 100).mensajes
        let lienzo = try XCTUnwrap(HacerPedidos.lienzoPorNombre("Obra nueva", en: todos)?.0, "se crea: \(avisos.lista)")
        let editor = try n.editarLienzo(chat: g, dibujo: try XCTUnwrap(lienzo.referencia))
        XCTAssertEqual(editor.imagenes().count, 1, "\(avisos.lista)")
        editor.cerrado()
        XCTAssertEqual(avisos.lista.last, "Apilado en el lienzo «Obra nueva».")
    }
}
