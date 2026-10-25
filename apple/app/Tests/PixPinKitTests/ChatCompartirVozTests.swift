// Lo puro de la zona Proyectos/Chat/Voz/Compartir del iPad: las reglas de la
// hoja de compartir, los ficheros que hace (PDF, PNG, JPG, SVG, web, texto),
// el exportar del asistente, «Escuchar», el cuadro de enviar fotos, la foto
// anotada, las tarjetas de enlace, el panel Información, el logo del
// proyecto, la biblioteca de audio y su cola.

import ImageIO
import PDFKit
import PixPinCore
import UIKit
import UniformTypeIdentifiers
import XCTest
@testable import PixPinKit

private func carpetaTemporalZona(_ nombre: String) throws -> URL {
    let u = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
    return u
}

private func escribirPNGZona(_ url: URL, ancho: Int, alto: Int) throws {
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

private func mensaje(_ id: String, clase: String = "NOTA", texto: String = "", nombre: String = "",
                     cuando: Int64 = 0, ruta: String? = nil, transcripcion: String? = nil,
                     fijado: Bool = false, enBuzon: Bool = false) -> MensajeChat {
    MensajeChat(id: id, cuando: cuando, clase: clase, texto: texto, nombre: nombre, ruta: ruta, bytes: 0,
                resumen: texto, codigoChat: nil, respondeA: nil, emoji: nil, fijado: fijado,
                referencia: nil, pagina: nil, duracionMs: 0, transcripcion: transcripcion, picos: [],
                enBuzon: enBuzon, recuerdaEn: nil, miniapp: nil, esLeccion: false,
                sePuedeRenombrar: false, sePuedeDescribir: false, punto: nil, soloLaFoto: true,
                vieneDe: nil, vieneDeDibujo: nil, recibidoDe: nil, lienzoIa: nil)
}

@MainActor
final class ChatCompartirVozTests: XCTestCase {
    private func compartible(paginas: Int) -> Compartible {
        Compartible(
            titulo: "Obra",
            paginas: (0 ..< paginas).map { PaginaCompartir(clave: "p\($0)", nombre: "Página \($0)", detalle: "") },
            formatos: [
                FormatoCompartir(id: IdFormato.pdf, nombre: "PDF", cuantas: .varias, simbolo: ""),
                FormatoCompartir(id: IdFormato.png, nombre: "PNG", cuantas: .una, simbolo: ""),
                FormatoCompartir(id: IdFormato.original, nombre: "Original", cuantas: .ninguna, simbolo: ""),
            ],
            marcadas: nil)
    }

    // MARK: La hoja de compartir

    func testConVariasMarcadasLosFormatosDeUnaPaginaDesaparecen() {
        let c = compartible(paginas: 3)
        var e = EstadoHoja(c)
        XCTAssertEqual(e.visibles(c), [0, 2])
        e.elegirFormato(c, 1)
        XCTAssertEqual(e.marcadas, ["p0"], "un formato de una pagina se queda con la primera")
        e.tocarPagina(c, "p2")
        XCTAssertEqual(e.elegidas(c), ["p2"])
        e.tocarPagina(c, "no-esta")
        XCTAssertEqual(e.marcadas, ["p2"])
    }

    func testUnaNotaSaleEnPdfPngSvgWebYTexto() async throws {
        let p = PreparadoCompartir(titulo: "Lista: compra")
        PrepararCompartir.anadirTexto(p, clave: "m1", nombre: "Lista", detalle: "Nota", titulo: nil,
                                      texto: (0 ..< 120).map { "Cosa \($0)" }.joined(separator: "\n"), solo: true)
        XCTAssertGreaterThan(p.piezas.count, 1)
        let c = p.compartible()
        XCTAssertEqual(c.formatos.map(\.id), [IdFormato.web, IdFormato.pdf, IdFormato.png, IdFormato.jpg,
                                              IdFormato.svg, IdFormato.texto])
        let carpeta = try carpetaTemporalZona("compartir")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let todas = c.paginas.map(\.clave)

        let pdf = try await p.generar(IdFormato.pdf, claves: todas, en: carpeta)
        XCTAssertEqual(PDFDocument(url: pdf[0])?.pageCount, p.piezas.count)
        let png = try await p.generar(IdFormato.png, claves: [todas[0]], en: carpeta)
        let img = try XCTUnwrap(UIImage(contentsOfFile: png[0].path)?.cgImage)
        XCTAssertEqual(img.width, Int((PiezasCompartir.a4.width * 2).rounded()))
        let svg = try await p.generar(IdFormato.svg, claves: [todas[0]], en: carpeta)
        XCTAssertTrue(try String(contentsOf: svg[0], encoding: .utf8).contains("Cosa 0"))
        let web = try await p.generar(IdFormato.web, claves: todas, en: carpeta)
        XCTAssertTrue(try String(contentsOf: web[0], encoding: .utf8).contains("Cosa 0"))
        let texto = try await p.generar(IdFormato.texto, claves: [], en: carpeta)
        XCTAssertTrue(try String(contentsOf: texto[0], encoding: .utf8).hasPrefix("Cosa 0\nCosa 1"))
        // Caso negativo: sin paginas marcadas no hay PDF.
        do {
            _ = try await p.generar(IdFormato.pdf, claves: [], en: carpeta)
            XCTFail("sin paginas no hay PDF")
        } catch {
            XCTAssertEqual(error.localizedDescription, "Marca alguna página")
        }
    }

    func testUnaFotoSueltaVaPrimeroEnteraYSeExportaAJpg() async throws {
        let carpeta = try carpetaTemporalZona("compartir-foto")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let foto = carpeta.appendingPathComponent("fachada.png")
        try escribirPNGZona(foto, ancho: 300, alto: 200)
        let p = await PrepararCompartir.preparar(.ficheros(titulo: "fachada", [foto]), almacen: nil)
        XCTAssertEqual(p.compartible().formatos.first?.id, IdFormato.original)
        XCTAssertEqual(p.piezas.first?.tamano, CGSize(width: 300, height: 200))
        XCTAssertEqual(PiezasCompartir.anchoEnPixeles(foto), 300)

        // El exportar del asistente: a JPG, en su carpeta.
        let salida = carpeta.appendingPathComponent("hecho", isDirectory: true)
        let jpg = try await ExportarConLaHoja.exportar(foto, formato: "JPEG", en: salida)
        XCTAssertEqual(jpg.map(\.pathExtension), ["jpg"])
        XCTAssertEqual(jpg.first?.deletingLastPathComponent().standardizedFileURL, salida.standardizedFileURL)
        // El original tambien acaba en su carpeta (se copia).
        let original = try await ExportarConLaHoja.exportar(foto, formato: "original", en: salida)
        XCTAssertEqual(original.first?.lastPathComponent, "fachada.png")
        XCTAssertEqual(original.first?.deletingLastPathComponent().standardizedFileURL, salida.standardizedFileURL)
        // Caso negativo: a Word no se sabe.
        do {
            _ = try await ExportarConLaHoja.exportar(foto, formato: "docx", en: salida)
            XCTFail("a docx no se exporta")
        } catch {}
    }

    func testLosFormatosQuePideElAsistente() {
        XCTAssertEqual(ExportarConLaHoja.idDeFormato("HTML"), IdFormato.web)
        XCTAssertEqual(ExportarConLaHoja.idDeFormato(".jpeg"), IdFormato.jpg)
        XCTAssertEqual(ExportarConLaHoja.idDeFormato("pdf"), IdFormato.pdf)
        XCTAssertEqual(ExportarConLaHoja.idDeFormato("svg"), IdFormato.svg)
        XCTAssertNil(ExportarConLaHoja.idDeFormato("docx"))
        XCTAssertNil(ExportarConLaHoja.idDeFormato(""))
    }

    // MARK: El chat

    func testLoQueSeEscuchaDeCadaMensaje() {
        XCTAssertEqual(ChatAbierto.textoParaEscuchar(mensaje("a", texto: "  Hola  ")), "Hola")
        XCTAssertEqual(ChatAbierto.textoParaEscuchar(mensaje("v", clase: "VOZ", texto: "pie", transcripcion: "dicho")),
                       "dicho", "una nota de voz lee lo dicho, no su pie")
        XCTAssertNil(ChatAbierto.textoParaEscuchar(mensaje("v2", clase: "VOZ", transcripcion: nil)))
        XCTAssertNil(ChatAbierto.textoParaEscuchar(mensaje("m", clase: "MINIAPP", texto: "{\"x\":1}")))
        XCTAssertEqual(ChatAbierto.textoParaEscuchar(mensaje("f", clase: "IMAGEN", texto: "La fachada")), "La fachada")
        XCTAssertEqual(ChatAbierto.parrafosParaEscuchar("Uno\n\n  Dos \n"), ["Uno", "Dos"])
        XCTAssertTrue(ChatAbierto.parrafosParaEscuchar(" \n ").isEmpty)
    }

    func testElCuadroDeEnviarFotosReordenaYDiceCuantas() {
        XCTAssertEqual(OrdenDeEnvio.mover(["a", "b", "c", "d"], desde: 3, hasta: 0), ["d", "a", "b", "c"])
        XCTAssertEqual(OrdenDeEnvio.mover(["a", "b", "c"], desde: 0, hasta: 2), ["b", "c", "a"])
        XCTAssertEqual(OrdenDeEnvio.mover(["a", "b"], desde: 0, hasta: 7), ["a", "b"], "fuera de la lista no cambia")
        XCTAssertEqual(OrdenDeEnvio.resumen(fotos: 3, videos: 0, proyecto: "Obra"), "3 fotos · a Obra")
        XCTAssertEqual(OrdenDeEnvio.resumen(fotos: 1, videos: 0, proyecto: "Obra"), "1 foto · a Obra")
        XCTAssertEqual(OrdenDeEnvio.resumen(fotos: 1, videos: 1, proyecto: ""), "2 archivos")
        XCTAssertEqual(OrdenDeEnvio.resumen(fotos: 0, videos: 2, proyecto: "X"), "2 vídeos · a X")
    }

    func testLaCajaDeUnaFotoAnotada() {
        let foto = CGRect(x: 10, y: 10, width: 100, height: 50)
        let todo = CGRect(x: 0, y: 0, width: 200, height: 200)
        XCTAssertEqual(cajaDeFotoAnotada(foto: foto, todo: todo, soloLaFoto: true), foto)
        XCTAssertEqual(cajaDeFotoAnotada(foto: foto, todo: todo, soloLaFoto: false), todo)
        XCTAssertEqual(cajaDeFotoAnotada(foto: nil, todo: todo, soloLaFoto: true), todo)
        XCTAssertNil(cajaDeFotoAnotada(foto: nil, todo: nil, soloLaFoto: true))
        XCTAssertEqual(tamanoDeFotoAnotada(CGSize(width: 400, height: 200), lado: 260), CGSize(width: 260, height: 130))
        let alta = tamanoDeFotoAnotada(CGSize(width: 100, height: 400), lado: 260)
        XCTAssertEqual(alta.height, 260 * 1.4, accuracy: 0.01)
        XCTAssertLessThan(alta.width, 260)
    }

    func testLaTarjetaDeUnEnlace() {
        XCTAssertEqual(TarjetasDeEnlace.primerEnlace("mira https://www.ejemplo.com/a?b=1 y ya")?.host, "www.ejemplo.com")
        XCTAssertNil(TarjetasDeEnlace.primerEnlace("sin enlaces aquí"))
        XCTAssertNil(TarjetasDeEnlace.primerEnlace("correo: http@no"), "solo direcciones web")
        XCTAssertEqual(TarjetasDeEnlace.sitio(URL(string: "https://www.pixpin.app/x")!), "pixpin.app")
    }

    func testLaFichaDelChatCuentaLosEnlacesComoElNucleo() throws {
        let carpeta = try carpetaTemporalZona("ficha")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let nucleo = try Nucleo.abrir(raiz: carpeta.path, nombreDelAparato: "iPad")
        let chat = try nucleo.crearChat(nombre: "Casa Lima").id
        _ = try nucleo.escribirNota(chat: chat, texto: "mira https://github.com/forge/pixpin", respondeA: nil)
        _ = try nucleo.escribirNota(chat: chat, texto: "sin enlace", respondeA: nil)
        let f = try nucleo.fichaDelChat(chat: chat, actual: nil)
        XCTAssertEqual(f.cosas, 2)
        XCTAssertEqual(f.pestanas.first { $0.seccion == .enlaces }?.cuenta, 1)
        // Caso negativo: sin videos no sale «Vídeos 0».
        XCTAssertNil(f.pestanas.first { $0.seccion == .videos })
        let enlaces = try nucleo.cosasDeLaFicha(chat: chat, seccion: .enlaces, buscado: "")
        XCTAssertEqual(enlaces.first?.enlace?.donde, "github.com")
        XCTAssertTrue(try nucleo.cosasDeLaFicha(chat: chat, seccion: .enlaces, buscado: "nada-de-esto").isEmpty)
        XCTAssertEqual(TextosFicha.rotulo(.videos), "Vídeos")
        XCTAssertEqual(TextosFicha.resumen(cosas: 1, codigo: ""), "1 cosa")
    }

    // MARK: Proyectos

    func testElLogoEsElCuadradoCentralReducido() throws {
        XCTAssertEqual(LogosProyecto.recorteCentral(ancho: 400, alto: 200), CGRect(x: 100, y: 0, width: 200, height: 200))
        XCTAssertEqual(LogosProyecto.recorteCentral(ancho: 100, alto: 300), CGRect(x: 0, y: 100, width: 100, height: 100))
        let carpeta = try carpetaTemporalZona("logo")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let foto = carpeta.appendingPathComponent("logo-origen.png")
        try escribirPNGZona(foto, ancho: 900, alto: 600)
        let png = try XCTUnwrap(LogosProyecto.cuadrado(try Data(contentsOf: foto), lado: LogosProyecto.lado))
        let img = try XCTUnwrap(UIImage(data: png)?.cgImage)
        XCTAssertEqual(img.width, 256)
        XCTAssertEqual(img.height, 256)
        // Caso negativo: lo que no es una imagen no da logo.
        XCTAssertNil(LogosProyecto.cuadrado(Data("no soy una foto".utf8), lado: 256))
    }

    // MARK: Voz

    func testLaBibliotecaSoloTraeVozConFicheroYLaMusicaPrimero() {
        let filas = BibliotecaAudio.ordenar(BibliotecaAudio.filas([
            mensaje("n1", clase: "VOZ", nombre: "nota.m4a", cuando: 1, ruta: "a/n1.m4a"),
            mensaje("m1", clase: "VOZ", nombre: "cancion.m4a", cuando: 0, ruta: "a/m1.m4a"),
            mensaje("n2", clase: "VOZ", nombre: "otra.m4a", cuando: 5, ruta: "a/n2.m4a"),
            mensaje("sin", clase: "VOZ", nombre: "sin.m4a", cuando: 9, ruta: nil),
            mensaje("buzon", clase: "VOZ", cuando: 9, ruta: "a/b.m4a", enBuzon: true),
            mensaje("foto", clase: "IMAGEN", cuando: 9, ruta: "a/f.png"),
        ], chat: "c", musica: ["m1"]))
        XCTAssertEqual(filas.map(\.id), ["m1", "n2", "n1"])
        XCTAssertEqual(filas.first?.grupo, .musica)
        XCTAssertEqual(filas.first?.titulo, "cancion")
    }

    func testLaColaNoRepiteYSinNadaSuenaYa() throws {
        let r = ReproductorVoz()
        r.mudo = true
        let carpeta = try carpetaTemporalZona("cola")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        // Sin un audio de verdad no se carga: la cola tampoco se queda con el.
        r.encolar(Pista(id: "x", url: carpeta.appendingPathComponent("no.m4a"), titulo: "x"))
        XCTAssertNil(r.actual)
        XCTAssertTrue(r.cola.isEmpty)
        r.quitarDeLaCola("x")
        r.siguiente()
        XCTAssertNil(r.actual)
    }
}
