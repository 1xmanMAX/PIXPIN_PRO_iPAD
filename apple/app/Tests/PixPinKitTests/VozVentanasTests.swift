// Las ventanitas de la voz del iPad: el teleprónter, Pronunciar, la
// conversacion por turnos, grabar una clase, quien llama, los dos idiomas de
// las notas, la velocidad que se recuerda y el reproductor flotante. Las
// cuentas puras (las del Mac y Windows) y lo grabado entrando en un chat de
// verdad, cruzando el puente. Sin pantallas ni microfono.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

private func medidas(_ n: Int, _ alto: CGFloat) -> [MedidaParrafo] {
    (0..<n).map { MedidaParrafo(cima: CGFloat($0) * alto, alto: alto) }
}

private func mensaje(_ id: String, clase: String = "NOTA", texto: String = "", nombre: String = "",
                     ruta: String? = nil) -> MensajeChat {
    MensajeChat(id: id, cuando: 0, clase: clase, texto: texto, nombre: nombre, ruta: ruta, bytes: 0,
                resumen: "", codigoChat: nil, respondeA: nil, emoji: nil, fijado: false,
                referencia: nil, pagina: nil, duracionMs: 0, transcripcion: nil, picos: [],
                enBuzon: false, recuerdaEn: nil, miniapp: nil, esLeccion: false,
                sePuedeRenombrar: false, sePuedeDescribir: false, punto: nil, soloLaFoto: true,
                vieneDe: nil, vieneDeDibujo: nil, recibidoDe: nil, lienzoIa: nil)
}

private func carpetaVoz(_ nombre: String) throws -> URL {
    let u = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-voz-ventanas-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
    return u
}

final class VozVentanasTelepronterTests: XCTestCase {
    func testElTextoSeParteEnParrafosYPierdeLasMarcasDeMarkdown() {
        XCTAssertEqual(Telepronter.parrafos(de: "# Titulo\n\n- **uno** y `dos`\nsigue aqui\n\n> citado"),
                       ["Titulo", "uno y dos sigue aqui", "citado"])
        XCTAssertTrue(Telepronter.parrafos(de: "   \n\n  \n").isEmpty)
        XCTAssertTrue(Telepronter.parrafos(de: "![foto](obra.png)").isEmpty, "una imagen no se lee")
    }

    func testElDesfileBajaALaVelocidadPedidaYSeParaAlFinal() {
        var d = Desfile(parrafos: 4)
        d.medir(medidas(4, 100), altoVisible: 200)
        XCTAssertEqual(d.tope, 200)
        XCTAssertFalse(d.avanzar(1, velocidad: 50))
        XCTAssertEqual(d.y, 50)
        XCTAssertTrue(d.avanzar(10, velocidad: 50), "al llegar al final deja de bajar")
        XCTAssertEqual(d.y, 200)
    }

    func testCadaParrafoSeQuedaConElMilisegundoEnQueCruzoLaLinea() {
        var d = Desfile(parrafos: 3)
        d.medir(medidas(3, 100), altoVisible: 200)
        XCTAssertEqual(d.lineaDeLectura, 60, accuracy: 0.001)
        d.apuntar(0)
        XCTAssertEqual([d.tiempo(0), d.tiempo(1), d.tiempo(2)], [0, nil, nil])
        d.empujar(50)
        d.apuntar(5_000)
        XCTAssertEqual(d.tiempo(1), 5_000)
        XCTAssertNil(d.tiempo(2))
        // Volver a subir no le quita el minuto a lo ya leido.
        d.empujar(-50)
        d.apuntar(9_000)
        XCTAssertEqual(d.tiempo(1), 5_000)
    }

    func testElCuerpoPoneElFinalALoQueNoLlegoACruzarYRecortaLoDeMas() {
        var d = Desfile(parrafos: 2)
        d.medir(medidas(2, 100), altoVisible: 300)
        d.apuntar(2_000)
        XCTAssertEqual(d.cuerpo(["primero", "segundo"], duracionMs: 61_000), "[0:02] primero\n\n[1:01] segundo")
        var e = Desfile(parrafos: 1)
        e.medir(medidas(1, 100), altoVisible: 300)
        e.apuntar(99_000)
        XCTAssertEqual(e.cuerpo(["uno"], duracionMs: 5_000), "[0:05] uno")
    }

    func testSoloSePintaElTrozoQueSeVeYReiniciarVuelveArriba() {
        var d = Desfile(parrafos: 10)
        d.medir(medidas(10, 100), altoVisible: 250)
        XCTAssertEqual(d.visibles, 0..<3)
        d.empujar(400)
        XCTAssertEqual(d.visibles, 4..<7)
        d.apuntar(1_000)
        d.reiniciar()
        XCTAssertEqual(d.y, 0)
        XCTAssertNil(d.tiempo(0))
        XCTAssertTrue(Desfile(parrafos: 0).visibles.isEmpty)
    }

    func testLaVelocidadSeAcotaYSeDiceConPalabras() {
        XCTAssertEqual(Telepronter.acotarVelocidad(5), Telepronter.velocidadMinima)
        XCTAssertEqual(Telepronter.acotarVelocidad(500), Telepronter.velocidadMaxima)
        XCTAssertEqual(Telepronter.palabra(deVelocidad: 20), "muy lenta")
        XCTAssertEqual(Telepronter.palabra(deVelocidad: Telepronter.velocidadPorDefecto), "lenta")
        XCTAssertEqual(Telepronter.palabra(deVelocidad: 150), "muy rápida")
        XCTAssertEqual(Telepronter.acotarTamano(100), Telepronter.tamanoMaximo)
    }

    func testElMenuOfreceLaCajaLasNotasYLosTxtLoUltimoPrimero() {
        let largo = String(repeating: "a", count: 60)
        let v = FuentesTelepronter.entradas(borrador: "  hola ", mensajes: [
            mensaje("1", texto: "\n  Primera nota\notra"),
            mensaje("2", clase: "ARCHIVO", nombre: "guion.md", ruta: "archivos/guion.md"),
            mensaje("3", clase: "ARCHIVO", nombre: "obra.pdf", ruta: "archivos/obra.pdf"),
            mensaje("4", texto: largo),
            mensaje("5", texto: "   "),
        ])
        XCTAssertEqual(v.map(\.rotulo), ["Lo escrito en la caja", String(repeating: "a", count: 48) + "…",
                                         "guion.md", "Primera nota"])
        XCTAssertEqual(v[2].fuente, .adjunto(ruta: "archivos/guion.md"))
        // Caso negativo: sin caja ni nada que leer, no hay de donde.
        XCTAssertTrue(FuentesTelepronter.entradas(borrador: " ", mensajes: [mensaje("x", clase: "IMAGEN",
            nombre: "a.png", ruta: "archivos/a.png")]).isEmpty)
    }
}

final class VozVentanasPronunciarYConversacionTests: XCTestCase {
    func testLaClaseDeGuiaSaleDeLaExtensionYLoDemasNoSirve() {
        XCTAssertEqual(Pronunciar.clase(de: "a/libro.PDF"), .pdf)
        XCTAssertEqual(Pronunciar.clase(de: "foto.jpeg"), .imagen)
        XCTAssertEqual(Pronunciar.clase(de: "frases.md"), .texto)
        XCTAssertNil(Pronunciar.clase(de: "hoja.xlsx"))
        XCTAssertNil(Pronunciar.clase(de: "sin_extension"))
    }

    func testDelChatSalenNotasYAdjuntosQueSirvenLoUltimoPrimero() {
        let v = Pronunciar.guiasDelChat([
            mensaje("1", texto: "\n  Frase para practicar\notra linea"),
            mensaje("2", clase: "ARCHIVO", nombre: "lecturas.pdf", ruta: "archivos/lecturas.pdf"),
            mensaje("3", clase: "ARCHIVO", nombre: "cuentas.xlsx", ruta: "archivos/cuentas.xlsx"),
        ])
        XCTAssertEqual(v.count, 2)
        XCTAssertEqual(v[0].titulo, "📄 lecturas.pdf")
        XCTAssertEqual(v[0].fuente, .adjunto(ruta: "archivos/lecturas.pdf"))
        XCTAssertEqual(v[1].titulo, "✎ Frase para practicar")
        XCTAssertEqual(Pronunciar.idiomas.map(\.codigo), ["", "es", "en", "pt", "fr", "de", "it", "ca"])
    }

    func testSinNombreLaPersonaSeLlamaPorSuNumeroYLasTeclas() {
        XCTAssertEqual(ConversacionPorTurnos.nombre(["  Pepe ", ""], 0), "Pepe")
        XCTAssertEqual(ConversacionPorTurnos.nombre(["  Pepe ", ""], 1), "Persona 2")
        XCTAssertEqual(ConversacionPorTurnos.nombre([], 7), "Persona 8")
        XCTAssertEqual(ConversacionPorTurnos.tecla("1"), .persona(0))
        XCTAssertEqual(ConversacionPorTurnos.tecla("9"), .persona(8))
        XCTAssertEqual(ConversacionPorTurnos.tecla("0"), .nadie)
        XCTAssertNil(ConversacionPorTurnos.tecla("a"), "una letra no hace nada")
        XCTAssertNil(ConversacionPorTurnos.tecla(nil))
    }

    func testLoQueSeGrabaConNadieNoSale() {
        let t = turnosDeLaConversacion(cambios: [CambioDeTurno(ms: 0, quien: "Ana"),
                                                 CambioDeTurno(ms: 3_000, quien: ""),
                                                 CambioDeTurno(ms: 6_000, quien: "Luis")], finMs: 9_000)
        XCTAssertEqual(t, [TurnoVoz(quien: "Ana", desdeMs: 0, hastaMs: 3_000),
                           TurnoVoz(quien: "Luis", desdeMs: 6_000, hastaMs: 9_000)])
    }
}

final class VozVentanasClaseIdiomasTests: XCTestCase {
    func testLaClaseSeLlamaPorSuDiaYLasFotosLlevanSuMinuto() throws {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = try XCTUnwrap(TimeZone(identifier: "Europe/Madrid"))
        let fecha = try XCTUnwrap(cal.date(from: DateComponents(year: 2026, month: 10, day: 25, hour: 9, minute: 5)))
        XCTAssertEqual(GrabarClase.nombre(fecha, calendario: cal), "Clase 25 oct 9:05")
        XCTAssertEqual(GrabarClase.pie(ms: 754_000), "En el 12:34 de la clase")
        XCTAssertTrue(GrabarClase.resumen(fotos: 1, conTexto: true).hasPrefix("Con 1 capturas y la transcripción"))
        XCTAssertTrue(GrabarClase.resumen(fotos: 3, conTexto: false).contains("«Pasar a texto»"))
    }

    func testElSegundoIdiomaSoloSiEsOtroYNoEsTodoEnElPrimero() {
        XCTAssertEqual(DosIdiomasVoz.segundo("en", modo: "cada_uno", principal: "es-ES"), "en-US")
        XCTAssertEqual(DosIdiomasVoz.segundo("pt-BR", modo: "", principal: "es-ES"), "pt-BR")
        XCTAssertNil(DosIdiomasVoz.segundo("", modo: "cada_uno", principal: "es-ES"), "ninguno")
        XCTAssertNil(DosIdiomasVoz.segundo("es", modo: "cada_uno", principal: "es-ES"), "el mismo que el primero")
        XCTAssertNil(DosIdiomasVoz.segundo("en", modo: "todo_en_uno", principal: "es-ES"), "el iPad no traduce")
        XCTAssertNil(DosIdiomasVoz.segundo("klingon", modo: "cada_uno", principal: "es-ES"), "uno raro es ninguno")
    }

    func testConDosIdiomasGanaElMasSeguroSiDiceAlgo() {
        let es = TranscriptorVoz.Reconocido(segmentos: [SegmentoVoz(desdeMs: 0, texto: "jelou")], confianza: 0.4)
        let en = TranscriptorVoz.Reconocido(segmentos: [SegmentoVoz(desdeMs: 0, texto: "hello")], confianza: 0.9)
        let nada = TranscriptorVoz.Reconocido(segmentos: [], confianza: 0)
        XCTAssertEqual(DosIdiomasVoz.elegir(principal: es, otro: en).map(\.texto), ["hello"])
        XCTAssertEqual(DosIdiomasVoz.elegir(principal: en, otro: es).map(\.texto), ["hello"])
        XCTAssertEqual(DosIdiomasVoz.elegir(principal: es, otro: nil).map(\.texto), ["jelou"])
        XCTAssertEqual(DosIdiomasVoz.elegir(principal: es, otro: nada).map(\.texto), ["jelou"], "el otro no oyo nada")
        XCTAssertEqual(DosIdiomasVoz.elegir(principal: nada, otro: es).map(\.texto), ["jelou"])
        // Casi igual de seguros: se queda el primero.
        let casi = TranscriptorVoz.Reconocido(segmentos: [SegmentoVoz(desdeMs: 0, texto: "otro")], confianza: 0.43)
        XCTAssertEqual(DosIdiomasVoz.elegir(principal: es, otro: casi).map(\.texto), ["jelou"])
    }

    func testLaVelocidadGuardadaSoloSiEsDeLasQueSeOfrecen() throws {
        let nombre = "pixpin-pruebas-velocidad-\(UUID().uuidString)"
        let d = try XCTUnwrap(UserDefaults(suiteName: nombre))
        defer { d.removePersistentDomain(forName: nombre) }
        XCTAssertEqual(VelocidadGuardada.leer("v", validas: [1, 1.5], en: d), 1, "sin nada, la normal")
        VelocidadGuardada.guardar(1.5, clave: "v", en: d)
        XCTAssertEqual(VelocidadGuardada.leer("v", validas: [1, 1.5], en: d), 1.5)
        XCTAssertEqual(VelocidadGuardada.leer("v", validas: [1, 2], en: d), 1, "una que ya no se ofrece")
        VelocidadGuardada.guardar(1, clave: "v", en: d)
        XCTAssertNil(d.object(forKey: "v"), "la normal no se guarda")
    }

    func testElReproductorFlotaSoloSiElChatNoEstaDelante() {
        XCTAssertFalse(ReproductorFlotante.sale(hayAlgo: false, enProyectos: false, conChat: false, conVisor: true))
        XCTAssertFalse(ReproductorFlotante.sale(hayAlgo: true, enProyectos: true, conChat: true, conVisor: false),
                       "el chat ya tiene su barra")
        XCTAssertTrue(ReproductorFlotante.sale(hayAlgo: true, enProyectos: false, conChat: true, conVisor: false))
        XCTAssertTrue(ReproductorFlotante.sale(hayAlgo: true, enProyectos: true, conChat: false, conVisor: false))
        XCTAssertTrue(ReproductorFlotante.sale(hayAlgo: true, enProyectos: true, conChat: true, conVisor: true))
    }

    func testLasHerramientasDeVozDeLaHojaDeAdjuntar() {
        XCTAssertEqual(HerramientaDeVoz.allCases.map(\.rotulo), ["Conversación", "Teleprónter", "Pronunciar", "Grabar clase"])
        XCTAssertEqual(AccionAdjuntar.voz(.telepronter), AccionAdjuntar.voz(.telepronter))
        XCTAssertNotEqual(AccionAdjuntar.voz(.telepronter), AccionAdjuntar.voz(.pronunciar))
        XCTAssertEqual(QuienLlama.limpiar("  " + String(repeating: "x", count: 60) + " "), String(repeating: "x", count: 40))
    }
}

@MainActor
final class VozVentanasEnElChatTests: XCTestCase {
    private func chatDePrueba(_ etiqueta: String) async throws -> (Almacen, String, URL) {
        let raiz = try carpetaVoz(etiqueta)
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "iPad de prueba")
        await almacen.crearChat(nombre: "Obra")
        let chat = try XCTUnwrap(almacen.chats.first { $0.nombre == "Obra" }).id
        await almacen.abrir(chat: chat)
        return (almacen, chat, raiz)
    }

    private func audio(_ raiz: URL, _ nombre: String) throws -> URL {
        let u = raiz.appendingPathComponent(nombre)
        try Data("m4a".utf8).write(to: u)
        return u
    }

    func testLaLecturaDelTelepronterEntraConSuTextoYSuNombre() async throws {
        let (almacen, chat, raiz) = try await chatDePrueba("lectura")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let u = try audio(raiz, "lectura-1758351600000.m4a")
        let g = GrabacionVoz(rutaOrigen: u.path, duracionMs: 12_500, picos: [10, 20_000],
                             texto: "[0:00] primero\n\n[0:07] segundo", turnos: [])
        let hecho = await almacen.adjuntarGrabacion(g, en: chat)
        let m = try XCTUnwrap(hecho, almacen.error ?? "")
        XCTAssertEqual(m.clase, "VOZ")
        XCTAssertEqual(m.nombre, "lectura-1758351600000.m4a")
        XCTAssertEqual(m.transcripcion, "[0:00] primero\n\n[0:07] segundo")
        XCTAssertEqual(almacen.mensajes[chat]?.last?.id, m.id)
        XCTAssertFalse(FileManager.default.fileExists(atPath: u.path), "el grabado ya esta dentro del proyecto")
    }

    func testLaConversacionSePasaATextoConLosNombres() async throws {
        let (almacen, chat, raiz) = try await chatDePrueba("conversacion")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let u = try audio(raiz, "conversacion-1.m4a")
        let turnos = [TurnoVoz(quien: "Ana", desdeMs: 0, hastaMs: 4_000),
                      TurnoVoz(quien: "Luis", desdeMs: 4_000, hastaMs: 9_000)]
        let hecho = await almacen.adjuntarGrabacion(
            GrabacionVoz(rutaOrigen: u.path, duracionMs: 9_000, picos: [], texto: nil, turnos: turnos), en: chat)
        let m = try XCTUnwrap(hecho, almacen.error ?? "")
        let conTexto = await almacen.ponerTranscripcion(
            [SegmentoVoz(desdeMs: 300, texto: "buenos dias"), SegmentoVoz(desdeMs: 4_500, texto: "hola")],
            en: m.id, chat: chat)
        XCTAssertEqual(conTexto?.transcripcion, "[0:00] **Ana:** buenos dias\n\n[0:04] **Luis:** hola")
    }

    func testQuienLlamaSeApuntaYVuelveElDeLaNotaEnBlanco() async throws {
        let (almacen, chat, raiz) = try await chatDePrueba("quien")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let u = try audio(raiz, "recado.m4a")
        let hecho = await almacen.adjuntarGrabacion(
            GrabacionVoz(rutaOrigen: u.path, duracionMs: 3_000, picos: [], texto: nil, turnos: []), en: chat)
        let m = try XCTUnwrap(hecho, almacen.error ?? "")
        let nucleo = almacen.nucleo
        XCTAssertTrue(QuienLlama.esLlamada(m))
        XCTAssertFalse(QuienLlama.esLlamada(mensaje("n", texto: "a las diez")), "una nota escrita no llama")
        XCTAssertEqual(QuienLlama.actual(m, nucleo: nucleo), "recado")
        QuienLlama.poner("  Mamá ", en: m, nucleo: nucleo)
        XCTAssertEqual(QuienLlama.actual(m, nucleo: nucleo), "Mamá")
        // Se apunta en el iPad (fuera del hilo): se espera a que llegue.
        for _ in 0..<50 where nucleo.quienLlama(mensaje: m.id) == nil {
            try await Task.sleep(for: .milliseconds(20))
        }
        XCTAssertEqual(nucleo.quienLlama(mensaje: m.id), "Mamá")
        XCTAssertEqual((try nucleo.llamadaSecreta(chat: chat, mensaje: m.id))?.nombre, "Mamá")
    }
}
