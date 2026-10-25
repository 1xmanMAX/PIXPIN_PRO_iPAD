// El lienzo del iPad, tanda de las letras y de sacar el dibujo: el catalogo
// de las ocho letras (y que vienen dentro de la app), que hojas salen en
// cada formato, los nombres de los ficheros, y por el puente de verdad: el
// SVG, la pagina web, el PDF y el PNG de un lienzo con un trazo; el fondo
// fijo y el papel de noche.

import Foundation
import PixPinCore
@testable import PixPinKit
import UIKit
import XCTest

private func carpetaTemporalSacar(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-sacar-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

private func trazoSacar(_ ed: EditorLienzo) {
    _ = ed.puntero(fase: .pulsar, x: 10, y: 10, presion: 0.5, modificadores: 0, zoom: 1)
    for i in 1...20 {
        _ = ed.puntero(fase: .mover, x: 10 + Float(i) * 5, y: 10 + Float(i % 5) * 4, presion: 0.6,
                       modificadores: 0, zoom: 1)
    }
    _ = ed.puntero(fase: .soltar, x: 110, y: 10, presion: nil, modificadores: 0, zoom: 1)
}

final class LienzoLetrasSacarTests: XCTestCase {
    func testElCatalogoDeLasOchoLetras() {
        XCTAssertEqual(LetrasLienzo.catalogo.map(\.nombre),
                       ["Excalifont", "Nunito", "Lilita One", "Comic Shanns", "Work Sans", "Fraunces",
                        "Courier New", "Caveat"])
        XCTAssertEqual(LetrasLienzo.letra(de: "excalifont")?.nombre, "Excalifont")
        XCTAssertEqual(LetrasLienzo.letra(de: "Comic Shanns 2")?.nombre, "Comic Shanns", "el alias del movil")
        XCTAssertNil(LetrasLienzo.letra(de: "Segoe UI"))
        // La linea base de Excalifont: 0,886 + (1,25 - 1,26) / 2.
        XCTAssertEqual(LetrasLienzo.letra(de: "Excalifont")?.lineaBase ?? 0, 0.881, accuracy: 0.001)
    }

    func testLasLetrasVienenDentroDeLaApp() {
        for nombre in LetrasLienzo.catalogo.map(\.nombre) {
            XCTAssertTrue(LetrasLienzo.existe(nombre), "falta la letra \(nombre)")
        }
        let negrita = LetrasLienzo.cara(familia: "Nunito", tam: 20, negrita: true, cursiva: false)
        XCTAssertFalse(negrita.falsaNegrita, "Nunito trae su negrita")
        let falsa = LetrasLienzo.cara(familia: "Excalifont", tam: 20, negrita: true, cursiva: false)
        XCTAssertTrue(falsa.falsaNegrita, "Excalifont no: se finge")
    }

    func testRenglonesConSuSaltoYPartidos() {
        let c = LetrasLienzo.cara(familia: "Work Sans", tam: 20, negrita: false, cursiva: false)
        let a: [NSAttributedString.Key: Any] = [NSAttributedString.Key(kCTFontAttributeName as String): c.fuente]
        XCTAssertEqual(LetrasLienzo.renglones("uno\n\ntres", fuente: c.fuente, anchoMax: 1e9, atributos: a).count, 3)
        let largo = String(repeating: "palabra ", count: 30)
        XCTAssertGreaterThan(LetrasLienzo.renglones(largo, fuente: c.fuente, anchoMax: 200, atributos: a).count, 3)
    }

    func testQueHojasSalenEnCadaFormato() {
        XCTAssertEqual(FormatoSalidaLienzo.allCases.map(\.titulo), ["HTML", "PDF", "PNG", "JPG", "SVG", ".pixpin"])
        XCTAssertEqual(SacarLienzo.ques(.png, hayElegidos: true, hayMarcos: true), [.seleccion])
        XCTAssertEqual(SacarLienzo.ques(.png, hayElegidos: false, hayMarcos: true), [.todo])
        XCTAssertEqual(SacarLienzo.ques(.web, hayElegidos: false, hayMarcos: true), [.todo, .marcos])
        XCTAssertEqual(SacarLienzo.ques(.pdf, hayElegidos: false, hayMarcos: false), [.todo])
        XCTAssertEqual(SacarLienzo.nombre("Planta", 0, de: 1, extension: "svg"), "Planta.svg")
        XCTAssertEqual(SacarLienzo.nombre("Planta", 1, de: 3, extension: "png"), "Planta-2.png")
        XCTAssertFalse(SacarLienzo.nombre("a/b:c", 0, de: 1, extension: "pdf").contains("/"))
        XCTAssertEqual(ColorLienzo(r: 1, g: 0, b: 0.5, a: 1).hex, "#ff0080")
    }

    func testElDibujoDeLaHojaPagina() {
        XCTAssertTrue(CargaLienzoIPad.esSuDibujo(nil, "abc"))
        XCTAssertTrue(CargaLienzoIPad.esSuDibujo("/x/lienzos/abc.excalidraw", "abc"))
        XCTAssertFalse(CargaLienzoIPad.esSuDibujo("/x/lienzos/otro.excalidraw", "abc"))
    }

    @MainActor
    func testSacarEnTodosLosFormatos() throws {
        let carpeta = try carpetaTemporalSacar("formatos")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("s.excalidraw").path, crear: true)
        trazoSacar(ed)
        let modelo = ModeloEditorLienzo(editor: ed, titulo: "Prueba")
        let vista = VistaEditorLienzoIPad(modelo: modelo)
        vista.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        for f in FormatoSalidaLienzo.allCases where f != .pixpin {
            let urls = modelo.ficheros(f, en: carpeta)
            XCTAssertEqual(urls.count, 1, "\(f)")
            XCTAssertEqual(urls.first?.pathExtension, f.sufijo)
            let datos = try Data(contentsOf: XCTUnwrap(urls.first))
            XCTAssertGreaterThan(datos.count, 100, "\(f)")
        }
        let html = try String(contentsOf: carpeta.appendingPathComponent("Prueba.html"), encoding: .utf8)
        XCTAssertTrue(html.contains("<svg"), "las hojas van dentro")
        XCTAssertNotNil(modelo.imagenParaPin())
    }

    @MainActor
    func testElFondoFijoYElPapelDeNoche() throws {
        let carpeta = try carpetaTemporalSacar("fondo")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        // Una foto de 300 x 200 de fondo.
        let ctx = try XCTUnwrap(MapaLienzo.mapa(300, 200))
        ctx.setFillColor(CGColor(red: 0, green: 0.3, blue: 0.8, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: 300, height: 200))
        let png = try XCTUnwrap(ctx.makeImage().flatMap(PintorDeHojas.png))
        let url = carpeta.appendingPathComponent("f.png")
        try png.write(to: url)
        let fondo = try XCTUnwrap(FondoLienzo(.imagen(url)))
        XCTAssertEqual(fondo.tamano, CGSize(width: 300, height: 200))
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("f.excalidraw").path, crear: true)
        let modelo = ModeloEditorLienzo(editor: ed, fondo: fondo)
        // Sin nada dibujado, el fondo sale igual (es el papel).
        XCTAssertFalse(ed.hojasParaExportar(que: .todo).isEmpty)
        // Con fondo, el modo noche no oscurece el papel.
        let antes = ModoNoche.compartido.activo
        defer { ModoNoche.compartido.activo = antes }
        ModoNoche.compartido.activo = true
        XCTAssertNil(modelo.papelDeVista)
        XCTAssertTrue(modelo.verOscuro, "la barra va de noche")
        let suelto = ModeloEditorLienzo(editor: try EditorLienzo.deFichero(
            ruta: carpeta.appendingPathComponent("g.excalidraw").path, crear: true))
        XCTAssertEqual(suelto.papelDeVista, ModeloEditorLienzo.papelDeNoche)
        ModoNoche.compartido.activo = false
        XCTAssertNil(suelto.papelDeVista)
    }
}
