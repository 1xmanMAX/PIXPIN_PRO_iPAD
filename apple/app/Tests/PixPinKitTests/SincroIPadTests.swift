// Pruebas de lo propio del iPad en Sincro y Ajustes: leer el codigo del
// grupo de un QR o de un texto pegado, el permiso de red local, copiar lo
// elegido en Archivos antes de enviarlo, el nombre de este iPad al
// renombrarlo, el tema y los ajustes de `pixpinmax.toml` (que se guardan y se
// leen otra vez, y que Aparatos y Ajustes comparten modelo).

import Foundation
import Network
import PixPinCore
@testable import PixPinKit
import XCTest

/// Una carpeta temporal propia de estas pruebas (la de otras zonas puede
/// llamarse igual).
func carpetaTemporalSincro(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-ipad-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

final class SincroCodigoDeGrupoTests: XCTestCase {
    func testElCodigoSaleDeLoTecleadoDelQRYDeUnaFrase() throws {
        let raiz = try carpetaTemporalSincro("codigo")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let n = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "PC")
        let codigo = try n.crearGrupo()
        let legible = codigoLegible(codigo: codigo)
        XCTAssertEqual(codigoDeGrupoEn(legible), legible)
        XCTAssertEqual(codigoDeGrupoEn("  \(legible)\n"), legible)
        // Dentro de otra cosa: el QR con prefijo o una frase copiada.
        XCTAssertEqual(codigoDeGrupoEn("pixpin-grupo:\(legible)"), legible)
        XCTAssertEqual(codigoDeGrupoEn("Mi código es \(legible), gracias"), legible)
        // Casos negativos: nada, un envio de seis cifras, una invitacion.
        XCTAssertNil(codigoDeGrupoEn(""))
        XCTAssertNil(codigoDeGrupoEn("pixpin-envio:1:482913:192.168.1.4:47474"))
        XCTAssertNil(codigoDeGrupoEn("hola"))
    }

    func testElQRDelGrupoSePintaCuadrado() throws {
        let qr = try XCTUnwrap(imagenQR("K7Q2M-9XMPA", lado: 300))
        XCTAssertEqual(qr.size.width, qr.size.height)
        XCTAssertGreaterThanOrEqual(qr.size.width, 150)
    }
}

final class SincroPermisoRedLocalTests: XCTestCase {
    func testLaRedLocalNegadaSeReconoceYLoDemasNo() {
        XCTAssertTrue(PermisoRedLocal.esNegado(.waiting(.dns(PermisoRedLocal.politicaDenegada))))
        XCTAssertTrue(PermisoRedLocal.esNegado(.failed(.dns(PermisoRedLocal.politicaDenegada))))
        XCTAssertFalse(PermisoRedLocal.esNegado(.waiting(.dns(-65537))))
        XCTAssertFalse(PermisoRedLocal.esNegado(.waiting(.posix(.ENETDOWN))))
        XCTAssertFalse(PermisoRedLocal.esNegado(.ready))
    }

    @MainActor
    func testSinExplicarNoSeMiraYAlContinuarQuedaApuntado() throws {
        let nombre = "pixpin-pruebas-\(UUID().uuidString)"
        let d = try XCTUnwrap(UserDefaults(suiteName: nombre))
        defer { d.removePersistentDomain(forName: nombre) }
        let p = PermisoRedLocal(defaults: d)
        XCTAssertEqual(p.estado, .sinExplicar)
        // Sin explicar, mirar no provoca el aviso del sistema.
        p.mirar()
        XCTAssertEqual(p.estado, .sinExplicar)
        p.continuar()
        XCTAssertNotEqual(p.estado, .sinExplicar)
        XCTAssertTrue(d.bool(forKey: PermisoRedLocal.claveExplicado))
        // La siguiente vez ya no se ensena la pantalla de antes.
        XCTAssertEqual(PermisoRedLocal(defaults: d).estado, .bien)
    }
}

final class SincroElegirArchivosTests: XCTestCase {
    func testLoElegidoSeCopiaConSuNombreAunqueSeRepita() async throws {
        let raiz = try carpetaTemporalSincro("elegir")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let a = raiz.appendingPathComponent("a", isDirectory: true)
        let b = raiz.appendingPathComponent("b", isDirectory: true)
        try FileManager.default.createDirectory(at: a, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: b, withIntermediateDirectories: true)
        try Data("uno".utf8).write(to: a.appendingPathComponent("foto.png"))
        try Data("dos".utf8).write(to: b.appendingPathComponent("foto.png"))
        let falta = raiz.appendingPathComponent("no-esta.pdf")
        let copias = await copiarParaEnviar([a.appendingPathComponent("foto.png"),
                                             b.appendingPathComponent("foto.png"), falta])
        // El que no existe se salta; los dos con el mismo nombre llegan con el.
        XCTAssertEqual(copias.count, 2)
        XCTAssertEqual(copias.map(\.lastPathComponent), ["foto.png", "foto.png"])
        XCTAssertNotEqual(copias[0], copias[1])
        XCTAssertEqual(try String(contentsOf: copias[0], encoding: .utf8), "uno")
        XCTAssertEqual(try String(contentsOf: copias[1], encoding: .utf8), "dos")
        for c in copias { try? FileManager.default.removeItem(at: c.deletingLastPathComponent()) }
    }
}

@MainActor
final class SincroNombreDelIPadTests: XCTestCase {
    func testRenombrarGuardaElNombreParaElProximoArranque() async throws {
        let raiz = try carpetaTemporalSincro("nombre")
        let antes = UserDefaults.standard.string(forKey: claveNombreDelAparato)
        defer {
            try? FileManager.default.removeItem(at: raiz)
            if let antes {
                UserDefaults.standard.set(antes, forKey: claveNombreDelAparato)
            } else {
                UserDefaults.standard.removeObject(forKey: claveNombreDelAparato)
            }
        }
        let s = Sincronizador(nucleo: try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad"))
        await s.renombrar("  iPad de Max  ")
        XCTAssertNil(s.error)
        XCTAssertEqual(s.aparato?.nombre, "iPad de Max")
        XCTAssertEqual(UserDefaults.standard.string(forKey: claveNombreDelAparato), "iPad de Max")
        XCTAssertEqual(nombreDeEsteAparato(), "iPad de Max")
        // Caso negativo: vacio no se guarda ni cambia nada.
        await s.renombrar("   ")
        XCTAssertNotNil(s.error)
        XCTAssertEqual(UserDefaults.standard.string(forKey: claveNombreDelAparato), "iPad de Max")
        s.parar()
    }
}

@MainActor
final class SincroAjustesIPadTests: XCTestCase {
    private var raiz: URL!
    private var suite: String!
    private var defaults: UserDefaults!

    override func setUp() async throws {
        raiz = try carpetaTemporalSincro("ajustes")
        suite = "pixpin-pruebas-\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: raiz)
        defaults.removePersistentDomain(forName: suite)
    }

    func testLosCambiosSeGuardanEnElTomlYSeDeshacen() async throws {
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        let m = ModeloAjustes(nucleo: nucleo, defaults: defaults)
        await m.cargar()
        XCTAssertTrue(m.cargado)
        m.fijar(diasCaducidad: 12)
        m.cambiar { $0.sincroPresencia = false }
        m.cambiarPdf { $0.aligerarAlEntrar = false }
        await m.guardar()
        XCTAssertNil(m.error)
        let leido = try nucleo.ajustes()
        XCTAssertEqual(leido.diasCaducidad, 12)
        XCTAssertFalse(leido.sincroPresencia)
        XCTAssertTrue(m.hayCambios)
        // Deshacer va hacia atras, uno a uno.
        m.deshacer()
        m.deshacer()
        await m.guardar()
        XCTAssertTrue(try nucleo.ajustes().sincroPresencia)
        XCTAssertEqual(try nucleo.ajustes().diasCaducidad, 12)
        // El rango es el de Windows.
        m.fijar(diasCaducidad: 500)
        XCTAssertEqual(m.diasCaducidad, 90)
        m.restablecer()
        await m.guardar()
        XCTAssertEqual(try nucleo.ajustes().diasCaducidad, m.fabrica.diasCaducidad)
    }

    func testLaSincronizacionSolaPasaAlSincronizador() async throws {
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        let s = Sincronizador(nucleo: nucleo)
        let m = ModeloAjustes(nucleo: nucleo, sincro: s, defaults: defaults)
        await m.cargar()
        m.fijar(sincroAutomatica: false)
        XCTAssertFalse(s.automatica)
        await m.guardar()
        // Otro arranque: el sincronizador lee el TOML al refrescar.
        let otro = Sincronizador(nucleo: nucleo)
        XCTAssertTrue(otro.automatica)
        await otro.refrescar()
        XCTAssertFalse(otro.automatica)
        otro.parar()
    }

    func testAparatosYAjustesUsanElMismoModelo() throws {
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "iPad")
        let a = ModeloAjustes.compartido(nucleo: nucleo, sincro: nil)
        XCTAssertTrue(a === ModeloAjustes.compartido(nucleo: nucleo, sincro: nil))
        let otroNucleo = try Nucleo.abrir(raiz: raiz.appendingPathComponent("otro").path, nombreDelAparato: "x")
        XCTAssertFalse(a === ModeloAjustes.compartido(nucleo: otroNucleo, sincro: nil))
    }

    func testElTemaSeGuardaYDeFabricaSigueAlSistema() {
        XCTAssertEqual(Tema.guardado(defaults), .sistema)
        XCTAssertNil(Tema.sistema.esquema)
        XCTAssertEqual(Tema.oscuro.esquema, .dark)
        XCTAssertEqual(Tema.claro.estilo, .light)
        Tema.oscuro.guardar(defaults)
        XCTAssertEqual(Tema.guardado(defaults), .oscuro)
        Tema.sistema.guardar(defaults)
        XCTAssertNil(defaults.string(forKey: "tema"))
    }

    func testElTextoDeUnErrorDelNucleoVaSinElNombreDelCaso() {
        XCTAssertEqual(ModeloAjustes.texto(de: ErrorNucleo.Ajustes(detalle: "mal")), "mal")
        XCTAssertEqual(ModeloAjustes.texto(de: ErrorNucleo.NoExiste(que: "x")), "no existe: x")
        XCTAssertFalse(ModeloAjustes.version.isEmpty)
    }
}
