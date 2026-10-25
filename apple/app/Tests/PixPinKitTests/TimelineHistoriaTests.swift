// La historia de un momento en el iPad: el degradado estable de un momento
// sin foto (el mismo que `pixpin_timeline::estilo::degradado_de`), la
// letra del titulo y el nombre del PNG que se comparte.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

final class TimelineHistoriaTests: XCTestCase {
    func testElDegradadoSaleDelIdComoEnWindows() {
        // FNV-1a de los bytes, modulo 8 (calculado aparte).
        XCTAssertTrue(EstiloHistoria.degradado(id: "") == (0x7A1F3D, 0xE76F51))
        XCTAssertTrue(EstiloHistoria.degradado(id: "m1") == (0x2B2D6E, 0x6A4BC4))
        XCTAssertTrue(EstiloHistoria.degradado(id: "tl-1-1") == EstiloHistoria.degradado(id: "tl-1-1"), "siempre el mismo")
        let distintos = Set((0 ..< 200).map { EstiloHistoria.degradado(id: "tl-\($0)-1").0 })
        XCTAssertGreaterThan(distintos.count, 4, "se reparten")
    }

    func testLaTarjetaEsUnPostYElTituloLargoEncoge() {
        XCTAssertEqual(EstiloHistoria.tarjeta, CGSize(width: 1080, height: 1350))
        XCTAssertEqual(EstiloHistoria.letraDelTitulo("Corto", base: 30), 30)
        XCTAssertLessThan(EstiloHistoria.letraDelTitulo(String(repeating: "a", count: 120), base: 30), 30)
    }

    func testElNombreDelPngNoLlevaBarras() {
        let m = MomentoTimeline(id: "x", cuando: 0, hora: "10:00", titulo: "Obra/planta 2: hecho", descripcion: "",
                                fotos: [], audio: nil, duracionMs: 0, enMomentos: false, leccion: nil)
        XCTAssertEqual(EstiloHistoria.nombreDeImagen(m), "Obra-planta 2- hecho.png")
    }
}
