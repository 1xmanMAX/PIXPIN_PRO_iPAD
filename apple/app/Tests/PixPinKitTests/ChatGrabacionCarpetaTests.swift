// Lo traido del Mac (06fe7d9..a0efbae): la carpeta de una grabacion y el
// chat sin sueltos (`Chat/CarpetasChat.swift`, Windows e015e2b y e1150e8),
// y compartir una grabacion con la hoja de toda la app
// (`Compartir/CompartirGrabacion.swift`, Windows 2921a1d), por el puente de
// verdad.

import ImageIO
import PDFKit
import PixPinCore
import UniformTypeIdentifiers
import XCTest
@testable import PixPinKit

private func carpetaTemporalGrabacion(_ nombre: String) throws -> URL {
    let u = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-ipad-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
    return u
}

private func escribirPNGGrabacion(_ url: URL, ancho: Int, alto: Int) throws {
    let ctx = try XCTUnwrap(CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    ctx.setFillColor(CGColor(red: 0.2, green: 0.5, blue: 0.8, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: ancho, height: alto))
    let img = try XCTUnwrap(ctx.makeImage())
    let d = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil))
    CGImageDestinationAddImage(d, img, nil)
    XCTAssertTrue(CGImageDestinationFinalize(d))
}

@MainActor
final class ChatGrabacionCarpetaTests: XCTestCase {
    func testLaClaseGuardadaVaEnSuCarpetaYNoSaleSuelta() async throws {
        let raiz = try carpetaTemporalGrabacion("grabacion-carpeta")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "iPad de prueba")
        await almacen.crearChat(nombre: "Física")
        let chat = try XCTUnwrap(almacen.chats.first { $0.nombre == "Física" }).id
        let audio = raiz.appendingPathComponent("clase.m4a")
        try Data("m4a".utf8).write(to: audio)
        let png = raiz.appendingPathComponent("a.png")
        try Data("png".utf8).write(to: png)
        let mp4 = raiz.appendingPathComponent("clase.m4a.video.mp4")
        try Data("mp4".utf8).write(to: mp4)
        let hecho = try almacen.nucleo.guardarClase(chat: chat, rutaAudio: audio.path, duracionMs: 60_000, picos: [1, 2],
                                                    rutaVideo: mp4.path,
                                                    capturas: [CapturaClase(ms: 65_400, rutaPng: png.path)], desfaseMs: 0)
        let voz = try XCTUnwrap(hecho.voz)
        let video = try XCTUnwrap(hecho.video)
        // Detras de todo, su carpeta (Windows e015e2b): una sola tarjeta, y
        // la nota, el video y la captura no salen sueltos en el chat.
        let carpeta = try XCTUnwrap(hecho.carpeta)
        XCTAssertTrue(CarpetasDelChat.esGrabacion(carpeta))
        XCTAssertEqual(CarpetasDelChat.detalle(carpeta), "1:00 · 1 captura")
        let todo = [voz, video] + hecho.capturas + [carpeta]
        XCTAssertEqual(CarpetasDelChat.sinLoDeDentro(todo, salvo: nil).map(\.id), [carpeta.id])
        XCTAssertEqual(CarpetasDelChat.sinLoDeDentro(todo, salvo: voz.id).map(\.id), [voz.id, carpeta.id],
                       "al ir a ella desde su carpeta, si se ve")
        XCTAssertEqual(ClaseEnElChat.deUnaClase(carpeta, chat: chat, nucleo: almacen.nucleo)?.voz, voz.id)
        // Su ventana: lo grabado en el orden del tiempo, con su minuto.
        let leida = try XCTUnwrap(await CarpetasDelChat.leer(carpeta.id, chat: chat, almacen: almacen))
        XCTAssertTrue(leida.grabacion)
        XCTAssertEqual(leida.voz, voz.id)
        XCTAssertFalse(leida.paginas.contains { $0.adjunto })
        // Caso negativo: sin carpetas se ve todo; un cuaderno no es una grabacion.
        XCTAssertEqual(CarpetasDelChat.sinLoDeDentro([voz, video], salvo: nil).count, 2)
        let cuaderno = try almacen.nucleo.crearCarpetaDePaginas(chat: chat, primera: nil)
        XCTAssertFalse(CarpetasDelChat.esGrabacion(cuaderno))
        XCTAssertEqual(CarpetasDelChat.detalle(cuaderno), "Vacía")
    }

    func testElIconoDeUnaPaginaDistingueElVideoYLaVoz() {
        func m(_ clase: String, _ nombre: String) -> MensajeChat {
            MensajeChat(id: "x", cuando: 0, clase: clase, texto: "", nombre: nombre, ruta: nil, bytes: 0,
                        resumen: "", codigoChat: nil, respondeA: nil, emoji: nil, fijado: false,
                        referencia: nil, pagina: nil, duracionMs: 0, transcripcion: nil, picos: [],
                        enBuzon: false, recuerdaEn: nil, miniapp: nil, esLeccion: false,
                        sePuedeRenombrar: false, sePuedeDescribir: false, punto: nil, soloLaFoto: true,
                        vieneDe: nil, vieneDeDibujo: nil, recibidoDe: nil, lienzoIa: nil)
        }
        XCTAssertEqual(CarpetasChat.icono(m("ARCHIVO", "Clase.MP4")), "video")
        XCTAssertEqual(CarpetasChat.icono(m("VOZ", "Clase.m4a")), "mic")
        // Caso negativo: un PDF no es un video.
        XCTAssertEqual(CarpetasChat.icono(m("ARCHIVO", "apuntes.pdf")), "doc")
        XCTAssertEqual(CarpetasChat.icono(nil), "square.and.pencil")
    }

    func testUnaGrabacionSaleConSuPaginaWebYSusFormatos() async throws {
        let raiz = try carpetaTemporalGrabacion("compartir-grabacion")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "iPad de prueba")
        await almacen.crearChat(nombre: "Física")
        let chat = try XCTUnwrap(almacen.chats.first { $0.nombre == "Física" }).id
        let captura = raiz.appendingPathComponent("c1.png")
        try escribirPNGGrabacion(captura, ancho: 80, alto: 60)
        let audio = raiz.appendingPathComponent("clase.m4a")
        try Data("aac".utf8).write(to: audio)
        let g = GrabacionParaCompartir(
            titulo: "Clase 1", fecha: "", cuando: 0, duracionMs: 60000, audio: audio.path, video: nil,
            capturas: [CapturaDeGrabacion(ms: 10000, ruta: captura.path)],
            lineas: [LineaDeGrabacion(ms: 0, texto: "hola"), LineaDeGrabacion(ms: 20000, texto: "adios")],
            marcadores: [MarcadorDeGrabacion(ms: 5000, texto: "ojo")], adjuntos: [],
            originales: [audio.path, captura.path], mazo: "PixPin::Física")
        let p = await PrepararCompartir.deGrabacion(g, chat: chat, almacen: almacen)
        let c = p.compartible()
        XCTAssertEqual(c.formatos.map(\.id), [IdFormato.web, IdFormato.pdf, IdFormato.png, IdFormato.jpg,
                                              IdFormato.svg, IdFormato.word, IdFormato.md, IdFormato.texto,
                                              IdFormato.anki, IdFormato.originales, IdFormato.pixpin])
        XCTAssertEqual(c.formatos[0].cuantas, .ninguna, "la pagina web va entera")
        XCTAssertEqual(c.paginas.first?.clave, "grabacion")
        XCTAssertEqual(p.piezas[0].tamano, PiezasCompartir.a4)
        let carpeta = raiz.appendingPathComponent("salida")
        let web = try await p.generar(IdFormato.web, claves: [], en: carpeta)
        XCTAssertEqual(web.map(\.lastPathComponent), ["Clase 1.html"])
        XCTAssertTrue(try String(contentsOf: web[0], encoding: .utf8).contains("data:image/png;base64,"))
        let pdf = try await p.generar(IdFormato.pdf, claves: c.paginas.map(\.clave), en: carpeta)
        XCTAssertEqual(PDFDocument(url: pdf[0])?.pageCount, p.piezas.count)
        let svg = try await p.generar(IdFormato.svg, claves: ["grabacion"], en: carpeta)
        XCTAssertTrue(try String(contentsOf: svg[0], encoding: .utf8).contains("<svg"))
        let texto = try await p.generar(IdFormato.texto, claves: [], en: carpeta)
        XCTAssertTrue(try String(contentsOf: texto[0], encoding: .utf8).contains("[0:20] adios"))
        let md = try await p.generar(IdFormato.md, claves: [], en: carpeta)
        XCTAssertEqual(md.map(\.lastPathComponent), ["Clase 1 (Markdown).zip"])
        let word = try await p.generar(IdFormato.word, claves: [], en: carpeta)
        XCTAssertEqual(word[0].pathExtension, "docx")
        // Caso negativo: sin capturas no hay Anki.
        var sin = g
        sin.capturas = []
        let otra = await PrepararCompartir.deGrabacion(sin, chat: chat, almacen: almacen)
        XCTAssertFalse(otra.compartible().formatos.contains { $0.id == IdFormato.anki })
        XCTAssertEqual(PrepararCompartir.fechaDeGrabacion(0).count, 16, "dd/MM/yyyy HH:mm")
    }
}
