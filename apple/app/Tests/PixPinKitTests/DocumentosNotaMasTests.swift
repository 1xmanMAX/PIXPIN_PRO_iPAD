import CoreGraphics
import PixPinCore
import XCTest
@testable import PixPinKit

final class DocumentosNotaMasTests: XCTestCase {
    func testUnaFotoCabeEnLaColumnaYNoPasaDeSuAlto() {
        // A lo ancho de la columna, con su proporcion.
        let a = tamanoDeFotoEnNota(natural: CGSize(width: 2000, height: 1000), columna: 600, pedido: nil)
        XCTAssertEqual(a, CGSize(width: 600, height: 300))
        // Con el ancho pedido.
        let b = tamanoDeFotoEnNota(natural: CGSize(width: 2000, height: 1000), columna: 600, pedido: 320)
        XCTAssertEqual(b, CGSize(width: 320, height: 160))
        // Una muy alta se queda en 480.
        let c = tamanoDeFotoEnNota(natural: CGSize(width: 1000, height: 4000), columna: 600, pedido: nil)
        XCTAssertEqual(c.height, 480)
        XCTAssertEqual(c.width, 120)
        // Caso negativo: sin tamano no hay hueco.
        XCTAssertEqual(tamanoDeFotoEnNota(natural: .zero, columna: 600, pedido: nil), .zero)
    }

    func testElRelojDelOrador() {
        XCTAssertEqual(relojDelOrador(segundos: 0), "0:00")
        XCTAssertEqual(relojDelOrador(segundos: 725), "12:05")
        XCTAssertEqual(relojDelOrador(segundos: 3725), "1:02:05")
        XCTAssertEqual(relojDelOrador(segundos: -3), "0:00")
    }

    func testLaTintaDeLaPaginaComoSvg() {
        XCTAssertEqual(SvgDeTintaIPad.color(ColorLienzo(r: 1, g: 0, b: 0.5, a: 1)), "#FF0080")
        XCTAssertEqual(SvgDeTintaIPad.xml("a<b & 'c'"), "a&lt;b &amp; &apos;c&apos;")
        // Una cadena de JavaScript sin romperse con comillas y saltos.
        XCTAssertEqual(SvgDeTintaIPad.cadenaJS("di \"hola\"\n"), "\"di \\\"hola\\\"\\n\"")
        let p = CGMutablePath()
        p.move(to: CGPoint(x: 1, y: 2))
        p.addLine(to: CGPoint(x: 3.5, y: 4))
        p.closeSubpath()
        XCTAssertEqual(SvgDeTintaIPad.d(p), "M1.00 2.00L3.50 4.00Z")
        // Sin tinta: el SVG vacio.
        XCTAssertTrue(SvgDeTintaIPad.svg(nil).hasSuffix("</svg>"))
    }

    func testElMenuDeLaBarraYLasTablasDelNucleo() {
        // La barra al principio del renglon abre el menu; en mitad, no.
        XCTAssertEqual(mdBarraConsulta(texto: "Hola\n/tab", cursor: 9)?.barra, 5)
        XCTAssertNil(mdBarraConsulta(texto: "12/03", cursor: 5))
        XCTAssertEqual(mdBarraComandos(escrito: "tab").first?.clave, "tabla")
        let t = mdTablas(texto: "| A | B |\n|---|---|\n| 1 | 2 |")
        XCTAssertEqual(t.count, 1)
        XCTAssertEqual(t.first?.filas, [["A", "B"], ["1", "2"]])
        XCTAssertEqual(mdTablaTexto(filas: [["A"], ["1"]], alineaciones: ["d"]), "| A |\n|---:|\n| 1 |")
    }
}
