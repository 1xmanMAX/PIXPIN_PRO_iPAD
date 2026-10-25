// El lienzo con Ajustes → Lienzo, Tareas y Pinear (`AjustesOrganizar`): la
// barra sin lo apagado, encender y apagar por sitio, el suavizado de 1 euro,
// las palabras magicas; y por el puente de verdad: la tabla que se edita, los
// marcadores, la zona al chat con su enlace y la fusion de paginas.

import Foundation
import PixPinCore
@testable import PixPinKit
import UIKit
import XCTest

private func carpetaTemporal(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-lienzo-ajustes-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

final class LienzoAjustesMarcasTests: XCTestCase {
    // MARK: Ajustes → Lienzo

    func testLaBarraSinLoApagadoYElCorteSigueEnSuSitio() {
        let barra = BotonBarraLienzo.barraIPad
        let corte = 5
        let (todo, mismo) = BotonBarraLienzo.visibles(barra, corte: corte, ocultas: [])
        XCTAssertEqual(todo, barra)
        XCTAssertEqual(mismo, corte)
        // Con la goma y todo el grupo de medir apagados no salen; el corte
        // pasa a la barra que queda.
        let medir = Set(GrupoHerramientaLienzo.medir.miembros.map(\.id))
        let (quedan, nuevo) = BotonBarraLienzo.visibles(barra, corte: barra.count, ocultas: medir.union(["borrador"]))
        XCTAssertFalse(quedan.contains(.grupo(.medir)))
        XCTAssertFalse(quedan.contains(.herramienta("borrador")))
        XCTAssertTrue(quedan.contains(.grupo(.formas)))
        XCTAssertEqual(nuevo, quedan.count)
        // Un grupo con una sola apagada sigue, sin ella.
        XCTAssertFalse(GrupoHerramientaLienzo.formas.miembros(sin: ["rombo"]).contains { $0.id == "rombo" })
    }

    @MainActor
    func testLaCaraDeUnGrupoNoEsUnaApagada() {
        GruposBarraLienzo.olvidar()
        let primera = GrupoHerramientaLienzo.formas.miembros[0].id
        let cara = GruposBarraLienzo.cara(.formas, activa: "lapiz", ocultas: [primera])
        XCTAssertNotNil(cara)
        XCTAssertNotEqual(cara?.id, primera)
    }

    func testLosNombresDelTomlYDondeSale() throws {
        XCTAssertEqual(AjustesOrganizarIPad.nombreEnToml("flecha_libre"), "flecha-libre")
        XCTAssertEqual(AjustesOrganizarIPad.nombreEnToml("zona_chat"), "zona")
        XCTAssertNil(AjustesOrganizarIPad.nombreEnToml("seleccion"), "la flecha no se apaga")
        // Todas las de la barra tienen nombre del TOML (o son la flecha).
        let nombres = Set(nombresDeHerramientas())
        for h in HerramientaLienzo.todas {
            guard let n = AjustesOrganizarIPad.nombreEnToml(h.id) else { continue }
            XCTAssertTrue(nombres.contains(n), "\(h.id) → \(n) no esta en [herramientas]")
        }
    }

    func testEncenderEnUnSitioLaQueEstabaApagadaEnTodos() throws {
        let raiz = try carpetaTemporal("sitios")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        var a = nucleo.ajustesOrganizarDeFabrica()
        HerramientasAjustes.alternar("lazo", en: nil, &a)
        XCTAssertFalse(HerramientasAjustes.activa("lazo", en: "pin", a))
        HerramientasAjustes.alternar("lazo", en: "lienzo", &a)
        XCTAssertTrue(HerramientasAjustes.activa("lazo", en: "lienzo", a))
        XCTAssertFalse(HerramientasAjustes.activa("lazo", en: "pin", a), "los demas sitios siguen como estaban")
        XCTAssertTrue(AjustesOrganizarIPad.sale("lazo", en: "lienzo", con: a))
        XCTAssertFalse(AjustesOrganizarIPad.sale("lazo", en: "lector", con: a))
        HerramientasAjustes.encenderEnTodos("lazo", &a)
        XCTAssertTrue(HerramientasAjustes.activaEnTodos("lazo", a))
        // Va y vuelve por el TOML.
        HerramientasAjustes.alternar("mosaico", en: "pin", &a)
        try nucleo.guardarAjustesOrganizar(nuevos: a)
        let b = try nucleo.ajustesOrganizar()
        XCTAssertFalse(HerramientasAjustes.activa("mosaico", en: "pin", b))
        XCTAssertTrue(HerramientasAjustes.activa("mosaico", en: "lienzo", b))
    }

    func testElFiltroDeUnEuroQuitaElTemblorQuietoYNoSeQuedaAtrasCorriendo() {
        var f = FiltroUnEuro()
        // Quieto con temblor de ±2: la salida tiembla mucho menos.
        var salida: [Float] = []
        for i in 0 ..< 60 {
            let x: Float = 100 + (i % 2 == 0 ? 2 : -2)
            salida.append(f.filtrar(x, 0, ms: Double(i) * 4).0)
        }
        let temblor = zip(salida.dropFirst(), salida).map { abs($0 - $1) }.suffix(20).max() ?? 0
        XCTAssertLessThan(temblor, 2)
        // Reiniciado, el primer punto es el que llega.
        f.reiniciar()
        XCTAssertEqual(f.filtrar(500, 300, ms: 0).0, 500)
        XCTAssertEqual(FiltroUnEuro.alfa(1, 0), 1, "sin tiempo deja pasar la muestra")
    }

    // MARK: Pinear y Tareas

    func testLasPalabrasMagicasYLasDeUrgencia() throws {
        let raiz = try carpetaTemporal("magia")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        XCTAssertEqual(AjustesOrganizarIPad.palabraMagica(" Timer ", nucleo: nucleo), "TIMER")
        XCTAssertNil(AjustesOrganizarIPad.palabraMagica("el timer", nucleo: nucleo))
        var a = try nucleo.ajustesOrganizar()
        a.palabrasMagicas = palabrasMagicasCambiadas(guardadas: nil, app: "DRAW", escrito: "dibujo, boceto")
        a.palabrasPropias = true
        a.tareasUrgente = FilasAjustes.palabras(de: "ya, hoy,\n ,pronto")
        try nucleo.guardarAjustesOrganizar(nuevos: a)
        AjustesOrganizarIPad.cargar(nucleo: nucleo)
        XCTAssertEqual(AjustesOrganizarIPad.palabraMagica("Boceto", nucleo: nucleo), "DRAW")
        XCTAssertEqual(try nucleo.ajustesOrganizar().tareasUrgente, ["ya", "hoy", "pronto"])
    }

    @MainActor
    func testLasFilasNuevasTienenIdUnicoYSeRestablecenDeUnaVez() async throws {
        let todos = FilasAjustes.todas.map(\.id)
        XCTAssertEqual(Set(todos).count, todos.count)
        XCTAssertFalse(FilasAjustes.de(.lienzo).isEmpty)
        XCTAssertTrue(FilasAjustes.buscar("iman").contains { $0.id == "iman" })
        let raiz = try carpetaTemporal("filas")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        let suite = "pixpin-ajustes-organizar-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let m = ModeloAjustes(nucleo: nucleo, defaults: defaults)
        await m.cargar()
        m.cambiarOrganizar { $0.imanRadioPx = 30; $0.suavizado = "natural" }
        XCTAssertTrue(FilasAjustes.hayCambios(en: .lienzo, m))
        let pasos = m.historial.count
        FilasAjustes.restablecer(.lienzo, m)
        XCTAssertFalse(FilasAjustes.hayCambios(en: .lienzo, m))
        XCTAssertEqual(m.historial.count, pasos + 1, "un solo paso de «Deshacer»")
        m.deshacer()
        XCTAssertEqual(m.organizarActual.imanRadioPx, 30)
        await m.guardar()
        XCTAssertEqual(try nucleo.ajustesOrganizar().suavizado, "natural")
    }

    // MARK: La tabla, los marcadores, la zona y la fusion

    func testLaRejillaDeLaTablaAnadeYQuitaFilasYColumnas() {
        var r = RejillaTabla(celdas: [["a", "b"], ["1"]], cabecera: true)
        XCTAssertEqual(r.celdas, [["a", "b"], ["1", ""]], "todas las filas del mismo ancho")
        r.activa = (0, 0)
        r.masFila()
        XCTAssertEqual(r.filas, 3)
        XCTAssertEqual(r.celdas[1], ["", ""])
        r.masColumna()
        XCTAssertEqual(r.columnas, 3)
        r.menosColumna()
        r.menosFila()
        XCTAssertEqual(r.celdas, [["a", "b"], ["1", ""]])
        var una = RejillaTabla(celdas: [["x"]], cabecera: false)
        una.menosFila()
        una.menosColumna()
        XCTAssertEqual(una.celdas, [["x"]], "siempre queda una celda")
    }

    func testLaTablaDibujadaSeEditaPorElPuente() throws {
        let raiz = try carpetaTemporal("tabla")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let ed = try EditorLienzo.deFichero(ruta: raiz.appendingPathComponent("t.excalidraw").path, crear: true)
        XCTAssertTrue(ed.estamparTabla(celdas: [["a", "b"], ["1", "2"]], cabecera: true, centroX: 0, centroY: 0))
        let t = try XCTUnwrap(ed.tablaElegida())
        var r = RejillaTabla(celdas: t.celdas, cabecera: t.cabecera)
        r.activa = (1, 1)
        r.masFila()
        r.poner("3", fila: 2, columna: 0)
        XCTAssertTrue(ed.reemplazarTabla(celdas: r.celdas, cabecera: r.cabecera))
        XCTAssertEqual(ed.tablaElegida()?.celdas.count, 3)
    }

    func testLosMarcadoresSeGuardanJuntoAlDibujoConSuVista() throws {
        let raiz = try carpetaTemporal("marcas")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let ruta = raiz.appendingPathComponent("d.excalidraw").path
        _ = try EditorLienzo.deFichero(ruta: ruta, crear: true)
        let m = MarcasDelLienzo.juntoAlDibujo(ruta: ruta)
        XCTAssertTrue(m.lista().isEmpty)
        let camara = CamaraDelLienzo(x: 0, y: 0, zoom: 2)
        let id = m.poner(x: 10, y: 20, emoji: "⭐", camara: camara, anchoPx: 800, altoPx: 600)
        let otro = MarcasDelLienzo.juntoAlDibujo(ruta: ruta)
        XCTAssertEqual(otro.lista().first?.emoji, "⭐")
        XCTAssertEqual(otro.lista().first?.conVista, true, "se planta con la vista de ese momento")
        // Volver lleva a su vista, con su aumento.
        let d = otro.destino(id: id, camara: CamaraDelLienzo(x: 500, y: 500, zoom: 1), anchoPx: 800, altoPx: 600,
                             bajoLaBarra: 110)
        XCTAssertEqual(d?.zoom ?? 0, 2, accuracy: 0.01)
        // Casos negativos: una marca que no esta no se quita ni tiene destino.
        XCTAssertFalse(otro.quitar(id: id + 1))
        XCTAssertNil(otro.destino(id: id + 1, camara: camara, anchoPx: 800, altoPx: 600, bajoLaBarra: 110))
        XCTAssertTrue(otro.quitar(id: id))
        XCTAssertTrue(otro.lista().isEmpty)
        XCTAssertFalse(otro.emojis().isEmpty)
    }

    func testLaZonaVaAlChatYSuEnlaceLlevaAlSublienzo() throws {
        let raiz = try carpetaTemporal("zona")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        let chat = try nucleo.crearChat(nombre: "Planta").id
        let lienzo = try nucleo.crearLienzo(chat: chat, nombre: "Planta baja")
        let dibujo = try XCTUnwrap(lienzo.referencia)
        let png = UIGraphicsImageRenderer(size: CGSize(width: 8, height: 8)).pngData { c in
            UIColor.red.setFill()
            c.fill(CGRect(x: 0, y: 0, width: 8, height: 8))
        }
        let z = try nucleo.zonaAlChat(chat: chat, dibujo: dibujo, pagina: nil, nombre: "Planta baja", png: png,
                                      ancho: 8, alto: 8)
        XCTAssertEqual(z.mensaje.clase, "IMAGEN")
        XCTAssertEqual(nucleo.dibujoDelEnlace(chat: chat, enlace: z.dibujo), z.dibujo)
        XCTAssertEqual(nucleo.mensajeDelDibujo(chat: chat, dibujo: z.dibujo)?.id, z.mensaje.id)
        let ed = try nucleo.editarLienzo(chat: chat, dibujo: dibujo)
        XCTAssertTrue(ed.marcarZona(x0: 0, y0: 0, x1: 100, y1: 50, dibujo: z.dibujo))
        XCTAssertEqual(ed.enlaceEn(x: 100, y: 0, zoom: 1), z.dibujo)
        XCTAssertEqual(ed.enlacesEnLaVista(x0: 0, y0: 0, x1: 300, y1: 300).count, 1)
    }

    func testFusionarDosPaginasDeUnPdf() throws {
        let raiz = try carpetaTemporal("fusion")
        defer { try? FileManager.default.removeItem(at: raiz) }
        // Un PDF de tres paginas.
        let pdf = raiz.appendingPathComponent("p.pdf")
        let datos = UIGraphicsPDFRenderer(bounds: CGRect(x: 0, y: 0, width: 300, height: 400)).pdfData { c in
            for _ in 0 ..< 3 { c.beginPage() }
        }
        try datos.write(to: pdf)
        let hechas = FusionarPaginasIPad.pintar(pdf: pdf, paginas: [0, 2, 7])
        XCTAssertEqual(hechas.map(\.pagina), [0, 2], "la que no esta se salta")
        XCTAssertTrue(hechas.allSatisfy { $0.ancho >= 700 && !$0.jpg.isEmpty })
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        let chat = try nucleo.crearChat(nombre: "Plano").id
        let m = try nucleo.fusionarPaginas(chat: chat, paginas: hechas)
        XCTAssertEqual(m.nombre, "Páginas 1 y 3")
        XCTAssertEqual(try nucleo.editarLienzo(chat: chat, dibujo: XCTUnwrap(m.referencia)).imagenes().count, 2)
    }
}
