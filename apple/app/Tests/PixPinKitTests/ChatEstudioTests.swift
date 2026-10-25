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

private func ficha(_ id: String, _ nombre: String, grupo: String, letras: [String] = []) -> FichaDeFigura {
    FichaDeFigura(id: id, nombre: nombre, grupo: grupo, deSerie: id.hasPrefix("serie-"), ancho: 100, alto: 50,
                  piezas: PiezasDeFigura(total: 3, trazos: 0, lineas: 1, formas: 2, textos: 0, otras: 0),
                  letras: letras)
}

final class ChatEstudioTests: XCTestCase {
    func testUnaCarpetaSeReconoceYLeeSusPaginas() {
        let doc = #"{"formato":1,"paginas":[{"id":"a","titulo":"Día 1"},{"id":"b"}]}"#
        let c = mensajeEstudio("1", clase: "MINIAPP", texto: doc, nombre: "Diario", miniapp: "carpeta")
        XCTAssertTrue(CarpetasChat.esCarpeta(c))
        XCTAssertEqual(CarpetasChat.paginas(c).map(\.id), ["a", "b"])
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
    }

    func testElNombreDeUnaHojaNoEsElDeSuFichero() {
        XCTAssertEqual(CarpetasChat.nombreDeHoja(mensajeEstudio("1", clase: "DIBUJO", nombre: "abc.excalidraw")), "")
        XCTAssertEqual(CarpetasChat.nombreDeHoja(mensajeEstudio("1", clase: "DIBUJO", nombre: "Lienzo")), "")
        XCTAssertEqual(CarpetasChat.nombreDeHoja(mensajeEstudio("1", clase: "IMAGEN", nombre: "Pizarra.jpg")), "Pizarra")
        XCTAssertEqual(CarpetasChat.nombreDeHoja(mensajeEstudio("1", clase: "NOTA", texto: "# Plan\nlo demás")), "Plan")
        let p = PaginaDeCarpeta(id: "a", titulo: "")
        XCTAssertEqual(CarpetasChat.titulo(p, mensajeEstudio("a", clase: "DIBUJO", nombre: "Planta")), "Planta")
        XCTAssertEqual(CarpetasChat.titulo(PaginaDeCarpeta(id: "a", titulo: "Día 2"), nil), "Día 2")
        XCTAssertEqual(carpetaSiguienteTitulo(titulo: "Día 2"), "Día 3")
    }

    @MainActor
    func testAnkiSoloSeOfreceConTarjetas() {
        XCTAssertTrue(AnkiChat.sePuede(mensajeEstudio("1", clase: "NOTA", texto: "Capital de Perú :: Lima")))
        XCTAssertTrue(AnkiChat.sePuede(mensajeEstudio("1", clase: "NOTA", texto: "P: ¿2+2?\nR: 4")))
        // Casos negativos: un parrafo suelto, una voz.
        XCTAssertFalse(AnkiChat.sePuede(mensajeEstudio("1", clase: "NOTA", texto: "Hola: qué tal")))
        XCTAssertFalse(AnkiChat.sePuede(mensajeEstudio("1", clase: "VOZ", texto: "a :: b")))
        XCTAssertEqual(AnkiChat.plano("x &amp; y<br><b>z</b>"), "x & y z")
        let t = ankiTarjetasDeLaNota(texto: "a :: b", proyecto: "Obra")
        let f = ankiFichero(mazo: ankiMazoDe(proyecto: "Obra"), tarjetas: t)
        XCTAssertTrue(f.contains("#deck:PixPin::Obra"))
    }

    func testLasFigurasSeAgrupanYSeBuscanSinTildes() {
        let cats = figurasCategorias()
        let figuras = [
            ficha("serie-1", "Gratitud", grupo: "Diario", letras: ["Hoy agradezco"]),
            ficha("u1", "Flecha", grupo: ""),
            ficha("serie-2", "Semanal", grupo: "Planificación"),
            ficha("u2", "Mía", grupo: "Inventada"),
        ]
        let grupos = FigurasChat.porGrupos(figuras, categorias: cats).map(\.grupo)
        XCTAssertEqual(grupos, ["", "Planificación", "Diario", "Inventada"], "lo del usuario primero y las de serie en su orden")
        XCTAssertTrue(FigurasChat.casa(figuras[0], "AGRADEZCO"))
        XCTAssertTrue(FigurasChat.casa(figuras[2], "planificacion"))
        XCTAssertFalse(FigurasChat.casa(figuras[1], "círculo"))
        XCTAssertEqual(FigurasChat.rotulo(""), "Mis figuras")
        XCTAssertEqual(FigurasChat.resumen(figuras[0].piezas), "1 líneas · 2 formas")
        XCTAssertEqual(FigurasChat.nombreParaGuardar(mensajeEstudio("1", clase: "DIBUJO", nombre: "x.excalidraw"), chat: "Obra"),
                       "Figura de Obra")
    }
}
