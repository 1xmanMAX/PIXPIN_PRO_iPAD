// El plano en la tarjeta y como se pinta (`gpu.rs` de Windows con Metal):
// se sube una vez y cada fotograma solo cambia la vista (48 bytes por
// grupo). Se dibujan los tramos que caen en la ventana y que de tamano se
// verian; lo demas ni se toca.
//
// Cinco pasadas, todas con antialias (MSAA 4x): rellenos, sombreados con
// patron (sus rayas pixel a pixel), rayas, arcos y letras. Cada pasada
// recorre todos los grupos que se ven: asi las rayas de una capa quedan
// encima de los rellenos de otra, como en el plano entero de Windows.

import Foundation
import Metal
import PixPinCore

/// Un grupo del plano (una capa con un grosor) en la tarjeta.
public final class GrupoMetal: @unchecked Sendable {
    public let ficha: GrupoPlano
    let vertices: MTLBuffer?
    let lineas: MTLBuffer?
    let triangulos: MTLBuffer?
    let verticesTrama: MTLBuffer?
    let triangulosTrama: MTLBuffer?
    let tramas: MTLBuffer?
    let familias: MTLBuffer?
    let malla: MTLBuffer?
    let glifos: MTLBuffer?
    let letras: MTLBuffer?
    let arcos: MTLBuffer?
    let tramosLineas: [TramoCAD]
    let tramosTriangulos: [TramoCAD]
    let tramosTrama: [TramoCAD]
    let tramosLetras: [TramoCAD]
    let tramosArcos: [TramoCAD]

    init(ficha: GrupoPlano, b: BuferesGrupo, motor: MotorMetal) {
        self.ficha = ficha
        let n = "\(ficha.capa)/\(ficha.grosor)"
        vertices = motor.bufer(b.vertices, nombre: n + " vertices")
        lineas = motor.bufer(b.lineas, nombre: n + " lineas")
        triangulos = motor.bufer(b.triangulos, nombre: n + " triangulos")
        verticesTrama = motor.bufer(b.verticesTrama, nombre: n + " vertices trama")
        triangulosTrama = motor.bufer(b.triangulosTrama, nombre: n + " triangulos trama")
        tramas = motor.bufer(b.tramas, nombre: n + " tramas")
        familias = motor.bufer(b.familias, nombre: n + " familias")
        malla = motor.bufer(b.mallaLetras, nombre: n + " malla")
        glifos = motor.bufer(b.glifos, nombre: n + " glifos")
        letras = motor.bufer(b.letras, nombre: n + " letras")
        arcos = motor.bufer(b.arcos, nombre: n + " arcos")
        tramosLineas = leerEstructuras(b.tramosLineas, como: TramoCAD.self)
        tramosTriangulos = leerEstructuras(b.tramosTriangulos, como: TramoCAD.self)
        tramosTrama = leerEstructuras(b.tramosTrama, como: TramoCAD.self)
        tramosLetras = leerEstructuras(b.tramosLetras, como: TramoCAD.self)
        tramosArcos = leerEstructuras(b.tramosArcos, como: TramoCAD.self)
    }

    /// Lo que se ve de este grupo, en sus coordenadas.
    func caja(_ c: CamaraPlano, ancho: Double, alto: Double) -> CajaF {
        let cx = c.centroX - ficha.origenX, cy = c.centroY - ficha.origenY
        let mx = ancho / 2 * c.px, my = alto / 2 * c.px
        return CajaF(x0: Float(cx - mx), y0: Float(cy - my), x1: Float(cx + mx), y1: Float(cy + my))
    }
}

/// **Un plano abierto**: lo de Rust (la regla, las capas, lo anotado) y lo
/// de la tarjeta.
public final class PlanoEnTarjeta: @unchecked Sendable {
    public let plano: Plano
    public let grupos: [GrupoMetal]
    public let resumen: ResumenPlano
    public let ruta: String

    init(plano: Plano, grupos: [GrupoMetal], resumen: ResumenPlano, ruta: String) {
        self.plano = plano
        self.grupos = grupos
        self.resumen = resumen
        self.ruta = ruta
    }

    /// **Lee el plano y lo sube** (bloquea: desde una `Task` aparte). Cada
    /// grupo cruza el puente una vez; ya en la tarjeta, Rust suelta lo que
    /// solo servia para dibujar.
    public static func cargar(ruta: String, nucleo: Nucleo, motor: MotorMetal) throws -> PlanoEnTarjeta {
        let plano = try nucleo.abrirPlano(ruta: ruta)
        var grupos: [GrupoMetal] = []
        for g in plano.grupos() {
            let b = try plano.buferes(indice: g.indice)
            grupos.append(GrupoMetal(ficha: g, b: b, motor: motor))
        }
        plano.soltarDibujo()
        return PlanoEnTarjeta(plano: plano, grupos: grupos, resumen: plano.resumen(), ruta: ruta)
    }
}

/// **Pinta un plano** en una pasada de Metal ya abierta.
struct PintorPlano {
    let motor: MotorMetal
    let color: MTLPixelFormat
    let muestras: Int

    private func tuberia(_ v: String, _ f: String) throws -> MTLRenderPipelineState {
        try motor.tuberia(fuente: SombreadoresCAD.plano, claveFuente: "plano", vertice: v, fragmento: f,
                          color: color, profundidad: .invalid, muestras: muestras, mezcla: true)
    }

    /// Prepara las tuberias (la primera vez compila los sombreadores).
    func preparar() throws {
        for (v, f) in [("vs_simple", "ps_relleno"), ("vs_trama", "ps_trama"), ("vs_simple", "ps_color"),
                       ("vs_arco", "ps_arco"), ("vs_letra", "ps_color")] {
            _ = try tuberia(v, f)
        }
    }

    /// Lo que el sombreador sabe de la vista, para un grupo.
    static func vista(_ c: CamaraPlano, grupo: GrupoPlano, ancho: Double, alto: Double, oscuro: Bool) -> PalabrasUniforme {
        var u = PalabrasUniforme()
        u.f(2 / (ancho * c.px))
        u.f(2 / (alto * c.px))
        u.f(c.centroX - grupo.origenX)
        u.f(c.centroY - grupo.origenY)
        if oscuro { u.f4(1, 1, 1, 1) } else { u.f4(0, 0, 0, 1) }
        u.f(c.px)
        u.f(Float(oscuro ? 1 : 2))
        u.f(Float(0))
        u.f(Float(0))
        return u
    }

    func pintar(_ p: PlanoEnTarjeta, visibles: Set<UInt32>, camara c: CamaraPlano, ancho: Double, alto: Double,
                oscuro: Bool, en enc: MTLRenderCommandEncoder) throws {
        let grupos = p.grupos.filter { visibles.contains($0.ficha.indice) }
        guard !grupos.isEmpty, ancho > 0, alto > 0 else { return }
        enc.setCullMode(.none)
        let px = Float(c.px)
        func conVista(_ g: GrupoMetal, _ hacer: (CajaF) -> Void) {
            let u = PintorPlano.vista(c, grupo: g.ficha, ancho: ancho, alto: alto, oscuro: oscuro)
            let largo = u.bytes
            u.palabras.withUnsafeBytes { b in
                if let base = b.baseAddress {
                    enc.setVertexBytes(base, length: largo, index: 1)
                    enc.setFragmentBytes(base, length: largo, index: 0)
                }
            }
            hacer(g.caja(c, ancho: ancho, alto: alto))
        }
        // 1. Rellenos.
        enc.setRenderPipelineState(try tuberia("vs_simple", "ps_relleno"))
        for g in grupos {
            guard let vb = g.vertices, let ib = g.triangulos else { continue }
            conVista(g) { caja in
                enc.setVertexBuffer(vb, offset: 0, index: 0)
                for (d, n) in tramosVisibles(g.tramosTriangulos, en: caja, minimo: px) {
                    enc.drawIndexedPrimitives(type: .triangle, indexCount: Int(n), indexType: .uint32,
                                              indexBuffer: ib, indexBufferOffset: Int(d) * 4)
                }
            }
        }
        // 2. Sombreados con patron.
        enc.setRenderPipelineState(try tuberia("vs_trama", "ps_trama"))
        for g in grupos {
            guard let vb = g.verticesTrama, let ib = g.triangulosTrama, let tr = g.tramas, let fa = g.familias else { continue }
            conVista(g) { caja in
                enc.setVertexBuffer(vb, offset: 0, index: 0)
                enc.setFragmentBuffer(tr, offset: 0, index: 1)
                enc.setFragmentBuffer(fa, offset: 0, index: 2)
                for (d, n) in tramosVisibles(g.tramosTrama, en: caja, minimo: px * 2) {
                    enc.drawIndexedPrimitives(type: .triangle, indexCount: Int(n), indexType: .uint32,
                                              indexBuffer: ib, indexBufferOffset: Int(d) * 4)
                }
            }
        }
        // 3. Rayas: tiras cortadas por 0xFFFFFFFF (Metal reinicia la tira).
        enc.setRenderPipelineState(try tuberia("vs_simple", "ps_color"))
        for g in grupos {
            guard let vb = g.vertices, let ib = g.lineas else { continue }
            conVista(g) { caja in
                enc.setVertexBuffer(vb, offset: 0, index: 0)
                for (d, n) in tramosVisibles(g.tramosLineas, en: caja, minimo: px * 0.75) {
                    enc.drawIndexedPrimitives(type: .lineStrip, indexCount: Int(n), indexType: .uint32,
                                              indexBuffer: ib, indexBufferOffset: Int(d) * 4)
                }
            }
        }
        // 3b. Circulos y arcos: un cuadrado (6 vertices) por arco.
        enc.setRenderPipelineState(try tuberia("vs_arco", "ps_arco"))
        for g in grupos {
            guard let ab = g.arcos else { continue }
            conVista(g) { caja in
                enc.setVertexBuffer(ab, offset: 0, index: 0)
                for (d, n) in tramosVisibles(g.tramosArcos, en: caja, minimo: px * 0.75) {
                    enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6,
                                       instanceCount: Int(n), baseInstance: Int(d))
                }
            }
        }
        // 4. Letras: un texto de menos de 3 pixeles no se lee.
        enc.setRenderPipelineState(try tuberia("vs_letra", "ps_color"))
        for g in grupos {
            guard let lb = g.letras, let ma = g.malla, let gl = g.glifos else { continue }
            conVista(g) { caja in
                enc.setVertexBuffer(lb, offset: 0, index: 0)
                enc.setVertexBuffer(ma, offset: 0, index: 2)
                enc.setVertexBuffer(gl, offset: 0, index: 3)
                for t in g.tramosLetras where t.tamano >= px * 3 && caja.corta(t) {
                    // Las de una fuente SHX son rayas; las TrueType, triangulos.
                    let rayas = t.clase & PasosCAD.glifoDeRayas != 0
                    let n = Int(t.clase & ~PasosCAD.glifoDeRayas)
                    guard n > 0 else { continue }
                    enc.drawPrimitives(type: rayas ? .line : .triangle, vertexStart: 0, vertexCount: n,
                                       instanceCount: Int(t.cuantos), baseInstance: Int(t.desde))
                }
            }
        }
    }
}
