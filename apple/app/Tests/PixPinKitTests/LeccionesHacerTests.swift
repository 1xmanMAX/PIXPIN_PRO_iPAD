// «Hacer lección» desde un mensaje: la ficha nueva trae lo que reparte el
// nucleo (`leccionDeMensaje`) o, si no sabe, el texto del mensaje, y queda
// enlazada a el.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

final class LeccionesHacerTests: XCTestCase {
    func testLaFichaTraeLoDelNucleo() {
        let l = LeccionDeMensaje(titulo: "Medir dos veces", quePaso: "Corte mal", porQue: "Prisa", proxima: "Si…",
                                 etiquetas: ["obra"], deMensaje: "m1", chat: "c1")
        let b = HacerLeccionIPad.borrador(chat: "c1", mensaje: "m1", de: l, texto: "otra cosa")
        XCTAssertEqual(b.titulo, "Medir dos veces")
        XCTAssertEqual(b.quePaso, "Corte mal")
        XCTAssertEqual(b.etiquetas, ["obra"])
        XCTAssertEqual(b.deMensaje, "m1")
        XCTAssertEqual(b.chat, "c1")
    }

    func testSinLoDelNucleoSaleElTexto() {
        let b = HacerLeccionIPad.borrador(chat: "c", mensaje: "m", de: nil, texto: "Primera linea\nlo demas")
        XCTAssertEqual(b.titulo, "Primera linea")
        XCTAssertEqual(b.quePaso, "Primera linea\nlo demas")
        let corto = HacerLeccionIPad.borrador(chat: "c", mensaje: "m", de: nil, texto: "solo esto")
        XCTAssertEqual(corto.quePaso, "", "una linea sola es el titulo")
    }
}
