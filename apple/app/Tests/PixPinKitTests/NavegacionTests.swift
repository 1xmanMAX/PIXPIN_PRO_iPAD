import XCTest
@testable import PixPinKit

final class NavegacionTests: XCTestCase {
    func testCadaFicheroVaASuVisor() {
        XCTAssertEqual(TipoDeFichero(extension: "DWG"), .plano)
        XCTAssertEqual(TipoDeFichero(extension: "ifc"), .modelo3d)
        XCTAssertEqual(TipoDeFichero(extension: "pdf"), .documento)
        XCTAssertEqual(TipoDeFichero(extension: "xlsx"), .documento)
        XCTAssertEqual(TipoDeFichero(extension: "m4a"), .medio)
        XCTAssertEqual(TipoDeFichero(extension: "heic"), .imagen)
        XCTAssertEqual(TipoDeFichero(extension: "zip"), .otro)
    }

    func testCadaSeccionTieneTituloEIcono() {
        for s in Seccion.allCases {
            XCTAssertFalse(s.titulo.isEmpty)
            XCTAssertFalse(s.icono.isEmpty)
        }
    }
}
