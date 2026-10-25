// Los visores de planos y modelos 3D del iPad: la disposicion de los
// buferes de Rust en Swift, abrir un DXF y un fichero de puntos por el
// puente de verdad, la camara del plano con la y de UIKit, lo anotado con el
// lapiz en la capa y, si hay tarjeta (el simulador la tiene), compilar los
// sombreadores y pintar el plano fuera de pantalla.

import Foundation
import Metal
import PixPinCore
@testable import PixPinKit
import XCTest

/// El DXF de las pruebas de Rust (`planos.rs`): «Muros» con 0,50 mm (dos
/// rayas en L) y «Ejes» apagada (un circulo).
private let dxfDePrueba = "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n2\n0\nLAYER\n2\nMuros\n70\n0\n62\n1\n6\nCONTINUOUS\n370\n50\n0\nLAYER\n2\nEjes\n70\n0\n62\n-5\n6\nCONTINUOUS\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nMuros\n10\n0\n20\n0\n30\n0\n11\n100\n21\n0\n31\n0\n0\nLINE\n8\nMuros\n10\n100\n20\n0\n30\n0\n11\n100\n21\n60\n31\n0\n0\nCIRCLE\n8\nEjes\n10\n50\n20\n30\n30\n0\n40\n20\n0\nENDSEC\n0\nEOF\n"

/// Cinco puntos PNEZD (Civil 3D): un terreno con sus curvas.
private let puntosDePrueba = "1,100,200,10,A\n2,100,210,11,B\n3,110,200,12,C\n4,110,210,10,D\n5,105,205,15,E\n"

private func carpetaDePlanos(_ nombre: String) throws -> URL {
    let c = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-planos-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: c, withIntermediateDirectories: true)
    return c
}

private func abrirPlanoDePrueba(_ carpeta: URL) throws -> (Nucleo, Plano, URL) {
    let ruta = carpeta.appendingPathComponent("planta.dxf")
    try dxfDePrueba.write(to: ruta, atomically: true, encoding: .utf8)
    let nucleo = try Nucleo.abrir(raiz: carpeta.appendingPathComponent("datos").path, nombreDelAparato: "iPad de prueba")
    return (nucleo, try nucleo.abrirPlano(ruta: ruta.path), ruta)
}

final class PlanosDisposicionIPadTests: XCTestCase {
    func testLasEstructurasDeSwiftTienenElTamanoYLosDesplazamientosDeRust() {
        XCTAssertEqual(MemoryLayout<VerticeCAD>.stride, PasosCAD.vertice)
        XCTAssertEqual(MemoryLayout<VerticeCAD>.offset(of: \.color), 8)
        XCTAssertEqual(MemoryLayout<VerticeTramaCAD>.stride, PasosCAD.verticeTrama)
        XCTAssertEqual(MemoryLayout<TramaCAD>.stride, PasosCAD.trama)
        XCTAssertEqual(MemoryLayout<TramaCAD>.offset(of: \.desde), 28)
        XCTAssertEqual(MemoryLayout<FamiliaCAD>.stride, PasosCAD.familia)
        XCTAssertEqual(MemoryLayout<FamiliaCAD>.offset(of: \.trazos), 32)
        XCTAssertEqual(MemoryLayout<LetraCAD>.stride, PasosCAD.letra)
        XCTAssertEqual(MemoryLayout<ArcoCAD>.stride, PasosCAD.arco)
        XCTAssertEqual(MemoryLayout<TramoCAD>.stride, PasosCAD.tramo)
        XCTAssertEqual(MemoryLayout<TramoCAD>.offset(of: \.tamano), 24)
        XCTAssertEqual(MemoryLayout<V3CAD>.stride, PasosCAD.v3)
        XCTAssertEqual(MemoryLayout<V3CAD>.offset(of: \.elemento), 20)
    }

    func testLosUniformesTienenElTamanoDeLosSombreadores() {
        let g = GrupoPlano(indice: 0, capa: "0", grosor: -3, encendido: true, origenX: 0, origenY: 0,
                           caja: CajaPlano(x0: 0, y0: 0, x1: 1, y1: 1), bytes: 0)
        let u = PintorPlano.vista(CamaraPlano(centroX: 0, centroY: 0, px: 1), grupo: g, ancho: 100, alto: 100, oscuro: true)
        XCTAssertEqual(u.bytes, 48)
        let c = Camara3d(objetivo: Punto3d(x: 0, y: 0, z: 0), rumbo: -0.8, altura: 0.5, dist: 10)
        let u3 = VistaModeloMetal.vista(c, ancho: 800, alto: 600, radio: 10, corte: nil, elegido: 3, aislado: nil,
                                        claro: false, radioPuntos: 4)
        XCTAssertEqual(u3.bytes, 176)
        // El elegido va + 1 (0 es ninguno), tras la matriz y cinco float4.
        XCTAssertEqual(u3.palabras[36], 4)
        // Caso negativo: sin aislado, un 0.
        XCTAssertEqual(u3.palabras[40], 0)
    }
}

final class PlanosPuenteIPadTests: XCTestCase {
    func testUnDxfSeAbrePorCapasYSeEnciendenYApagan() throws {
        let carpeta = try carpetaDePlanos("capas")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let (_, plano, _) = try abrirPlanoDePrueba(carpeta)
        XCTAssertEqual(plano.capas().map(\.nombre), ["Ejes", "Muros"])
        XCTAssertFalse(plano.capas()[0].encendida, "Ejes esta apagada en el plano")
        let antes = Set(plano.gruposVisibles())
        XCTAssertTrue(plano.ponerCapa(nombre: "Ejes", encendida: true))
        XCTAssertGreaterThan(Set(plano.gruposVisibles()).count, antes.count)
        // Caso negativo: una capa que no existe no cambia nada.
        XCTAssertFalse(plano.ponerCapa(nombre: "No esta", encendida: true))
    }

    func testLaReglaSeEnganchaYLasCotasViajanEnLaCapa() throws {
        let carpeta = try carpetaDePlanos("cotas")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let (_, plano, _) = try abrirPlanoDePrueba(carpeta)
        let e = plano.enganchar(x: 100.3, y: 0.2, radio: 1, desde: nil)
        XCTAssertEqual(e.tipo, .punto)
        XCTAssertEqual(e.x, 100, accuracy: 1e-4)
        // Caso negativo: lejos de todo, libre.
        XCTAssertEqual(plano.enganchar(x: 40, y: 30, radio: 1, desde: nil).tipo, .libre)
        let cota = CotaPlano(puntos: [PuntoPlano(x: 0, y: 0), PuntoPlano(x: 100, y: 0)])
        XCTAssertTrue(try plano.guardarAnotado(anotado: AnotadoPlano(cotas: [cota], marcos: [])))
        XCTAssertEqual(plano.anotado().cotas.count, 1)
        XCTAssertGreaterThan(plano.encaje().u, 0)
    }

    @MainActor
    func testLoAnotadoConElLapizQuedaEnLaCapa() throws {
        let carpeta = try carpetaDePlanos("lapiz")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let (_, plano, _) = try abrirPlanoDePrueba(carpeta)
        let a = try AnotadorPlanoIPad(plano: plano)
        let vacio = a.leido?.figuras.count ?? 0
        a.puntero(.pulsar, x: 10, y: 10, presion: 0.5, zoom: 1)
        for i in 1 ... 10 { a.puntero(.mover, x: 10 + Double(i) * 5, y: 10 + Double(i) * 2, presion: 0.5, zoom: 1) }
        a.puntero(.soltar, x: 60, y: 30, presion: 0.5, zoom: 1)
        XCTAssertTrue(a.releer())
        XCTAssertGreaterThan(a.leido?.figuras.count ?? 0, vacio)
        XCTAssertTrue(a.puedeDeshacer)
        // Caso negativo: sin cambios no se vuelve a leer.
        XCTAssertFalse(a.releer())
        XCTAssertTrue(a.deshacer())
        XCTAssertEqual(a.leido?.figuras.count ?? 0, vacio)
    }

    func testUnFicheroDePuntosDaUnModelo3dQueSeEligeConUnRayo() throws {
        let carpeta = try carpetaDePlanos("puntos")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let csv = carpeta.appendingPathComponent("PNEZD.csv")
        try puntosDePrueba.write(to: csv, atomically: true, encoding: .utf8)
        let m = try Modelo3dApple.abrir(ruta: csv.path)
        let r = m.resumen()
        XCTAssertEqual(UInt64(leerEstructuras(m.vertices(), como: V3CAD.self).count), r.vertices)
        let c = r.caja
        let todo = Filtro3d(nivelesOcultos: [], tiposOcultos: [], aislado: nil)
        let e = m.elegir(origen: Punto3d(x: Double(c.x0 + c.x1) / 2, y: Double(c.y0 + c.y1) / 2, z: Double(c.z1) + 50),
                         direccion: Punto3d(x: 0, y: 0, z: -1), filtro: todo, corte: nil)
        XCTAssertNotNil(m.elemento(id: try XCTUnwrap(e).elemento))
        // Caso negativo: hacia arriba no hay nada.
        XCTAssertNil(m.elegir(origen: Punto3d(x: Double(c.x0 + c.x1) / 2, y: Double(c.y0 + c.y1) / 2, z: Double(c.z1) + 50),
                              direccion: Punto3d(x: 0, y: 0, z: 1), filtro: todo, corte: nil))
        XCTAssertThrowsError(try Modelo3dApple.abrir(ruta: carpeta.appendingPathComponent("no.ifc").path))
    }
}

final class PlanosCamaraIPadTests: XCTestCase {
    func testLaCamaraDelPlanoEncuadraYAcercaSinMoverElPuntoFijo() {
        let caja = CajaPlano(x0: 500_000, y0: 8_000_000, x1: 500_100, y1: 8_000_060)
        let c = CamaraPlano.encuadrar(caja, ancho: 2048, alto: 1536)
        let a = c.aPixel(PuntoPlano(x: caja.x0, y: caja.y0), ancho: 2048, alto: 1536)
        let b = c.aPixel(PuntoPlano(x: caja.x1, y: caja.y1), ancho: 2048, alto: 1536)
        XCTAssertTrue(a.x >= 0 && a.y >= 0 && b.x <= 2048 && b.y <= 1536, "\(a) \(b)")
        let p = c.aDibujo(x: 123, y: 456, ancho: 2048, alto: 1536)
        let z = c.zoom(2, x: 123, y: 456, ancho: 2048, alto: 1536)
        XCTAssertEqual(z.aDibujo(x: 123, y: 456, ancho: 2048, alto: 1536).x, p.x, accuracy: 1e-6)
        // Caso negativo: un factor imposible no cambia nada.
        XCTAssertEqual(c.zoom(.nan, x: 0, y: 0, ancho: 800, alto: 600), c)
    }

    /// La tinta: de la capa a la vista (y hacia abajo) y de vuelta, como
    /// hace el lapiz (`x = cero_x + x_capa * u`, `y = cero_y - y_capa * u`).
    func testLaTintaCaeDondeElLapizLaPuso() {
        let camara = CamaraPlano(centroX: 50, centroY: 30, px: 0.05)
        let e = EncajePlano(u: 0.1, ceroX: -20, ceroY: 90)
        let escala = 2.0, tamano = CGSize(width: 1000, height: 700)
        let s = camara.px * escala
        let t = PintorTintaPlano.transformacion(camara: camara, s: s, encaje: e, tamano: tamano)
        let capa = CGPoint(x: 600, y: 650)
        let enVista = capa.applying(t)
        // Lo mismo por la camara del plano, con la y de UIKit.
        let dibujo = PuntoPlano(x: e.ceroX + Double(capa.x) * e.u, y: e.ceroY - Double(capa.y) * e.u)
        let px = camara.aPixel(dibujo, ancho: Double(tamano.width) * escala, alto: Double(tamano.height) * escala)
        XCTAssertEqual(enVista.x, px.x / escala, accuracy: 1e-6)
        XCTAssertEqual(enVista.y, tamano.height - px.y / escala, accuracy: 1e-6)
        // Y de vuelta.
        let vuelta = enVista.applying(t.inverted())
        XCTAssertEqual(vuelta.x, capa.x, accuracy: 1e-6)
        XCTAssertEqual(vuelta.y, capa.y, accuracy: 1e-6)
    }

    func testLasHerramientasDelLapizSonLasDelLienzo() {
        XCTAssertEqual(HerramientaTintaPlano.allCases.map(\.rawValue), ["lapiz", "resaltador", "borrador"])
        for h in HerramientaTintaPlano.allCases {
            XCTAssertFalse(h.titulo.isEmpty)
            XCTAssertFalse(h.simbolo.isEmpty)
        }
    }
}

final class PlanosMetalIPadTests: XCTestCase {
    private func motor() throws -> MotorMetal {
        guard let m = MotorMetal(dispositivo: MTLCreateSystemDefaultDevice()) else {
            throw XCTSkip("Sin tarjeta con Metal")
        }
        return m
    }

    func testLosSombreadoresCompilan() throws {
        let m = try motor()
        let plano = try m.biblioteca(SombreadoresCAD.plano, clave: "plano")
        for f in SombreadoresCAD.funcionesPlano { XCTAssertNotNil(plano.makeFunction(name: f), f) }
        let modelo = try m.biblioteca(SombreadoresCAD.modelo, clave: "modelo")
        for f in SombreadoresCAD.funcionesModelo { XCTAssertNotNil(modelo.makeFunction(name: f), f) }
        try PintorPlano(motor: m, color: .bgra8Unorm, muestras: m.muestras).preparar()
        // Caso negativo: una funcion que no existe.
        XCTAssertThrowsError(try m.tuberia(fuente: SombreadoresCAD.plano, claveFuente: "plano", vertice: "vs_no_esta",
                                           fragmento: "ps_color", color: .bgra8Unorm, profundidad: .invalid,
                                           muestras: 1, mezcla: true))
    }

    func testElPlanoSePintaFueraDePantalla() throws {
        let m = try motor()
        let carpeta = try carpetaDePlanos("pintar")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ruta = carpeta.appendingPathComponent("planta.dxf")
        try dxfDePrueba.write(to: ruta, atomically: true, encoding: .utf8)
        let nucleo = try Nucleo.abrir(raiz: carpeta.appendingPathComponent("datos").path, nombreDelAparato: "iPad")
        let datos = try PlanoEnTarjeta.cargar(ruta: ruta.path, nucleo: nucleo, motor: m)
        XCTAssertEqual(datos.grupos.count, 2)

        let lado = 128
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: lado, height: lado, mipmapped: false)
        d.usage = [.renderTarget, .shaderRead]
        d.storageMode = .shared
        let tex = try XCTUnwrap(m.dispositivo.makeTexture(descriptor: d))
        let pasada = MTLRenderPassDescriptor()
        pasada.colorAttachments[0].texture = tex
        pasada.colorAttachments[0].loadAction = .clear
        pasada.colorAttachments[0].storeAction = .store
        pasada.colorAttachments[0].clearColor = MTLClearColor(red: 1, green: 1, blue: 1, alpha: 1)
        let cb = try XCTUnwrap(m.cola.makeCommandBuffer())
        let enc = try XCTUnwrap(cb.makeRenderCommandEncoder(descriptor: pasada))
        let camara = CamaraPlano.encuadrar(datos.resumen.cajaUtil, ancho: Double(lado), alto: Double(lado))
        try PintorPlano(motor: m, color: .bgra8Unorm, muestras: 1)
            .pintar(datos, visibles: Set(datos.plano.gruposVisibles()), camara: camara,
                    ancho: Double(lado), alto: Double(lado), oscuro: false, en: enc)
        enc.endEncoding()
        cb.commit()
        cb.waitUntilCompleted()
        XCTAssertNil(cb.error)
        var pixeles = [UInt8](repeating: 0, count: lado * lado * 4)
        tex.getBytes(&pixeles, bytesPerRow: lado * 4, from: MTLRegionMake2D(0, 0, lado, lado), mipmapLevel: 0)
        let pintados = stride(from: 0, to: pixeles.count, by: 4).filter { pixeles[$0] < 200 || pixeles[$0 + 1] < 200 }.count
        XCTAssertGreaterThan(pintados, 20)
    }
}
