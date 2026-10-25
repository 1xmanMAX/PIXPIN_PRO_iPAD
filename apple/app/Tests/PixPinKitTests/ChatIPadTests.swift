// Lo puro del chat del iPad: lo elegido, los separadores de dia, el nombre
// de lo soltado, el color de los archivos, las tareas y la busqueda.

import PixPinCore
import UniformTypeIdentifiers
import XCTest
@testable import PixPinKit

@MainActor
final class ChatIPadTests: XCTestCase {
    private func m(_ id: String, clase: String = "NOTA", texto: String = "", nombre: String = "",
                   cuando: Int64 = 0, emoji: String? = nil, codigo: String? = nil) -> MensajeChat {
        MensajeChat(id: id, cuando: cuando, clase: clase, texto: texto, nombre: nombre, ruta: nil, bytes: 0,
                    resumen: texto, codigoChat: codigo, respondeA: nil, emoji: emoji, fijado: false,
                    referencia: nil, pagina: nil, duracionMs: 0, transcripcion: nil, picos: [],
                    enBuzon: false, recuerdaEn: nil, miniapp: nil, esLeccion: false,
                    sePuedeRenombrar: false, sePuedeDescribir: false, punto: nil, soloLaFoto: true,
                    vieneDe: nil, vieneDeDibujo: nil, recibidoDe: nil, lienzoIa: nil)
    }

    func testLoElegidoSeAlternaYHaceTramos() {
        var e = EleccionChat()
        XCTAssertTrue(e.vacia)
        e.alternar("b")
        e.tramo(hasta: "d", en: ["a", "b", "c", "d", "e"])
        XCTAssertEqual(e.enOrden(["a", "b", "c", "d", "e"]), ["b", "c", "d"])
        e.alternar("c")
        XCTAssertEqual(e.enOrden(["a", "b", "c", "d", "e"]), ["b", "d"])
        e.quedarse(con: ["d"])
        XCTAssertEqual(e.ids, ["d"])
        e.vaciar()
        XCTAssertTrue(e.vacia)
    }

    func testElSeparadorDiceHoyAyerOLaFecha() {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "Europe/Madrid")!
        let ahora = cal.date(from: DateComponents(year: 2026, month: 10, day: 25, hour: 12))!
        let ms = { (d: Date) in Int64(d.timeIntervalSince1970 * 1000) }
        XCTAssertEqual(SeparadorDeDia.rotulo(ms(ahora), ahora: ahora, calendario: cal), "Hoy")
        XCTAssertEqual(SeparadorDeDia.rotulo(ms(ahora.addingTimeInterval(-86_400)), ahora: ahora, calendario: cal), "Ayer")
        let antes = SeparadorDeDia.rotulo(ms(ahora.addingTimeInterval(-5 * 86_400)), ahora: ahora, calendario: cal)
        XCTAssertTrue(antes.contains("20"), antes)
        XCTAssertTrue(SeparadorDeDia.cambia(nil, m("1")))
        XCTAssertFalse(SeparadorDeDia.cambia(m("1", cuando: 1000), m("2", cuando: 2000)))
    }

    func testLoSoltadoSinNombreSeLlamaPegado() {
        XCTAssertTrue(EntradaDeFicheros.nombre(sugerido: nil, tipo: .png).hasPrefix("pegado-"))
        XCTAssertTrue(EntradaDeFicheros.nombre(sugerido: nil, tipo: .png).hasSuffix(".png"))
        XCTAssertEqual(EntradaDeFicheros.nombre(sugerido: "Plano", tipo: .pdf), "Plano.pdf")
        XCTAssertEqual(EntradaDeFicheros.nombre(sugerido: "foto.jpg", tipo: .jpeg), "foto.jpg")
        XCTAssertNil(EntradaDeFicheros.tipoDeFichero([UTType.plainText.identifier]))
        XCTAssertEqual(EntradaDeFicheros.tipoDeFichero([UTType.plainText.identifier, UTType.png.identifier]), .png)
    }

    func testCadaArchivoTieneSuColorDeWindows() {
        XCTAssertEqual(CuadroExtensionChat.familia("pdf").color, 0xE5484D)
        XCTAssertEqual(CuadroExtensionChat.familia("DOCX").color, 0x2F7BF5)
        XCTAssertTrue(CuadroExtensionChat.familia("dwg").oscuro)
        XCTAssertEqual(CuadroExtensionChat.familia("").color, 0x868E96)
    }

    func testLasTareasSeCuentan() {
        let c = BurbujaTareas.cuenta("# Obra\n- [ ] muro\n- [x] techo\n* [X] suelo\ntexto")
        XCTAssertEqual(c.hechas, 2)
        XCTAssertEqual(c.total, 3)
        XCTAssertEqual(BurbujaTareas.nombre(m("1", clase: "MINIAPP", texto: "# Obra\n- [ ] a")), "Obra")
    }

    func testLoDelAsistenteSeReconocePorSuCodigo() {
        XCTAssertTrue(MenuMensaje.esDelAsistente(m("1", codigo: "3·IA")))
        XCTAssertFalse(MenuMensaje.esDelAsistente(m("1", codigo: "3·K7Q2")))
        XCTAssertFalse(MenuMensaje.esDelAsistente(m("1")))
    }

    func testCopiarJuntaLosTextosSinMiniapps() {
        let t = ChatAbierto.textoParaCopiar([m("1", texto: " uno "), m("2", clase: "MINIAPP", texto: "{}"),
                                             m("3", texto: "dos")])
        XCTAssertEqual(t, "uno\n\ndos")
    }

    func testLaTablaSeReconoce() {
        XCTAssertTrue(BurbujaTablaChat.esTabla(m("1", clase: "TABLA")))
        XCTAssertTrue(BurbujaTablaChat.esTabla(m("1", clase: "MINIAPP", texto: "{\"celdas\": []}")))
        XCTAssertFalse(BurbujaTablaChat.esTabla(m("1", clase: "MINIAPP", texto: "- [ ] a")))
    }
}
