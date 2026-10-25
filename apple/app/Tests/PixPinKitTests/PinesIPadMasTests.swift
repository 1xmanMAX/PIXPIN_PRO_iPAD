// Pruebas de lo nuevo de los pines del iPad: las guias azules y el iman
// (las de `guias.rs` de Windows), las paginas de un pin de PDF y la barra
// del pin de audio.

import CoreGraphics
import Foundation
@testable import PixPinKit
import XCTest

final class PinesIPadMasTests: XCTestCase {
    // MARK: Guias

    func testUnBordeCercaSePegaAlMismoBorde() {
        let otro = CGRect(x: 100, y: 100, width: 200, height: 150)
        // La izquierda a 6 puntos de la del otro, y lejos en vertical.
        let movido = CGRect(x: 106, y: 400, width: 120, height: 80)
        let a = ReglasGuias.alinear(movido, otros: [otro])
        XCTAssertEqual(a.rect.minX, 100)
        XCTAssertEqual(a.rect.minY, 400, "En y no hay nada a tiro")
        XCTAssertEqual(a.guias.count, 1)
        XCTAssertTrue(a.guias[0].vertical)
        XCTAssertEqual(a.guias[0].pos, 100)
        // La linea cubre a los dos pines.
        XCTAssertEqual(a.guias[0].desde, 100)
        XCTAssertEqual(a.guias[0].hasta, 480)
    }

    func testLejosNoHayGuias() {
        let otro = CGRect(x: 0, y: 0, width: 100, height: 100)
        let movido = CGRect(x: 300, y: 300, width: 50, height: 50)
        let a = ReglasGuias.alinear(movido, otros: [otro])
        XCTAssertEqual(a.rect, movido)
        XCTAssertTrue(a.guias.isEmpty)
    }

    func testSeOfreceLaSeparacionDe16() {
        let otro = CGRect(x: 0, y: 0, width: 100, height: 100)
        // A la derecha del otro, a 20 puntos: se pega a 16.
        let movido = CGRect(x: 120, y: 0, width: 50, height: 50)
        let a = ReglasGuias.alinear(movido, otros: [otro])
        XCTAssertEqual(a.rect.minX, 100 + ReglasGuias.separacion)
        XCTAssertEqual(a.rect.minY, 0)
    }

    func testGanaElQueMenosMueve() {
        let lejos = CGRect(x: 108, y: 0, width: 10, height: 10)
        let cerca = CGRect(x: 102, y: 500, width: 10, height: 10)
        let movido = CGRect(x: 100, y: 250, width: 40, height: 40)
        let a = ReglasGuias.alinear(movido, otros: [lejos, cerca])
        XCTAssertEqual(a.rect.minX, 102)
    }

    func testElMarcoDeUnaGuia() {
        let v = GuiaPin(vertical: true, pos: 50, desde: 10, hasta: 110)
        XCTAssertEqual(ReglasGuias.marco(de: v, grosor: 2), CGRect(x: 49, y: 10, width: 2, height: 100))
        let h = GuiaPin(vertical: false, pos: 20, desde: 0, hasta: 0)
        XCTAssertEqual(ReglasGuias.marco(de: h, grosor: 2), CGRect(x: 0, y: 19, width: 1, height: 2))
        XCTAssertEqual(ReglasGuias.marco(centro: CGPoint(x: 50, y: 50), tamano: CGSize(width: 20, height: 10)),
                       CGRect(x: 40, y: 45, width: 20, height: 10))
    }

    // MARK: PDF

    func testLasPaginasDeUnPinDePdfNoSeSalen() {
        var p = PaginasPdfPin(actual: 0, total: 3)
        XCTAssertEqual(p.rotulo, "1 / 3")
        XCTAssertFalse(p.hayAnterior)
        XCTAssertFalse(p.pasar(-1))
        XCTAssertTrue(p.pasar(1))
        XCTAssertEqual(p.rotulo, "2 / 3")
        XCTAssertTrue(p.pasar(5))
        XCTAssertEqual(p.actual, 2)
        XCTAssertFalse(p.haySiguiente)
        XCTAssertTrue(p.ir(a: 0))
        XCTAssertEqual(p.actual, 0)
        // Una guardada que ya no existe (el PDF encogio) se queda en la ultima.
        XCTAssertEqual(PaginasPdfPin(actual: 40, total: 5).actual, 4)
        XCTAssertEqual(PaginasPdfPin(actual: -2, total: 0).total, 1)
    }

    // MARK: Audio

    func testLaBarraDelPinDeAudio() {
        XCTAssertEqual(ReglasAudioPin.reloj(0), "0:00")
        XCTAssertEqual(ReglasAudioPin.reloj(65.9), "1:05")
        XCTAssertEqual(ReglasAudioPin.reloj(3725), "1:02:05")
        XCTAssertEqual(ReglasAudioPin.reloj(.nan), "0:00")
        XCTAssertEqual(ReglasAudioPin.saltar(5, -15, duracion: 100), 0)
        XCTAssertEqual(ReglasAudioPin.saltar(95, 15, duracion: 100), 100)
        XCTAssertEqual(ReglasAudioPin.saltar(95, 15, duracion: 0), 110)
        XCTAssertEqual(ReglasAudioPin.rotulo(1), "1×")
        XCTAssertEqual(ReglasAudioPin.rotulo(1.5), "1,5×")
    }
}
