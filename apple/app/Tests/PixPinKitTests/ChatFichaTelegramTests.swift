// La ficha del chat al estilo Telegram (Windows 99f918a, 2439c5c, 8c92ba4 y
// bd8ac36; Mac `CajaDelChatTests`): los proyectos en su primera pestana, las
// fotos de las notas ricas en «Fotos», lo mas nuevo primero, los meses, las
// negritas de lo buscado y el teclado. Y la mezcla del motor de audio de
// Windows por la que graban ahora la voz y la clase (`Voz/MezclaVoz.swift`).

import AVFoundation
import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

private func carpetaFicha(_ nombre: String) throws -> URL {
    let u = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-ficha-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
    return u
}

final class ChatFichaTelegramTests: XCTestCase {
    // MARK: Lo del nucleo

    func testLaFichaSeAbreEnLosProyectosYMarcaElDeEsteChat() throws {
        let carpeta = try carpetaFicha("proyectos")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let nucleo = try Nucleo.abrir(raiz: carpeta.path, nombreDelAparato: "iPad")
        let chat = try nucleo.crearChat(nombre: "Casa Lima").id
        let otro = try nucleo.crearChat(nombre: "Puente Sur").id
        _ = try nucleo.escribirNota(chat: chat, texto: "hola", respondeA: nil)
        let f = try nucleo.fichaDelChat(chat: chat, actual: nil)
        XCTAssertEqual(f.primera, .proyectos)
        XCTAssertEqual(f.pestanas.first?.seccion, .proyectos)
        let p = try nucleo.proyectosDeLaFicha(chat: chat, buscado: "")
        XCTAssertTrue(p.first { $0.ficha.id == chat }?.este == true)
        XCTAssertFalse(p.first { $0.ficha.id == otro }?.este ?? true)
        // Lo buscado filtra por el nombre, sin mirar mayusculas.
        let buscados = try nucleo.proyectosDeLaFicha(chat: chat, buscado: "puente")
        XCTAssertEqual(buscados.map(\.ficha.id), [otro])
        // Caso negativo: los proyectos no son mensajes.
        XCTAssertTrue(try nucleo.cosasDeLaFicha(chat: chat, seccion: .proyectos, buscado: "").isEmpty)
    }

    func testLasFotosDeUnaNotaRicaSalenEnFotosConSuNombre() throws {
        let carpeta = try carpetaFicha("notas")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let nucleo = try Nucleo.abrir(raiz: carpeta.path, nombreDelAparato: "iPad")
        let chat = try nucleo.crearChat(nombre: "Obra").id
        _ = try nucleo.escribirNota(chat: chat, texto: "# Obra\n\n![planta|300](pixpin:files/a.png)\n\n![](pixpin:files/b.png)",
                                    respondeA: nil)
        let fotos = try nucleo.cosasDeLaFicha(chat: chat, seccion: .fotos, buscado: "")
        XCTAssertEqual(fotos.map(\.foto), [0, 1])
        XCTAssertEqual(fotos.first?.alt, "planta", "sin el ancho del editor")
        XCTAssertEqual(TextosFicha.nombre(fotos[0]), "planta")
        // Caso negativo: la foto no esta en el aparato y no es un enlace.
        XCTAssertNil(fotos.first?.rutaFoto)
        XCTAssertNil(fotos.first?.enlace)
        XCTAssertNotEqual(ModeloFichaDelChat.clave(fotos[0]), ModeloFichaDelChat.clave(fotos[1]))
    }

    func testLoMasNuevoVaPrimero() throws {
        let carpeta = try carpetaFicha("orden")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let nucleo = try Nucleo.abrir(raiz: carpeta.path, nombreDelAparato: "iPad")
        let chat = try nucleo.crearChat(nombre: "Orden").id
        _ = try nucleo.escribirNota(chat: chat, texto: "primera", respondeA: nil)
        _ = try nucleo.escribirNota(chat: chat, texto: "segunda", respondeA: nil)
        let todo = try nucleo.cosasDeLaFicha(chat: chat, seccion: .todo, buscado: "")
        XCTAssertEqual(todo.map(\.mensaje.texto), ["segunda", "primera"])
    }

    // MARK: Lo de Swift

    func testLosTextosYLosMeses() {
        XCTAssertEqual(TextosFicha.rotulo(.proyectos), "Proyectos")
        XCTAssertEqual(TextosFicha.vacia(.proyectos), "Aún no hay proyectos")
        XCTAssertEqual(TextosFicha.hojas(1), "1 hoja")
        XCTAssertEqual(TextosFicha.hojas(12), "12 hojas")
        XCTAssertNil(TextosFicha.clase("NOTA"), "una nota es solo su texto")
        XCTAssertEqual(TextosFicha.clase("CROQUIS"), "CROQUIS")
        var c = DateComponents()
        c.year = 2026
        c.month = 10
        c.day = 25
        c.hour = 12
        let octubre = Int64(Calendar.current.date(from: c)!.timeIntervalSince1970 * 1000)
        c.month = 11
        let noviembre = Int64(Calendar.current.date(from: c)!.timeIntervalSince1970 * 1000)
        XCTAssertEqual(TextosFicha.rotuloDelMes(octubre), "Octubre de 2026")
        XCTAssertEqual(TextosFicha.claveDelMes(noviembre) - TextosFicha.claveDelMes(octubre), 1)
        // Lo mas nuevo arriba: noviembre, noviembre, octubre son dos meses.
        let meses = ModeloFichaDelChat.agrupar(cuando: [noviembre, noviembre, octubre])
        XCTAssertEqual(meses.map(\.desde), [0, 2])
        XCTAssertEqual(meses.map(\.hasta), [2, 3])
        XCTAssertTrue(ModeloFichaDelChat.agrupar(cuando: []).isEmpty)
        // Lo buscado, en negrita; sin nada buscado, nada que marcar.
        XCTAssertEqual(negritasDeLaFicha(texto: "Casa Lima", buscado: "lima").first?.inicio, 5)
        XCTAssertTrue(negritasDeLaFicha(texto: "Casa Lima", buscado: "").isEmpty)
        XCTAssertEqual(duracionEnLaFicha(ms: 7_000), "0:07")
    }

    @MainActor
    func testElTecladoCambiaDePestanaYEscCierra() throws {
        let carpeta = try carpetaFicha("teclado")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let almacen = try Almacen(raiz: carpeta)
        let m = ModeloFichaDelChat(chat: "nada", almacen: almacen)
        var cerrada = false
        m.alCerrar = { cerrada = true }
        // Sin pestanas leidas, ← → no van a ningun sitio, pero son suyas.
        XCTAssertTrue(m.tecla(.derecha))
        XCTAssertFalse(cerrada)
        // Con la lupa, ← → son de su caja y Esc la apaga sin cerrar.
        m.buscando = true
        XCTAssertFalse(m.tecla(.izquierda))
        XCTAssertTrue(m.tecla(.escape))
        XCTAssertFalse(m.buscando)
        XCTAssertFalse(cerrada)
        // Sin foco ni previa, Esc cierra.
        XCTAssertTrue(m.tecla(.escape))
        XCTAssertTrue(cerrada)
    }

    // MARK: La mezcla del motor de audio

    func testLaMezclaDelNucleoVaAlRelojYNoSaleAntesDeEmpezar() throws {
        let m = try MezclaDeAudio.nueva(fuentes: [FuenteDeMezcla(papel: .microfono, muestreo: 44_100, canales: 2)],
                                        muestreo: 48_000)
        XCTAssertEqual(m.empujar(fuente: 0, muestras: [Float](repeating: 0.25, count: 882 * 2), canales: 2,
                                 separadas: true), 882)
        let primero = m.mezclar()
        XCTAssertEqual(primero.posicion, 0)
        XCTAssertTrue(primero.muestras.isEmpty)
        XCTAssertEqual(m.llevadoMs(), 0)
        // Caso negativo: una fuente que no existe no entra.
        XCTAssertEqual(m.empujar(fuente: 7, muestras: [0, 0], canales: 1, separadas: false), 0)
        XCTAssertThrowsError(try MezclaDeAudio.nueva(fuentes: [], muestreo: 48_000))
        // En pausa el medidor no se mueve.
        m.pausar(pausa: true)
        XCTAssertEqual(m.nivel(), 0)
    }

    func testUnBloqueMezcladoSeEscribeComoMonoDeLaNota() throws {
        let formato = try XCTUnwrap(AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: MezclaVoz.muestreo,
                                                  channels: 1, interleaved: false))
        let b = try XCTUnwrap(MezclaVoz.bloque([0.5, -0.25, 0.125], formato: formato))
        XCTAssertEqual(b.frameLength, 3)
        XCTAssertEqual(b.floatChannelData?[0][1], -0.25)
        XCTAssertNil(MezclaVoz.bloque([], formato: formato))
    }
}
