import PixPinCore
import XCTest
@testable import PixPinKit

/// Lo comun del sistema: el buzon del App Group, las ventanas de los
/// visores, los ficheros que llegan de fuera, las acciones rapidas y los
/// widgets.
final class SistemaTests: XCTestCase {
    private var raiz: URL!

    override func setUpWithError() throws {
        raiz = FileManager.default.temporaryDirectory.appendingPathComponent("buzon-\(UUID().uuidString)", isDirectory: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: raiz)
    }

    // MARK: El buzon

    func testLaListaDeProyectosVaYVuelveYSeOrdenaComoEnWindows() throws {
        let p = [
            ProyectoCompartido(id: "a", nombre: "Casa Rosales", tocado: 5, esGuardados: false),
            ProyectoCompartido(id: "g", nombre: "Mensajes guardados", tocado: 1, esGuardados: true),
            ProyectoCompartido(id: "t", nombre: "Tesis", tocado: 9, esGuardados: false),
        ]
        try BuzonCompartido.escribirProyectos(p, en: raiz)
        let leidos = BuzonCompartido.leerProyectos(en: raiz)
        XCTAssertEqual(Set(leidos), Set(p))
        // Guardados primero; luego lo ultimo tocado.
        XCTAssertEqual(BuzonCompartido.ordenados(leidos).map(\.id), ["g", "t", "a"])
        // Buscar sin mirar mayusculas ni tildes.
        XCTAssertEqual(BuzonCompartido.ordenados(leidos, buscando: "ROSÁLES").map(\.id), ["a"])
        XCTAssertEqual(BuzonCompartido.ordenados(leidos, buscando: "   ").count, 3)
    }

    func testSinListaNoHayProyectos() {
        XCTAssertTrue(BuzonCompartido.leerProyectos(en: raiz).isEmpty)
    }

    func testLosNombresNoChocanNiSeSalenDeLaCarpeta() {
        XCTAssertEqual(BuzonCompartido.nombreLibre("foto.png", usados: []), "foto.png")
        XCTAssertEqual(BuzonCompartido.nombreLibre("foto.png", usados: ["Foto.png", "foto (2).png"]), "foto (3).png")
        XCTAssertEqual(BuzonCompartido.nombreLibre("notas", usados: ["notas"]), "notas (2)")
        XCTAssertFalse(BuzonCompartido.nombreLibre("../../a/b:c.pdf", usados: []).contains("/"))
        XCTAssertFalse(BuzonCompartido.nombreLibre(".oculto", usados: []).hasPrefix("."))
        XCTAssertEqual(BuzonCompartido.nombreLibre("  ", usados: []), "archivo")
        XCTAssertLessThanOrEqual(BuzonCompartido.nombreLibre(String(repeating: "x", count: 300) + ".pdf", usados: []).count, 120)
    }

    func testLaNotaJuntaComentarioYTextosSinRepetir() {
        var e = EntregaCompartida(chat: "a", nombreDelChat: "Casa", comentario: " Mira esto ",
                                  textos: ["Mira esto", "https://ejemplo.es", ""], ficheros: [], creada: 0)
        XCTAssertEqual(BuzonCompartido.textoDeLaNota(e), "Mira esto\nhttps://ejemplo.es")
        e.comentario = ""
        e.textos = ["  "]
        XCTAssertNil(BuzonCompartido.textoDeLaNota(e))
    }

    func testUnaEntregaSoloSeRecogeCuandoEstaAcabada() throws {
        let c = try BuzonCompartido.nuevaEntrega(en: raiz)
        let n1 = try c.anadir(datos: Data("hola".utf8), nombre: "nota.txt")
        let n2 = try c.anadir(datos: Data("otra".utf8), nombre: "nota.txt")
        XCTAssertEqual(n1, "nota.txt")
        XCTAssertEqual(n2, "nota (2).txt")
        // A medias (sin `entrega.json`): no se recoge.
        XCTAssertTrue(BuzonCompartido.entregasPendientes(en: raiz).isEmpty)
        let e = EntregaCompartida(chat: "a", nombreDelChat: "Casa", comentario: "", textos: [], ficheros: [n1, n2], creada: 1)
        try c.cerrar(e)
        let pendientes = BuzonCompartido.entregasPendientes(en: raiz)
        XCTAssertEqual(pendientes.count, 1)
        XCTAssertEqual(pendientes.first?.entrega, e)
        // Apartada no se reintenta sola; «Reintentar» la devuelve.
        BuzonCompartido.apartar(pendientes[0].carpeta)
        XCTAssertTrue(BuzonCompartido.entregasPendientes(en: raiz).isEmpty)
        XCTAssertEqual(BuzonCompartido.cuantasApartadas(en: raiz), 1)
        BuzonCompartido.reintentarApartadas(en: raiz)
        XCTAssertEqual(BuzonCompartido.entregasPendientes(en: raiz).count, 1)
        BuzonCompartido.quitar(pendientes[0].carpeta)
        XCTAssertTrue(BuzonCompartido.entregasPendientes(en: raiz).isEmpty)
    }

    func testLoAMediasViejoSeBorra() throws {
        let c = try BuzonCompartido.nuevaEntrega(en: raiz)
        _ = BuzonCompartido.entregasPendientes(en: raiz, ahora: Date())
        XCTAssertTrue(FileManager.default.fileExists(atPath: c.url.path), "lo reciente se deja: la extension puede estar escribiendo")
        _ = BuzonCompartido.entregasPendientes(en: raiz, ahora: Date().addingTimeInterval(2 * 86_400))
        XCTAssertFalse(FileManager.default.fileExists(atPath: c.url.path))
    }

    func testLosWidgetsVanYVuelven() throws {
        let w = DatosDeWidgets(escrita: 3, capturas: [.init(miniatura: "a.jpg", nombre: "Captura", cuando: 2)],
                               tareasDeHoy: [.init(titulo: "Llamar", proyecto: "Casa", hecha: false)])
        try BuzonCompartido.escribirWidgets(w, en: raiz)
        XCTAssertEqual(BuzonCompartido.leerWidgets(en: raiz), w)
    }

    @MainActor
    func testSeOfrecenLosProyectosSinArchivadosNiAsistente() {
        func ficha(_ id: String, archivado: Bool = false, asistente: Bool = false) -> FichaChat {
            FichaChat(id: id, nombre: id, tocado: 0, resumen: "", hojas: 0, sinLeer: 0,
                      esGuardados: false, esAsistente: asistente, archivado: archivado)
        }
        let ofrecidos = RecogedorCompartido.proyectos(de: [ficha("a"), ficha("b", archivado: true), ficha("c", asistente: true)])
        XCTAssertEqual(ofrecidos.map(\.id), ["a"])
    }

    @MainActor
    func testElResumenDeLoRecogido() {
        var r = RecogedorCompartido.Resumen()
        XCTAssertNil(r.texto)
        r.anadir(proyecto: "Casa", ficheros: 2, nota: true)
        r.anadir(proyecto: "Casa", ficheros: 1, nota: false)
        XCTAssertEqual(r.texto, "Llegó a «Casa»: 3 archivos y 1 nota.")
        r.fallos = 1
        XCTAssertTrue(r.texto?.contains("no se pudo") == true)
    }

    // MARK: Ventanas

    func testLaPeticionDeVentanaGuardaRutasRelativasALaCarpetaDeDatos() throws {
        let datos = URL(fileURLWithPath: "/var/datos/PixPinMax", isDirectory: true)
        let dentro = datos.appendingPathComponent("proyectos/abc/plano.dwg")
        XCTAssertEqual(PeticionVentana.guardar(dentro, datos: datos), "proyectos/abc/plano.dwg")
        XCTAssertEqual(PeticionVentana.url(de: "proyectos/abc/plano.dwg", datos: datos).path, dentro.path)
        XCTAssertEqual(PeticionVentana.guardar(URL(fileURLWithPath: "/tmp/x.pdf"), datos: datos), "/tmp/x.pdf")
        XCTAssertEqual(PeticionVentana.url(de: "/tmp/x.pdf", datos: datos).path, "/tmp/x.pdf")

        let p = PeticionVentana(.plano(url: dentro, titulo: "Planta baja"), datos: datos)
        XCTAssertEqual(p.clase, .plano)
        XCTAssertEqual(p.ruta, "proyectos/abc/plano.dwg")
        XCTAssertEqual(p.nombre, "Planta baja")
        let vuelta = try JSONDecoder().decode(PeticionVentana.self, from: JSONEncoder().encode(p))
        XCTAssertEqual(vuelta, p)
        XCTAssertEqual(PeticionVentana(clase: .nota).nombre, "Nota")
    }

    // MARK: Ficheros de fuera

    func testCadaFicheroDeFueraVaASuSitio() {
        let landxml = Data("<?xml version=\"1.0\"?>\n<LandXML xmlns=\"http://www.landxml.org/schema/LandXML-1.2\">".utf8)
        XCTAssertEqual(AbrirFicheroDeFuera.destino(de: URL(fileURLWithPath: "/a/b.excalidraw")), .lienzo)
        XCTAssertEqual(AbrirFicheroDeFuera.destino(de: URL(fileURLWithPath: "/a/b.pixpin")), .proyecto)
        XCTAssertEqual(AbrirFicheroDeFuera.destino(de: URL(fileURLWithPath: "/a/b.PIXPIN2D")), .dibujoAntiguo)
        XCTAssertEqual(AbrirFicheroDeFuera.destino(de: URL(fileURLWithPath: "/a/b.landxml")), .terreno)
        XCTAssertEqual(AbrirFicheroDeFuera.destino(de: URL(fileURLWithPath: "/a/b.xml"), cabeza: landxml), .terreno)
        XCTAssertEqual(AbrirFicheroDeFuera.destino(de: URL(fileURLWithPath: "/a/b.xml"), cabeza: Data("<rss>".utf8)), .visor)
        XCTAssertEqual(AbrirFicheroDeFuera.destino(de: URL(fileURLWithPath: "/a/b.md")), .visor)
        XCTAssertEqual(AbrirFicheroDeFuera.destino(de: URL(fileURLWithPath: "/a/b.rvt")), .visor)
        XCTAssertFalse(AbrirFicheroDeFuera.esLandXML(nil))
    }

    // MARK: Acciones rapidas

    @MainActor
    func testLasAccionesRapidasSonLasDelPlist() {
        XCTAssertEqual(AccionesRapidas.Accion.allCases.count, 4)
        XCTAssertEqual(AccionesRapidas.Accion.nota.pedido?.accion, "nota_nueva")
        XCTAssertEqual(AccionesRapidas.Accion.buscar.pedido?.texto("cual"), "buscar")
        XCTAssertEqual(AccionesRapidas.Accion.sincronizar.pedido?.accion, "sincronizar")
        XCTAssertNil(AccionesRapidas.Accion.capturar.pedido)
        XCTAssertFalse(AccionesRapidas.hacer("otra.cosa"))
    }

    // MARK: Widgets

    func testLasTareasDeHoy() {
        let dia: Int64 = 86_400_000
        let desfase: Int64 = 2 * 3_600_000
        let ahora: Int64 = 1_760_000_000_000
        let local = ahora + desfase
        let hoy = local - local % dia
        func t(_ i: UInt32, _ texto: String, hecha: Bool = false, recuerda: Int64? = nil, dias: UInt32? = nil) -> Tarea {
            Tarea(indice: i, crudo: texto, texto: texto, hecha: hecha, recuerda: recuerda, creada: nil, dias: dias, imagenes: [])
        }
        let lista = ListaTareas(chat: "c", nombreChat: "Casa", guardados: false, mensaje: "m", titulo: "Tareas",
                                cuando: 0, esInbox: false, tareas: [
                                    t(0, "apuntada hoy", dias: 0),
                                    t(1, "a las diez", recuerda: hoy + 10 * 3_600_000),
                                    t(2, "a las ocho", recuerda: hoy + 8 * 3_600_000),
                                    t(3, "mañana", recuerda: hoy + dia + 1),
                                    t(4, "hecha", hecha: true, dias: 0),
                                    t(5, "vieja", dias: 9),
                                ], pendientes: 5)
        let hechas = DatosDeWidgetsIPad.tareasDeHoy([lista], ahoraMs: ahora, desfaseMs: desfase)
        XCTAssertEqual(hechas.map(\.titulo), ["a las ocho", "a las diez", "apuntada hoy"])
        XCTAssertEqual(hechas.first?.proyecto, "Casa")
    }
}
