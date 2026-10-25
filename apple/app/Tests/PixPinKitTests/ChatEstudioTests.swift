// Lo puro de lo de estudiar del chat (Windows, 24 a 26-oct): las carpetas de
// paginas, «Enviar a Anki» y la galeria de las figuras.

import PixPinCore
import XCTest
@testable import PixPinKit

private func mensajeEstudio(_ id: String, clase: String, texto: String = "", nombre: String = "",
                            miniapp: String? = nil, referencia: String? = nil) -> MensajeChat {
    MensajeChat(id: id, cuando: 1_761_400_000_000, clase: clase, texto: texto, nombre: nombre, ruta: nil, bytes: 0,
                resumen: texto, codigoChat: nil, respondeA: nil, emoji: nil, fijado: false,
                referencia: referencia, pagina: nil, duracionMs: 0, transcripcion: nil, picos: [],
                enBuzon: false, recuerdaEn: nil, miniapp: miniapp, esLeccion: false,
                sePuedeRenombrar: false, sePuedeDescribir: false, punto: nil, soloLaFoto: true,
                vieneDe: nil, vieneDeDibujo: nil, recibidoDe: nil, lienzoIa: nil)
}

final class ChatEstudioTests: XCTestCase {
    func testUnaCarpetaSeReconoceYLeeSusPaginas() {
        let doc = #"{"formato":1,"paginas":[{"id":"a","titulo":"Día 1"},{"id":"b"}]}"#
        let c = mensajeEstudio("1", clase: "MINIAPP", texto: doc, nombre: "Diario", miniapp: "carpeta")
        XCTAssertTrue(CarpetasChat.esCarpeta(c))
        XCTAssertTrue(CarpetasDelChat.esCarpeta(c))
        XCTAssertTrue(esCarpetaDePaginas(clase: c.clase, miniapp: c.miniapp))
        XCTAssertEqual(CarpetasChat.paginas(c), ["a", "b"])
        XCTAssertEqual(CarpetasChat.cuantas(0), "Vacía")
        XCTAssertEqual(CarpetasChat.cuantas(1), "1 página")
        XCTAssertEqual(CarpetasChat.cuantas(3), "3 páginas")
        // Casos negativos: una tabla no es carpeta; una carpeta no va en otra.
        let tabla = mensajeEstudio("2", clase: "MINIAPP", texto: doc, miniapp: "tabla")
        XCTAssertFalse(CarpetasChat.esCarpeta(tabla))
        XCTAssertTrue(CarpetasChat.paginas(tabla).isEmpty)
        XCTAssertFalse(CarpetasChat.seOfrece(c))
        XCTAssertFalse(CarpetasChat.seOfrece(mensajeEstudio("auto-figuras", clase: "ARCHIVO")))
        XCTAssertTrue(CarpetasChat.seOfrece(mensajeEstudio("3", clase: "DIBUJO", referencia: "r")))
        XCTAssertEqual(nombreDelPdfDeCarpeta(titulo: ""), "Carpeta.pdf")
    }

    func testElTituloDeUnaPaginaEsElSuyoOElDeSuFila() {
        let m = mensajeEstudio("a", clase: "NOTA", texto: "Plan", nombre: "")
        XCTAssertEqual(CarpetasChat.titulo(PaginaDeCarpeta(titulo: "Día 2", mensaje: m)), "Día 2")
        XCTAssertFalse(CarpetasChat.titulo(PaginaDeCarpeta(titulo: "  ", mensaje: m)).isEmpty)
    }

    @MainActor
    func testAnkiDiceComoFueYNombraLasImagenesComoElNucleo() {
        XCTAssertTrue(ankiHayTarjetasEnNota(texto: "Capital de Perú :: Lima"))
        XCTAssertTrue(ankiHayTarjetasEnNota(texto: "P: ¿2+2?\nR: 4"))
        // Caso negativo: un parrafo suelto no da tarjetas.
        XCTAssertFalse(ankiHayTarjetasEnNota(texto: "Hola: qué tal"))
        XCTAssertTrue(ChatAbierto.textoDeAnki(.nada).hasPrefix("Aquí no hay tarjetas"))
        XCTAssertEqual(ChatAbierto.textoDeAnki(.hecho(n: 3, mazo: "PixPin::Obra")), "3 tarjetas en Anki, mazo «PixPin::Obra»")
        XCTAssertEqual(AnkiChat.nombreDeImagen(id: "m1", url: URL(fileURLWithPath: "/x/Foto.JPG")), "pixpin-m1.jpg")
    }

    func testLaCapturaDeLaClaseEsLaUltimaHechaAntes() {
        XCTAssertNil(ClaseEnElChat.captura(deLaClaseEn: [], posicionMs: 10))
        XCTAssertEqual(ClaseEnElChat.captura(deLaClaseEn: [1_000, 5_000], posicionMs: 0), 0)
        XCTAssertEqual(ClaseEnElChat.captura(deLaClaseEn: [1_000, 5_000], posicionMs: 6_000), 1)
    }

    func testLosCamposDelIntervaloVanDeDosEnDos() {
        let c = { (k: String, ancho: Bool) in CampoDeIntervalo(clave: k, rotulo: k, pista: "", aLoAncho: ancho) }
        let filas = FigurasChat.filas([c("desde", false), c("hasta", false), c("de_a", true), c("mes", false)])
        XCTAssertEqual(filas.map { $0.map(\.clave) }, [["desde", "hasta"], ["de_a"], ["mes"]])
        XCTAssertEqual(FigurasChat.resumen(DetalleDeFigura(total: 3, trazos: 0, lineas: 1, formas: 2, textos: 0,
                                                           otras: 0, letras: [])), "1 líneas · 2 formas")
        XCTAssertEqual(FigurasChat.resumen(DetalleDeFigura(total: 4, trazos: 0, lineas: 0, formas: 0, textos: 0,
                                                           otras: 0, letras: [])), "4")
    }
}
