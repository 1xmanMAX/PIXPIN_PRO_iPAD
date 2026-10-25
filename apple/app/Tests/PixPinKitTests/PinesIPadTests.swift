// Pruebas de la capa de pines del iPad: las cuentas de la capa (donde nace,
// que no se salga, el giro con dos dedos, el paso al almacen y de vuelta),
// el panel «Pines abiertos» y que un pin guardado vuelve igual.

import CoreGraphics
import Foundation
@testable import PixPinKit
import XCTest

final class PinesIPadTests: XCTestCase {
    // MARK: ReglasCapa

    func testUnPinNoSeVaDeLaVentana() {
        let area = CGSize(width: 800, height: 600)
        let tam = CGSize(width: 200, height: 100)
        // Muy a la derecha y abajo: quedan 44 puntos a la vista.
        let c = ReglasCapa.centroDentro(CGPoint(x: 5000, y: 5000), tamano: tam, area: area)
        XCTAssertEqual(c.x - tam.width / 2, area.width - ReglasCapa.aLaVista, accuracy: 0.001)
        XCTAssertEqual(c.y - tam.height / 2, area.height - ReglasCapa.aLaVista, accuracy: 0.001)
        // Por arriba no se sale nada (ahi se agarran las herramientas).
        let arriba = ReglasCapa.centroDentro(CGPoint(x: 400, y: -300), tamano: tam, area: area)
        XCTAssertEqual(arriba.y, tam.height / 2, accuracy: 0.001)
        // Por la izquierda, como por la derecha.
        let izq = ReglasCapa.centroDentro(CGPoint(x: -900, y: 300), tamano: tam, area: area)
        XCTAssertEqual(izq.x + tam.width / 2, ReglasCapa.aLaVista, accuracy: 0.001)
        // Uno que ya esta dentro no se mueve.
        let dentro = CGPoint(x: 300, y: 300)
        XCTAssertEqual(ReglasCapa.centroDentro(dentro, tamano: tam, area: area), dentro)
    }

    func testLosSeguidosNacenEscalonados() {
        let area = CGSize(width: 1000, height: 800)
        let a = ReglasCapa.centroDeNacer(area: area, n: 0)
        let b = ReglasCapa.centroDeNacer(area: area, n: 1)
        XCTAssertEqual(a, CGPoint(x: 500, y: 400))
        XCTAssertEqual(b.x - a.x, 24)
        XCTAssertEqual(ReglasCapa.centroDeNacer(area: area, n: 8), a, "da la vuelta cada ocho")
    }

    func testElGiroConDosDedosSePegaAlCuarto() {
        XCTAssertEqual(ReglasCapa.cuartosDeGiro(grados: 10), 0, "pellizcar no gira sin querer")
        XCTAssertEqual(ReglasCapa.cuartosDeGiro(grados: 80), 1)
        XCTAssertEqual(ReglasCapa.cuartosDeGiro(grados: -95), -1)
        XCTAssertEqual(ReglasCapa.cuartosDeGiro(grados: 170), 2)
        XCTAssertEqual(ReglasCapa.cuartosDeGiro(grados: .nan), 0)
    }

    func testDeLaCapaAlAlmacenYDeVuelta() {
        let g = ReglasCapa.guardado(centro: CGPoint(x: 300, y: 200), tamanoBase: CGSize(width: 200, height: 100),
                                    escala: 1.5, escalaPantalla: 2, opacidad: 0.7)
        XCTAssertEqual(g.x, 150)
        XCTAssertEqual(g.y, 125)
        XCTAssertEqual(g.ancho, 300)
        XCTAssertEqual(g.alto, 150)
        XCTAssertEqual(g.zoomPorCien, 150)
        XCTAssertEqual(g.escalaPorCien, 200)
        XCTAssertEqual(g.opacidad, 70)
        let v = ReglasCapa.deGuardado(g)
        XCTAssertEqual(v.centro, CGPoint(x: 300, y: 200))
        XCTAssertEqual(v.tamanoBase.width, 200, accuracy: 0.001)
        XCTAssertEqual(v.tamanoBase.height, 100, accuracy: 0.001)
        XCTAssertEqual(v.escala, 1.5, accuracy: 0.001)
    }

    func testLaOpacidadNoBajaDelMinimo() {
        let g = ReglasCapa.guardado(centro: .zero, tamanoBase: CGSize(width: 10, height: 10), escala: 1,
                                    escalaPantalla: 2, opacidad: 0.01)
        XCTAssertEqual(g.opacidad, 20)
    }

    func testUnaRutaDeOtroArranqueSeBuscaEnLaCarpetaDeAhora() {
        let datos = URL(fileURLWithPath: "/nuevo/Library/Application Support/PixPinMax", isDirectory: true)
        let vieja = "/viejo/Library/Application Support/PixPinMax/proyectos/A/video.mp4"
        let nueva = "/nuevo/Library/Application Support/PixPinMax/proyectos/A/video.mp4"
        XCTAssertEqual(ReglasCapa.rutaVigente(vieja, datos: datos, existe: { $0 == nueva })?.path, nueva)
        XCTAssertNil(ReglasCapa.rutaVigente(vieja, datos: datos, existe: { _ in false }))
        XCTAssertEqual(ReglasCapa.rutaVigente("/fuera/x.pdf", datos: datos, existe: { _ in true })?.path, "/fuera/x.pdf")
    }

    func testLasHerramientasNacenConSuTamano() {
        XCTAssertEqual(ReglasCapa.tamanoHerramienta("ruleta").height, 420)
        XCTAssertEqual(ReglasCapa.tamanoHerramienta("tareas"), CGSize(width: 300, height: 340))
        XCTAssertEqual(ReglasCapa.tamanoHerramienta(nil), CGSize(width: 300, height: 260))
    }

    // MARK: Panel

    func testElPanelAgrupaPorColorYBuscaSinTildes() {
        let filas = [
            FilaPin(id: 1, nombre: "Compras", detalle: "Nota", tipo: .nota, grupo: 1, oculto: false),
            FilaPin(id: 2, nombre: "Tesis.pdf", detalle: "PDF", tipo: .pdf, grupo: nil, oculto: false),
            FilaPin(id: 3, nombre: "Cronómetro", detalle: "Cronómetro", tipo: .herramienta, grupo: 0, oculto: true),
            FilaPin(id: 4, nombre: "Foto", detalle: "Foto", tipo: .foto, grupo: 1, oculto: false),
        ]
        let grupos = ReglasPanel.agrupar(filas)
        XCTAssertEqual(grupos.map(\.grupo), [0, 1, nil], "los colores en su orden y sin grupo al final")
        XCTAssertEqual(grupos[1].filas.map(\.id), [1, 4])
        XCTAssertEqual(ReglasPanel.filtrar(filas, busqueda: "cronometro").map(\.id), [3])
        XCTAssertEqual(ReglasPanel.filtrar(filas, busqueda: "  ").count, 4)
        XCTAssertEqual(ColoresGrupo.nombre(9), "Naranja", "da la vuelta")
    }

    // MARK: Un pin

    @MainActor
    func testUnPinGiradoSeGuardaYVuelveIgual() throws {
        let foto = try XCTUnwrap(Self.foto(ancho: 40, alto: 20))
        let pin = PinIPad(id: 1, contenido: .imagen(foto, fichero: nil), centro: CGPoint(x: 100, y: 100),
                          tamanoBase: CGSize(width: 40, height: 20))
        pin.girar(1)
        XCTAssertEqual(pin.tamanoBase, CGSize(width: 20, height: 40), "girar un cuarto cambia ancho y alto")
        XCTAssertEqual(pin.vista?.width, 20)
        pin.ponerFiltros(FiltrosPin(gris: true, invertido: false, brillo: 2))
        pin.opacidad = 0.5
        pin.escala = 2
        let g = pin.guardado(escalaPantalla: 2)
        XCTAssertEqual(g.giro, 1)
        XCTAssertTrue(g.gris)
        XCTAssertEqual(g.brillo, 2)
        XCTAssertEqual(g.opacidad, 50)
        XCTAssertEqual(g.ancho, 40)
        XCTAssertEqual(g.alto, 80)

        // Vuelve: el tamano ya girado, y la foto se gira al aplicar.
        let sitio = ReglasCapa.deGuardado(g)
        let vuelta = PinIPad(id: 2, contenido: .imagen(foto, fichero: nil), centro: sitio.centro,
                             tamanoBase: sitio.tamanoBase)
        vuelta.escala = sitio.escala
        vuelta.aplicar(g)
        XCTAssertEqual(vuelta.tamano, pin.tamano)
        XCTAssertEqual(vuelta.vista?.width, 20)
        XCTAssertEqual(vuelta.opacidad, 0.5, accuracy: 0.001)
        XCTAssertEqual(vuelta.guardado(escalaPantalla: 2), g)

        // Restaurar quita filtros y giro (y devuelve el tamano).
        vuelta.restaurarVista()
        XCTAssertEqual(vuelta.tamanoBase, CGSize(width: 40, height: 20))
        XCTAssertEqual(vuelta.vista?.width, 40)
    }

    @MainActor
    func testUnaFotoAnotadaGuardaSuProporcion() throws {
        let foto = try XCTUnwrap(Self.foto(ancho: 40, alto: 20))
        let pin = PinIPad(id: 1, contenido: .imagen(foto, fichero: nil), centro: .zero,
                          tamanoBase: CGSize(width: 200, height: 100))
        pin.cambiarImagen(try XCTUnwrap(Self.foto(ancho: 40, alto: 40)))
        XCTAssertEqual(pin.tamanoBase, CGSize(width: 200, height: 200))
        XCTAssertEqual(pin.fila.detalle, "Captura")
    }

    func testElAlmacenGuardaLoDeLaCapa() throws {
        let raiz = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-capa-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: raiz) }
        let g = ReglasCapa.guardado(centro: CGPoint(x: 400, y: 300), tamanoBase: CGSize(width: 100, height: 50),
                                    escala: 2, escalaPantalla: 2, opacidad: 0.8)
        let id = try AlmacenPines(raiz: raiz).guardarNota("hola", origen: AlmacenPines.origenMini("tareas", chat: "c", mensaje: "m"), pin: g)
        let otra = try AlmacenPines(raiz: raiz)
        XCTAssertEqual(otra.abiertas.map(\.id), [id])
        XCTAssertEqual(otra.entrada(id)?.pin, g)
        XCTAssertEqual(AlmacenPines.miniDeOrigen(otra.entrada(id)?.origen ?? "")?.cual, "tareas")
    }

    static func foto(ancho: Int, alto: Int) -> CGImage? {
        guard let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: espacio, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: ancho, height: alto))
        return ctx.makeImage()
    }
}
