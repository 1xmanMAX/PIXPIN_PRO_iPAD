// Pruebas de lo puro de la tanda 3 (planos, pines, captura, sistema,
// recordatorios, pedidos): el calendario del iPad, la ficha de una clase,
// la extension de Safari, la captura larga, el GIF, la fila del
// reproductor flotante, abrir otro plano y la busqueda pedida.

import Foundation
import PixPinCore
import XCTest
@testable import PixPinKit

final class Tanda3SistemaPlanosPinesTests: XCTestCase {
    // MARK: El calendario del iPad

    private func evento(_ id: String, _ titulo: String, _ inicio: Date, lugar: String? = nil) -> EventoDelCalendario {
        EventoDelCalendario(id: id, titulo: titulo, inicio: inicio, fin: inicio.addingTimeInterval(3600),
                                          lugar: lugar)
    }

    func testLosEventosDelIPadLlamanAntesSinRepetirLosDeGoogle() {
        let ahora = Date(timeIntervalSince1970: 1_800_000_000)
        let a = evento("A", "Dentista", ahora.addingTimeInterval(2 * 3600), lugar: "Calle Mayor 3")
        let b = evento("B", "Reunión", ahora.addingTimeInterval(5 * 3600))
        let pasado = evento("C", "Ya fue", ahora.addingTimeInterval(5 * 60))
        // Google ya trae la reunion (mismo nombre, sin tildes ni mayusculas, misma hora de aviso).
        let msB = Int64((b.inicio.addingTimeInterval(-10 * 60).timeIntervalSince1970 * 1000).rounded())
        let deGoogle = AvisoHorario(id: "horario:gcal:x", cuandoUtcMs: msB, nombre: "reunion", detalle: "",
                                    texto: nil, audio: nil)
        let v = CalendarioDelIPad.avisos([b, pasado, a], antelacion: 10, ahora: ahora, evitando: [deGoogle])
        XCTAssertEqual(v.map(\.nombre), ["Dentista"], "por hora, sin lo que ya paso ni lo de Google")
        XCTAssertTrue(v[0].id.hasPrefix(CalendarioDelIPad.prefijo + "A|"))
        XCTAssertTrue(v[0].id.hasPrefix("horario:"), "el vigia lo trata como del horario")
        XCTAssertEqual(v[0].cuandoUtcMs, Int64((a.inicio.timeIntervalSince1970 - 600) * 1000))
        XCTAssertTrue(v[0].detalle.hasSuffix(" · Calle Mayor 3"), v[0].detalle)
        // Caso negativo: sin eventos, nada.
        XCTAssertTrue(CalendarioDelIPad.avisos([], antelacion: 10, ahora: ahora, evitando: []).isEmpty)
    }

    func testElDetalleDeUnEventoLlevaSuHoraYSuLugar() {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        let inicio = Date(timeIntervalSince1970: 9 * 3600 + 5 * 60)
        let e = EventoDelCalendario(id: "x", titulo: "Clase", inicio: inicio,
                                                  fin: inicio.addingTimeInterval(90 * 60), lugar: "  ")
        XCTAssertEqual(CalendarioDelIPad.detalle(e, calendario: cal), "9:05–10:35")
        var sinFin = e
        sinFin.fin = nil
        sinFin.lugar = "Aula 3"
        XCTAssertEqual(CalendarioDelIPad.detalle(sinFin, calendario: cal), "9:05 · Aula 3")
    }

    // MARK: La ficha de una clase

    func testLaFichaDeUnaClaseSeLeeComoEnWindows() {
        XCTAssertEqual(TextoFichaClase.minutos("15"), 15)
        XCTAssertEqual(TextoFichaClase.minutos(" 20 min "), 20)
        XCTAssertEqual(TextoFichaClase.minutos("99999"), 24 * 60, "como mucho un dia")
        XCTAssertNil(TextoFichaClase.minutos(""), "vacio: el del horario")
        XCTAssertNil(TextoFichaClase.minutos("pronto"))
        XCTAssertEqual(TextoFichaClase.notas("llevar compás\n\n  repasar  \n"), ["llevar compás", "repasar"])
        XCTAssertNil(TextoFichaClase.aula("   "))
        XCTAssertEqual(TextoFichaClase.aula(" A-12 "), "A-12")
        XCTAssertEqual(EdicionHorario.clase(2).id, "c2")
        XCTAssertNotEqual(EdicionHorario.clase(2), EdicionHorario.asignatura(2))
    }

    // MARK: La extension de Safari

    func testLoDelNavegadorSeVuelveUnEnlaceQueLaAppEntiende() throws {
        let e = try XCTUnwrap(PedidoDelNavegador.enlace(accion: "tarea", texto: " comprar yeso + cola & 2 ",
                                                        url: nil, titulo: nil))
        let partes = try Enlaces.partir(e)
        XCTAssertEqual(partes.verbo, "tarea")
        XCTAssertEqual(partes.campos["texto"], "comprar yeso + cola & 2", "el + no es un espacio")
        let enlace = try XCTUnwrap(PedidoDelNavegador.enlace(accion: "enlace_al_chat", texto: nil,
                                                             url: "https://a.es/x?y=1", titulo: "Una página"))
        XCTAssertEqual(try Enlaces.partir(enlace).campos["texto"], "Una página\nhttps://a.es/x?y=1")
        let pregunta = try XCTUnwrap(PedidoDelNavegador.enlace(accion: "asistente", texto: "E = mc²", url: nil,
                                                               titulo: nil, pregunta: "Explícamelo"))
        XCTAssertEqual(try Enlaces.partir(pregunta).verbo, "asistente")
        XCTAssertNoThrow(try Enlaces.leer(e))
        // Casos negativos: sin seleccion, una accion que no existe, la foto (no va por enlace).
        XCTAssertNil(PedidoDelNavegador.enlace(accion: "nota", texto: "  ", url: nil, titulo: nil))
        XCTAssertNil(PedidoDelNavegador.enlace(accion: "borrar_todo", texto: "x", url: nil, titulo: nil))
        XCTAssertNil(PedidoDelNavegador.enlace(accion: "captura_pagina", texto: nil, url: "https://a.es", titulo: nil))
        // Un texto enorme se recorta (el enlace tiene tope).
        let largo = String(repeating: "a", count: 50_000)
        let recortado = try XCTUnwrap(PedidoDelNavegador.enlace(accion: "chat", texto: largo, url: nil, titulo: nil))
        XCTAssertLessThan(recortado.utf8.count, Enlaces.tope)
    }

    func testUnaEntregaViejaSinEnlaceSeSigueLeyendo() throws {
        let vieja = #"{"version":1,"chat":"a","nombreDelChat":"Casa","comentario":"","textos":[],"ficheros":[],"creada":1}"#
        let e = try JSONDecoder().decode(EntregaCompartida.self, from: Data(vieja.utf8))
        XCTAssertNil(e.enlace)
        var nueva = e
        nueva.enlace = "pixpinmax://tarea?texto=x"
        let otra = try JSONDecoder().decode(EntregaCompartida.self, from: JSONEncoder().encode(nueva))
        XCTAssertEqual(otra.enlace, "pixpinmax://tarea?texto=x")
    }

    // MARK: La captura larga y el GIF

    func testLaCapturaLargaBajaPantallaAPantallaHastaElFin() {
        XCTAssertEqual(CapturaLargaIPad.desplazamientos(arriba: 0, fin: 250, alto: 100, tope: 10_000), [0, 100, 200, 250])
        XCTAssertEqual(CapturaLargaIPad.desplazamientos(arriba: -20, fin: -20, alto: 100, tope: 10_000), [-20],
                       "sin rollo, un trozo")
        XCTAssertEqual(CapturaLargaIPad.desplazamientos(arriba: 0, fin: 10_000, alto: 100, tope: 300), [0, 100, 200],
                       "con tope")
        XCTAssertTrue(CapturaLargaIPad.desplazamientos(arriba: 0, fin: 100, alto: 0, tope: 300).isEmpty)
        XCTAssertEqual(CapturaLargaIPad.escala(pantalla: 2, altoTotal: 4000, tope: 16_000), 2)
        XCTAssertEqual(CapturaLargaIPad.escala(pantalla: 2, altoTotal: 16_000, tope: 16_000), 1)
        XCTAssertEqual(CapturaLargaIPad.escala(pantalla: 2, altoTotal: 1_000_000, tope: 16_000), 0.5, "nunca menos de 0,5")
    }

    func testElGifSacaDiezFotogramasPorSegundoConTope() {
        XCTAssertEqual(GrabadorIPad.instantesGif(duracion: 0.35, fps: 10), [0, 0.1, 0.2])
        XCTAssertEqual(GrabadorIPad.instantesGif(duracion: 0.01, fps: 10), [0], "al menos uno")
        XCTAssertEqual(GrabadorIPad.instantesGif(duracion: 600, fps: 10, tope: 60).count, 600)
        XCTAssertTrue(GrabadorIPad.instantesGif(duracion: .nan).isEmpty)
        XCTAssertTrue(GrabadorIPad.instantesGif(duracion: 0).isEmpty)
    }

    func testLaFilaDelReproductorDiceCualSuena() {
        XCTAssertEqual(ReglasAudioPin.fila(0, de: 3, nombre: "clase.m4a"), "1 de 3 · clase.m4a")
        XCTAssertEqual(ReglasAudioPin.fila(7, de: 3, nombre: ""), "3 de 3", "no se pasa de la ultima")
        XCTAssertEqual(ReglasAudioPin.fila(-1, de: 2, nombre: "a"), "1 de 2 · a")
    }

    // MARK: Abrir otro plano y la busqueda pedida

    func testOtroPlanoSeCopiaConSuNombre() {
        let c = URL(fileURLWithPath: "/tmp/planos", isDirectory: true)
        XCTAssertEqual(OtroPlanoIPad.destino(de: "Planta baja.dwg", en: c).lastPathComponent, "Planta baja.dwg")
        XCTAssertEqual(OtroPlanoIPad.destino(de: "a/b.dxf", en: c).lastPathComponent, "a_b.dxf", "sin subir de carpeta")
        XCTAssertEqual(OtroPlanoIPad.destino(de: "  ", en: c).lastPathComponent, "plano")
    }

    @MainActor
    func testLaBusquedaPedidaSeTomaUnaVez() {
        let c = ConsultaPedida()
        c.poner("  yeso ")
        XCTAssertEqual(c.texto, "yeso")
        XCTAssertEqual(c.tomar(), "yeso")
        XCTAssertNil(c.tomar(), "una sola vez")
        c.poner("   ")
        XCTAssertNil(c.texto, "vacio: nada")
    }

    // MARK: El tema Cosmos

    func testElCieloCosmosTieneSusEstrellasSiempreEnElMismoSitio() {
        let a = CuentasCosmos.estrellas(), b = CuentasCosmos.estrellas()
        XCTAssertEqual(a.map(\.count), [160, 45, 12])
        XCTAssertEqual(a, b, "el cielo no cambia de una vez a otra")
        XCTAssertTrue(a.joined().allSatisfy { (0 ..< 1).contains($0.x) && (0 ..< 1).contains($0.y) })
        XCTAssertEqual(Tema.cosmos.esquema, .dark)
        XCTAssertEqual(Tema.cosmos.estilo, .dark)
        XCTAssertEqual(Tema(rawValue: "cosmos"), .cosmos)
    }

    // MARK: La palabra magica

    func testLaPalabraMagicaSolaSacaSuHerramienta() {
        XCTAssertEqual(PalabraMagicaPin.detectar(" Timer\n"), .miniapp("temporizador"))
        XCTAssertEqual(PalabraMagicaPin.detectar("TODO"), .miniapp("tareas"))
        XCTAssertEqual(PalabraMagicaPin.detectar("pizarra"), .lienzo)
        // Casos negativos: dentro de un texto, con espacios, o una palabra cualquiera.
        XCTAssertNil(PalabraMagicaPin.detectar("timer de la cocina"))
        XCTAssertNil(PalabraMagicaPin.detectar("nota"))
        XCTAssertNil(PalabraMagicaPin.detectar(""))
        XCTAssertNil(PalabraMagicaPin.detectar(String(repeating: "x", count: 40)))
    }
}
