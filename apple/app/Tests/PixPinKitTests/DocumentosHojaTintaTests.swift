import CoreGraphics
import PixPinCore
import XCTest
@testable import PixPinKit

final class DocumentosHojaTintaTests: XCTestCase {
    /// La rejilla de aqui (numeros de 46, columnas de 96, filas de 30 desde
    /// el 26) contra una de Excel de 64 x 20.
    private func rejilla() -> RejillaDeTintaHoja {
        RejillaDeTintaHoja(vistaX: [46, 142, 238, 334], vistaY0: 26, altoFila: 30,
                           tintaX: [0, 64, 128, 192], tintaY: [0, 20, 40], anchoTinta: 64, altoTinta: 20)
    }

    func testCadaPuntoCaeEnLaMismaFraccionDeSuCelda() {
        let r = rejilla()
        // La mitad de B2 aqui es la mitad de B2 en Excel.
        let q = r.aTinta(CGPoint(x: 142 + 48, y: 26 + 30 + 15))
        XCTAssertEqual(q.x, 96, accuracy: 0.001)
        XCTAssertEqual(q.y, 30, accuracy: 0.001)
        // Y de vuelta.
        let p = r.aVista(q)
        XCTAssertEqual(p.x, 190, accuracy: 0.001)
        XCTAssertEqual(p.y, 71, accuracy: 0.001)
    }

    func testMasAllaDeLoConocidoSeAlargaConLoQueMideCadaUna() {
        let r = rejilla()
        // La fila 9 (mas alla de las 2 que trae Excel): 9 * 20 en la tinta.
        let q = r.aTinta(CGPoint(x: 46, y: 26 + 9 * 30))
        XCTAssertEqual(q.y, 180, accuracy: 0.001)
        XCTAssertEqual(q.x, 0, accuracy: 0.001)
        // La columna 5: tres de la vista conocidas y dos de 96 mas.
        let x = r.aVista(CGPoint(x: 5 * 64, y: 0)).x
        XCTAssertEqual(x, 46 + 5 * 96, accuracy: 0.001)
        // Caso negativo: un punto en las letras de las columnas (antes de la
        // fila 0) no revienta: cae por encima de la hoja.
        XCTAssertLessThan(r.aTinta(CGPoint(x: 60, y: 10)).y, 0)
    }

    func testLaCajaDeUnGraficoVaDeCeldaACelda() {
        let r = rejilla()
        let c = r.caja(columna: 1, fila: 0, dx: 0, dy: 10, hasta: (columna: 3, fila: 2, dx: 0, dy: 0), ancho: 0, alto: 0)
        XCTAssertEqual(c.minX, 142, accuracy: 0.001)
        XCTAssertEqual(c.minY, 26 + 15, accuracy: 0.001)
        XCTAssertEqual(c.maxX, 334, accuracy: 0.001)
        XCTAssertEqual(c.maxY, 26 + 60, accuracy: 0.001)
        // Con tamano fijo en pixeles de Excel.
        let d = r.caja(columna: 0, fila: 0, dx: 0, dy: 0, hasta: nil, ancho: 64, alto: 40)
        XCTAssertEqual(d.width, 96, accuracy: 0.001)
        XCTAssertEqual(d.height, 60, accuracy: 0.001)
    }

    func testUnMarcadorDeHojaVaYVuelveASuCelda() {
        let s = sitioDeMarcaDeHoja(hoja: 2, fila: 41, columna: 3)
        XCTAssertEqual(s.x, 3)
        XCTAssertEqual(s.y, 20_000_041)
        let m = MarcaDocumento(id: 1, x: s.x, y: s.y, emoji: "⭐")
        let c = celdaDeMarcaDeHoja(m)
        XCTAssertEqual(c.hoja, 2)
        XCTAssertEqual(c.fila, 41)
        XCTAssertEqual(c.columna, 3)
        // Caso negativo: uno con numeros de mas (negativos) cae en A1 de la primera.
        let raro = celdaDeMarcaDeHoja(MarcaDocumento(id: 2, x: -4, y: -9, emoji: "⭐"))
        XCTAssertEqual(raro.hoja, 0)
        XCTAssertEqual(raro.fila, 0)
        XCTAssertEqual(raro.columna, 0)
    }

    func testLaPestanaSeEncuentraEnElLibro() {
        let hojas = ["Portada", "Gastos", "Resumen"]
        XCTAssertEqual(indiceEnElLibro(pestana: "Gastos", posicion: 0, hojasDelLibro: hojas), 1)
        XCTAssertEqual(indiceEnElLibro(pestana: "Obra · Resumen", posicion: 1, hojasDelLibro: hojas), 2)
        // No se encuentra: la misma posicion; fuera del libro, ninguna.
        XCTAssertEqual(indiceEnElLibro(pestana: "Otra", posicion: 1, hojasDelLibro: hojas), 1)
        XCTAssertNil(indiceEnElLibro(pestana: "Otra", posicion: 7, hojasDelLibro: hojas))
    }

    func testLosGraficosSeEntiendenYSusHuecosNoSalen() {
        XCTAssertEqual(ClaseGraficoHoja.de("barChart"), .columnas)
        XCTAssertEqual(ClaseGraficoHoja.de("barHChart"), .barras)
        XCTAssertEqual(ClaseGraficoHoja.de("line3DChart"), .lineas)
        XCTAssertEqual(ClaseGraficoHoja.de("doughnutChart"), .anillo)
        XCTAssertEqual(ClaseGraficoHoja.de("pieChart"), .tarta)
        XCTAssertEqual(ClaseGraficoHoja.de(""), .columnas)
        let p = puntosDeGrafico(categorias: ["Ene", "Feb"],
                                series: [(nombre: "", valores: [1, .nan, 3])])
        XCTAssertEqual(p.map(\.categoria), ["Ene", "3"])
        XCTAssertEqual(p.map(\.serie), ["Serie 1", "Serie 1"])
        XCTAssertEqual(p.map(\.valor), [1, 3])
    }
}
