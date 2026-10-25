import PixPinCore
import XCTest
@testable import PixPinKit

final class DocumentosTests: XCTestCase {
    func testCadaDocumentoVaASuVisor() {
        func visor(_ nombre: String, chat: Bool = true) -> VisorDeDocumento {
            VisorDeDocumento.para(URL(fileURLWithPath: "/tmp/" + nombre), deUnChat: chat)
        }
        XCTAssertEqual(visor("Tema 3.PDF"), .lector)
        XCTAssertEqual(visor("informe.docx"), .reflujo)
        XCTAssertEqual(visor("plantilla.docm"), .reflujo)
        XCTAssertEqual(visor("clase.pptx"), .lector)
        XCTAssertEqual(visor("cuentas.xlsx"), .hoja)
        XCTAssertEqual(visor("datos.csv"), .hoja)
        // Un libro suelto (sin mensaje) se abre en la misma hoja (el
        // archivo no cambia: lo editado sale como una copia).
        XCTAssertEqual(visor("cuentas.xlsx", chat: false), .hoja)
        // El .xls antiguo no lo lee el nucleo.
        XCTAssertEqual(visor("viejo.xls"), .vistaRapida)
        XCTAssertEqual(visor("pagina.html"), .html)
        XCTAssertEqual(visor("apuntes.md"), .texto)
        XCTAssertEqual(visor("leeme.txt"), .texto)
        XCTAssertEqual(visor("antiguo.doc"), .vistaRapida)
        XCTAssertEqual(visor("libro.epub"), .reflujo)
        XCTAssertEqual(visor("datos.csv", chat: false), .tablaSuelta)
        XCTAssertEqual(visor("datos.tsv", chat: false), .tablaSuelta)
    }

    func testLetrasYReferenciasComoExcel() {
        XCTAssertEqual(letrasDeColumna(0), "A")
        XCTAssertEqual(letrasDeColumna(25), "Z")
        XCTAssertEqual(letrasDeColumna(26), "AA")
        XCTAssertEqual(letrasDeColumna(701), "ZZ")
        XCTAssertEqual(letrasDeColumna(702), "AAA")
        XCTAssertEqual(referenciaDeRango(f0: 2, c0: 1, f1: 2, c1: 1), "B3")
        // Da igual por que esquina se empiece.
        XCTAssertEqual(referenciaDeRango(f0: 3, c0: 2, f1: 0, c1: 0), "A1:C4")
    }

    func testCeldaConFormatoDeNumero() {
        let estilo = EstiloCeldaHoja(negrita: false, cursiva: false, subrayado: false, tachado: false,
                                     alineacion: nil, fondo: nil, color: nil, ajustar: false,
                                     formatoNumero: "porcentaje")
        let c = CeldaHoja(fila: 0, columna: 0, contenido: "0.25", mostrado: "0.25", numero: 0.25,
                          error: false, estilo: estilo)
        XCTAssertEqual(textoDeCelda(c, locale: Locale(identifier: "en_US")), "25.00%")
        let sin = CeldaHoja(fila: 0, columna: 0, contenido: "=1/0", mostrado: "#DIV/0!", numero: nil,
                            error: true, estilo: nil)
        XCTAssertEqual(textoDeCelda(sin), "#DIV/0!")
    }

    func testRenglonDeUnaPosicion() {
        let t = "# Titulo\nuno\n\ndos"
        XCTAssertEqual(EstiloDeNotaIPad.linea(de: 0, en: t), 0)
        XCTAssertEqual(EstiloDeNotaIPad.linea(de: 9, en: t), 1)
        XCTAssertEqual(EstiloDeNotaIPad.linea(de: (t as NSString).length, en: t), 3)
    }

    func testBotonesDeMarkdown() {
        let (negrita, sel) = EstiloDeNotaIPad.marcar("hola mundo", elegido: NSRange(location: 5, length: 5),
                                                     marca: .negrita)
        XCTAssertEqual(negrita, "hola **mundo**")
        XCTAssertEqual(sel, NSRange(location: 7, length: 5))
        let (lista, _) = EstiloDeNotaIPad.marcar("uno\ndos", elegido: NSRange(location: 5, length: 0), marca: .lista)
        XCTAssertEqual(lista, "uno\n- dos")
        // Otra vez el mismo boton quita la marca del renglon.
        let (sinLista, _) = EstiloDeNotaIPad.marcar(lista, elegido: NSRange(location: 7, length: 0), marca: .lista)
        XCTAssertEqual(sinLista, "uno\ndos")
    }

    func testTintaEnUnidadesDeLaHoja() {
        // Una hoja de 700 x 990 puntos en la vista, desde (50, 20).
        let g = GeometriaHoja(rect: CGRect(x: 50, y: 20, width: 700, height: 990))
        XCTAssertEqual(g.escala, 0.5, accuracy: 1e-9)
        let q = g.aHoja(CGPoint(x: 400, y: 520))
        XCTAssertEqual(q.x, 700, accuracy: 1e-9)
        XCTAssertEqual(q.y, 1000, accuracy: 1e-9)
        let p = g.aVista(q)
        XCTAssertEqual(p.x, 400, accuracy: 1e-9)
        XCTAssertEqual(p.y, 520, accuracy: 1e-9)
    }

    func testMarcadoresEnOrdenDeLectura() {
        var m: [MarcaDocumento] = []
        m = conMarca(m, x: 0.5, y: 3.2, emoji: "⭐", ahora: 10)
        m = conMarca(m, x: 0.5, y: 1.5, emoji: "❗", ahora: 10)
        XCTAssertEqual(m.map(\.emoji), ["❗", "⭐"])
        // Mismo instante: no se pisan las identidades.
        XCTAssertEqual(Set(m.map(\.id)).count, 2)
    }
}
