// El horario de clases semanal y la llamada en el iPad: la rejilla (dias y
// franjas de 15 min), la semana del nucleo editada desde el modelo (con
// deshacer), lo puro de la llamada (volver a llamar, colgar sola) y el
// timbre que se hace aqui para la notificacion.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

@MainActor
final class RecordatoriosHorarioTests: XCTestCase {
    // MARK: La rejilla

    func testLaRejillaRedondeaALaFranjaDe15MinYNoSeSale() {
        let r = ReglasHorario(cuantosDias: 6, anchoDia: 100, altoHora: 60)
        XCTAssertEqual(r.altoTotal, 15 * 60)
        XCTAssertEqual(r.y(8 * 60), 60)
        // 8:29 cae en la franja de las 8:15.
        XCTAssertEqual(r.minuto(enY: 60 + 29), 8 * 60 + 15)
        XCTAssertEqual(r.minutoCercano(enY: 60 + 29), 8 * 60 + 30)
        // Fuera de la rejilla: dentro.
        XCTAssertEqual(r.minuto(enY: -50), ReglasHorario.desde)
        XCTAssertEqual(r.minuto(enY: 99_999), ReglasHorario.hasta - ReglasHorario.paso)
        XCTAssertEqual(r.dia(enX: 250), 2)
        XCTAssertEqual(r.dia(enX: 9_999), 5, "sin domingo, el sabado es el ultimo")
        XCTAssertEqual(r.dia(enX: -3), 0)
        XCTAssertEqual(ReglasHorario.tramo(8 * 60, 10 * 60 + 15), "8:00–10:15")
    }

    func testHoyEsElDiaDelHorario() throws {
        var c = Calendar(identifier: .gregorian)
        c.timeZone = try XCTUnwrap(TimeZone(identifier: "Europe/Madrid"))
        // El 26 de octubre de 2026 es lunes.
        let lunes = try XCTUnwrap(c.date(from: DateComponents(year: 2026, month: 10, day: 26, hour: 9, minute: 40)))
        XCTAssertEqual(ReglasHorario.hoy(lunes, calendario: c), 0)
        XCTAssertEqual(ReglasHorario.hoy(lunes.addingTimeInterval(6 * 86400), calendario: c), 6, "domingo")
        XCTAssertEqual(ReglasHorario.ahora(lunes, calendario: c), 9 * 60 + 40)
    }

    // MARK: La semana del nucleo

    func testLaSemanaSeEditaYSeDeshace() throws {
        let (almacen, raiz) = try almacenTemporalOrganizar("horario")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let m = ModeloSemana(nucleo: almacen.nucleo)
        XCTAssertNil(m.error)
        m.cambiar { _ = $0.anadirAsignatura(escrito: "Geometría @A-12") }
        XCTAssertEqual(m.asignaturas.map(\.nombre), ["Geometría"])
        XCTAssertEqual(m.asignaturas.first?.aula, "A-12")
        // Soltarla el martes a las 8:10: una clase de 1 h desde las 8:00.
        m.cambiar { $0.soltar(asignatura: 0, dia: 1, minuto: 8 * 60 + 10) }
        let b = try XCTUnwrap(m.bloques.first)
        XCTAssertEqual([b.dia, UInt8(b.inicio / 60)], [1, 8])
        XCTAssertEqual(b.fin - b.inicio, 60)
        // Alargarla: en saltos de 15 min.
        m.cambiar { $0.alargar(i: 0, fin: 9 * 60 + 50) }
        XCTAssertEqual(m.bloques.first?.fin, 9 * 60 + 45)
        // Se escribio en horario.md (lo que lee el vigia).
        XCTAssertTrue(m.texto()?.contains("Geometría") == true)
        // Borrar y deshacer.
        m.cambiar(deshacible: "Clase borrada: Geometría") { $0.borrar(i: 0) }
        XCTAssertTrue(m.bloques.isEmpty)
        XCTAssertEqual(m.deshacer, "Clase borrada: Geometría")
        m.hacerDeshacer()
        XCTAssertEqual(m.bloques.count, 1)
        XCTAssertNil(m.deshacer)
        // Caso negativo: repetir la asignatura no la duplica.
        m.cambiar { XCTAssertFalse($0.anadirAsignatura(escrito: "Geometría")) }
        XCTAssertEqual(m.asignaturas.count, 1)
    }

    // MARK: La llamada

    func testVolverALlamarYColgarSola() {
        XCTAssertEqual(Llamada.volverEn.map(Llamada.rotulo(deVolver:)), ["5 min", "15 min", "30 min", "1 h", "2 h"])
        let t0 = Date(timeIntervalSince1970: 1000)
        XCTAssertEqual(Llamada.horaDeVolver(t0, minutos: 15), t0.addingTimeInterval(900))
        XCTAssertFalse(Llamada.tocaColgar(.sonando, desde: t0, ahora: t0.addingTimeInterval(44.9), acaboEn: nil))
        XCTAssertTrue(Llamada.tocaColgar(.sonando, desde: t0, ahora: t0.addingTimeInterval(45), acaboEn: nil))
        XCTAssertFalse(Llamada.tocaColgar(.contestada, desde: t0, ahora: t0.addingTimeInterval(500), acaboEn: nil),
                       "mientras suena la nota no cuelga")
        XCTAssertTrue(Llamada.tocaColgar(.contestada, desde: t0, ahora: t0.addingTimeInterval(11), acaboEn: t0.addingTimeInterval(10)))
        XCTAssertEqual(Llamada.titulo("  "), "Llamada")
        XCTAssertTrue(Llamada.aunSuena(t0, ahora: t0.addingTimeInterval(60)))
        XCTAssertFalse(Llamada.aunSuena(t0, ahora: t0.addingTimeInterval(11 * 60)), "eso ya es una llamada perdida")
        XCTAssertTrue(Llamada.esDelHorario("horario:3:2026-10-26|123"))
        XCTAssertFalse(Llamada.esDelHorario("m1"))
    }

    func testElAvisoDeUnaLlamadaLlevaElTimbreYElNombre() {
        var r = RecordatorioPuesto(chat: Recordatorios.chatHorario, id: "horario:1", cuando: Date(timeIntervalSince1970: 50),
                                   texto: "Física")
        r.llamada = true
        let a = AvisosRecordatorio.aviso(de: r)
        XCTAssertEqual(a.categoria, .llamada)
        XCTAssertEqual(a.sonido, TonoLlamada.nombre)
        XCTAssertEqual(a.titulo, "Llamada entrante")
        XCTAssertEqual(a.texto, "Física")
        XCTAssertEqual(a.datos[Llamada.claveNombre], "Física")
        XCTAssertEqual(CategoriaAviso.llamada.acciones, [.posponer])
        XCTAssertEqual(CategoriaAviso.llamada.alPulsar, .abrir)
        // Un recordatorio de siempre sigue igual.
        let normal = AvisosRecordatorio.aviso(de: RecordatorioPuesto(chat: "c", id: "m", cuando: Date(), texto: "x"))
        XCTAssertEqual(normal.categoria, .recordatorio)
        XCTAssertNil(normal.sonido)
    }

    func testElTimbreEsUnWavDeMenosDe30Segundos() {
        XCTAssertEqual(TonoLlamada.envolvente(0.2), 1)
        XCTAssertEqual(TonoLlamada.envolvente(0.5), 0, "el silencio de en medio")
        XCTAssertEqual(TonoLlamada.envolvente(0.8), 1)
        XCTAssertEqual(TonoLlamada.envolvente(2.0), 0)
        XCTAssertEqual(TonoLlamada.envolvente(3.2), 1, "otra vez, cada 3 s")
        XCTAssertLessThan(TonoLlamada.envolvente(0.001), 1, "sube sin chasquido")
        let d = TonoLlamada.wav(segundos: 1, muestrasPorSegundo: 8000)
        XCTAssertEqual(d.count, 44 + 8000 * 2)
        XCTAssertEqual(String(decoding: d.prefix(4), as: UTF8.self), "RIFF")
        XCTAssertEqual(String(decoding: d[8 ..< 12], as: UTF8.self), "WAVE")
        XCTAssertLessThan(TonoLlamada.segundos, 30)
    }
}
