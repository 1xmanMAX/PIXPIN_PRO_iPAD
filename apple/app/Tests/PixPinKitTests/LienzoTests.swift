// El lienzo del iPad: las herramientas y la barra (datos, sin pantalla), el
// reloj del gesto de pararse (con un programador de mentira), y por el
// puente de verdad: dibujar, guardar 1 s despues y volver a abrir; la vista
// de UIKit que pinta; la miniatura; y el lienzo que nace con una foto.

import Foundation
import PixPinCore
@testable import PixPinKit
import SwiftUI
import UIKit
import XCTest

/// Una carpeta temporal nueva para una prueba.
private func carpetaTemporalLienzo(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-lienzo-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

/// Un trazo a mano de `pasos` avisos desde `(x, y)`.
private func trazo(_ ed: EditorLienzo, x: Float, y: Float, pasos: Int = 20) {
    _ = ed.puntero(fase: .pulsar, x: x, y: y, presion: 0.5, modificadores: 0, zoom: 1)
    for i in 1...pasos {
        let t = Float(i)
        _ = ed.puntero(fase: .mover, x: x + t * 5, y: y + sin(t * 0.6) * 15, presion: 0.6,
                       modificadores: 0, zoom: 1)
    }
    _ = ed.puntero(fase: .soltar, x: x + Float(pasos) * 5, y: y, presion: nil, modificadores: 0, zoom: 1)
}

/// Cuantos pixeles oscuros (y no transparentes) tiene una imagen, mirando
/// uno de cada `paso`.
private func oscuros(_ img: CGImage, paso: Int = 2) -> Int {
    let w = img.width, h = img.height
    guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                              space: CGColorSpaceCreateDeviceRGB(),
                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
          let datos = ctx.data else { return 0 }
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
    let p = datos.bindMemory(to: UInt8.self, capacity: w * h * 4)
    var n = 0
    for y in stride(from: 0, to: h, by: paso) {
        for x in stride(from: 0, to: w, by: paso) {
            let i = (y * w + x) * 4
            let brillo = (Int(p[i]) + Int(p[i + 1]) + Int(p[i + 2])) / 3
            if p[i + 3] > 128, brillo < 128 { n += 1 }
        }
    }
    return n
}

/// Un programador que no espera: apunta lo programado y lo dispara a mano.
private final class ProgramadorDePrueba {
    struct Pendiente {
        let segundos: Double
        let accion: () -> Void
        var cancelada = false
    }

    var pendientes: [Pendiente] = []

    var programar: RelojFormaRapida.Programar {
        { [unowned self] segundos, accion in
            self.pendientes.append(Pendiente(segundos: segundos, accion: accion))
            let i = self.pendientes.count - 1
            return { self.pendientes[i].cancelada = true }
        }
    }

    var vivas: [Pendiente] { pendientes.filter { !$0.cancelada } }
}

final class LienzoHerramientasTests: XCTestCase {
    func testLaBarraDelIPadNoLlevaCerrarYTodaHerramientaSeAlcanza() {
        XCTAssertFalse(BotonBarraLienzo.barraIPad.contains(.salir), "cierra la ✕ de la barra del visor")
        var alcanzables = Set<String>()
        for b in BotonBarraLienzo.barraIPad {
            switch b {
            case let .herramienta(id): alcanzables.insert(id)
            case let .grupo(g): g.miembros.forEach { alcanzables.insert($0.id) }
            default: break
            }
        }
        XCTAssertEqual(alcanzables, Set(HerramientaLienzo.todas.map(\.id)), "ninguna se queda sin boton")
        XCTAssertTrue(BotonBarraLienzo.barraIPad.contains(.deshacer))
        XCTAssertTrue(BotonBarraLienzo.barraIPad.contains(.rehacer))
    }

    func testEnUnIPadDePieLaBarraSeParteEnDosFilas() {
        let botones = BotonBarraLienzo.barraIPad
        // Un iPad de 11" de pie: 834 puntos menos los bordes.
        let corte = BotonBarraLienzo.cortePara(ancho: 834 - 32, en: botones)
        XCTAssertLessThanOrEqual(corte, botones.count)
        if corte < botones.count {
            XCTAssertNotEqual(botones[corte - 1].seccion, botones[corte].seccion, "se parte por un separador")
        }
        // De lado (1194) cabe en una.
        XCTAssertEqual(BotonBarraLienzo.cortePara(ancho: 1194 - 32, en: botones), botones.count)
    }

    func testLasLetrasYLosNumerosDeLosCajetines() {
        XCTAssertEqual(HerramientaLienzo.deTecla("L")?.id, "lapiz")
        XCTAssertEqual(HerramientaLienzo.deTecla("b")?.id, "borrador")
        XCTAssertNil(HerramientaLienzo.deTecla("k"))
        // La «W» es del angulo (Windows); extender no tiene letra.
        XCTAssertEqual(HerramientaLienzo.deTecla("W")?.id, "angulo")
        XCTAssertNil(HerramientaLienzo.con(id: "extender")?.tecla)
        XCTAssertEqual(HerramientaLienzo.con(id: "angulo")?.grupo, .nombrar)
        // Ninguna letra elige dos herramientas.
        let letras = HerramientaLienzo.herramientas.compactMap(\.tecla)
        XCTAssertEqual(Set(letras).count, letras.count)
        XCTAssertEqual(CajetinesLienzo.numero(" 2,5 "), 2.5)
        XCTAssertEqual(CajetinesLienzo.numero("3.25"), 3.25)
        XCTAssertNil(CajetinesLienzo.numero("dos"))
        XCTAssertNil(CajetinesLienzo.numero(""))
    }

    func testElColorDelSelectorLlegaAlMotorEnSRGB() throws {
        let c = try XCTUnwrap(OpcionesEstiloLienzo.estilo(de: Color(.sRGB, red: 1, green: 0.5, blue: 0, opacity: 1)))
        XCTAssertEqual(c.r, 1, accuracy: 0.01)
        XCTAssertEqual(c.g, 0.5, accuracy: 0.01)
        XCTAssertEqual(c.b, 0, accuracy: 0.01)
        XCTAssertTrue(OpcionesEstiloLienzo.igual(OpcionesEstiloLienzo.hex(0xe03131), OpcionesEstiloLienzo.trazos[1].valor))
    }

    func testElRelojDelGestoDePararse() {
        let p = ProgramadorDePrueba()
        let reloj = RelojFormaRapida(temblorPx: 8, programar: p.programar)
        reloj.pulsar(en: .zero, esperaMs: 550)
        XCTAssertEqual(p.vivas.count, 1)
        reloj.mover(a: CGPoint(x: 3, y: 3))
        XCTAssertEqual(p.vivas.count, 1, "el temblor de la mano no cuenta")
        reloj.mover(a: CGPoint(x: 40, y: 0))
        XCTAssertEqual(p.vivas.count, 1, "se reprograma: una sola viva")
        XCTAssertEqual(p.pendientes.count, 2)
        reloj.soltar()
        XCTAssertTrue(p.vivas.isEmpty)
        // Caso negativo: sin trazo a mano no hay espera.
        reloj.pulsar(en: .zero, esperaMs: nil)
        XCTAssertFalse(reloj.armado)
    }
}

final class LienzoEditorTests: XCTestCase {
    @MainActor
    func testDibujarConElModeloSeGuardaSoloYSeVuelveAAbrir() throws {
        let carpeta = try carpetaTemporalLienzo("guardar")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ruta = carpeta.appendingPathComponent("a.excalidraw").path
        let ed = try EditorLienzo.deFichero(ruta: ruta, crear: true)
        let modelo = ModeloEditorLienzo(editor: ed, referencia: "a")
        let vista = VistaEditorLienzoIPad(modelo: modelo)
        vista.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        XCTAssertEqual(modelo.herramienta, "lapiz")

        trazo(ed, x: 20, y: 20)
        let guardado = expectation(forNotification: .pixpinLienzoGuardado, object: nil) { aviso in
            (aviso.object as? String) == "a"
        }
        vista.trasCambio()
        XCTAssertTrue(modelo.puedeDeshacer)
        wait(for: [guardado], timeout: 5)
        XCTAssertFalse(ed.sinGuardar())
        XCTAssertEqual(try EditorLienzo.deFichero(ruta: ruta, crear: false).cuantosElementos(), 1)
        let json = try String(contentsOfFile: ruta, encoding: .utf8)
        XCTAssertTrue(json.contains("\"freedraw\""), "el formato de Windows")

        // Deshacer desde la barra, y la goma con el doble toque del Pencil.
        modelo.deshacer()
        XCTAssertEqual(ed.cuantosElementos(), 0)
        modelo.alternarGoma()
        XCTAssertEqual(modelo.herramienta, "borrador")
        modelo.alternarGoma()
        XCTAssertEqual(modelo.herramienta, "lapiz")
    }

    @MainActor
    func testLaVistaPintaLoQueDibujaElEditor() throws {
        let carpeta = try carpetaTemporalLienzo("vista")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("v.excalidraw").path, crear: true)
        trazo(ed, x: 20, y: 20, pasos: 30)
        let modelo = ModeloEditorLienzo(editor: ed)
        let vista = VistaEditorLienzoIPad(modelo: modelo)
        vista.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        vista.layoutIfNeeded()
        let formato = UIGraphicsImageRendererFormat()
        formato.scale = 1
        // `draw` pinta en el contexto de ahora (el del renderizador), con la
        // Y hacia abajo como en pantalla.
        let img = UIGraphicsImageRenderer(bounds: vista.bounds, format: formato).image { _ in
            vista.draw(vista.bounds)
        }
        let cg = try XCTUnwrap(img.cgImage)
        XCTAssertGreaterThan(oscuros(cg), 5, "el trazo tiene que verse")
    }

    func testLaMiniaturaPintaElDibujoEnUnMapaDeBits() throws {
        let carpeta = try carpetaTemporalLienzo("mini")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("m.excalidraw").path, crear: true)
        trazo(ed, x: 0, y: 0, pasos: 40)
        let leido = try leerLienzo(ed.plano())
        let img = try XCTUnwrap(MapaLienzo.imagen(leido, ancho: 440, alto: 308, margen: 16) { _ in nil })
        XCTAssertEqual(img.width, 440)
        XCTAssertEqual(img.height, 308)
        XCTAssertGreaterThan(oscuros(img), 10)
        // Caso negativo: un lienzo vacio es solo papel.
        let vacio = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("v.excalidraw").path, crear: true)
        let enBlanco = try XCTUnwrap(MapaLienzo.imagen(try leerLienzo(vacio.plano()), ancho: 100, alto: 70,
                                                       margen: 4) { _ in nil })
        XCTAssertEqual(oscuros(enBlanco), 0)
    }

    func testElLienzoNaceConLaFotoASuTamano() throws {
        let carpeta = try carpetaTemporalLienzo("foto")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        // Una foto de 300 x 200 (un PNG de verdad).
        let formato = UIGraphicsImageRendererFormat()
        formato.scale = 1
        let foto = UIGraphicsImageRenderer(size: CGSize(width: 300, height: 200), format: formato).pngData { c in
            UIColor.systemTeal.setFill()
            c.fill(CGRect(x: 0, y: 0, width: 300, height: 200))
        }
        let url = carpeta.appendingPathComponent("foto.png")
        try foto.write(to: url)
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("f.excalidraw").path, crear: true)
        try CargaLienzoIPad.ponerFoto(url, en: ed)
        XCTAssertEqual(ed.cuantosElementos(), 1)
        let caja = try XCTUnwrap(ed.caja())
        XCTAssertEqual(caja.x1 - caja.x0, 300, accuracy: 1, "a su tamano, sin reducir")
        XCTAssertEqual(caja.y1 - caja.y0, 200, accuracy: 1)
        XCTAssertTrue(try ed.guardar())
        // `imagenes()` es lo que habia al abrir: la foto se ve al volver a
        // abrir lo guardado (su fichero en `imagenes/` y su entrada en `files`).
        let otra = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("f.excalidraw").path, crear: false)
        XCTAssertEqual(otra.imagenes().count, 1)
        // Caso negativo: lo que no es una imagen no entra.
        let texto = carpeta.appendingPathComponent("nota.png")
        try Data("hola".utf8).write(to: texto)
        try CargaLienzoIPad.ponerFoto(texto, en: ed)
        XCTAssertEqual(ed.cuantosElementos(), 1)
    }

    func testUnLienzoNuevoEnElChatSeEditaYSeGuardaEnSuSitio() throws {
        let raiz = try carpetaTemporalLienzo("chat")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        let chat = try nucleo.crearChat(nombre: "Obra")
        let m = try nucleo.crearLienzo(chat: chat.id, nombre: nil)
        XCTAssertEqual(m.clase, "DIBUJO")
        let referencia = try XCTUnwrap(m.referencia)
        let ed = try nucleo.editarLienzo(chat: chat.id, dibujo: referencia)
        trazo(ed, x: 0, y: 0)
        XCTAssertTrue(try ed.guardar())
        XCTAssertEqual(try nucleo.abrirLienzo(chat: chat.id, dibujo: referencia).cuantosElementos(), 1)
    }
}
