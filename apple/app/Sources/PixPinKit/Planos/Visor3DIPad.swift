// El visor 3D del iPad (`ventana3d.rs` + `gpu3d.rs` de Windows,
// `VisorModelo3D.swift` del Mac) con Metal y el tacto: IFC, Revit, LandXML,
// puntos de Civil 3D, y un DWG/DXF visto en 3D.
//
// - Un dedo gira alrededor del punto tocado, dos dedos mueven, pellizcar
//   acerca hacia lo que hay entre los dedos, doble toque lo ve entero. La
//   camara es la de Windows (`camara*` de Rust: `Orbita`).
// - Un toque: que elemento es (un rayo de la camara contra los triangulos,
//   en Rust), resaltado, con sus propiedades; «Aislar» deja solo ese.
// - Niveles y categorias se encienden y apagan: solo se rehacen los indices.
// - Caja de seccion con sus tapas, como Revit: sus caras se mueven con las
//   asas (arrastrando) o con los deslizadores del panel.
//
// Como el visor de planos, la vista esta parada y pinta solo al cambiar.

import Metal
import MetalKit
import PixPinCore
import SwiftUI
import UIKit

/// El modelo en la tarjeta: los vertices (una vez) y los indices de lo que
/// se ve (se rehacen al filtrar).
final class ModeloEnTarjeta: @unchecked Sendable {
    let modelo: Modelo3dApple
    let resumen: ResumenModelo3d
    /// Lo que pide un croquis del movil; `nil` en un modelo de un fichero.
    let croquis: ExtrasCroquis?
    let vertices: MTLBuffer?
    private(set) var indices: [MTLBuffer?] = [nil, nil, nil, nil, nil]
    private(set) var cuantos: [Int] = [0, 0, 0, 0, 0]
    let motor: MotorMetal

    enum Clase: Int { case opacos, transparentes, aristas, lineas, puntos }

    init(modelo: Modelo3dApple, motor: MotorMetal) {
        self.modelo = modelo
        self.motor = motor
        resumen = modelo.resumen()
        croquis = modelo.extrasCroquis()
        vertices = motor.bufer(modelo.vertices(), nombre: "vertices 3d")
        poner(modelo.indices())
    }

    /// Sube unos indices nuevos (al filtrar).
    func poner(_ i: Indices3d) {
        let todos = [i.opacos, i.transparentes, i.aristas, i.lineas, i.puntos]
        indices = todos.map { motor.bufer($0, nombre: "indices 3d") }
        cuantos = todos.map { $0.count / 4 }
    }

    func bufer(_ c: Clase) -> (MTLBuffer, Int)? {
        guard let b = indices[c.rawValue], cuantos[c.rawValue] > 0 else { return nil }
        return (b, cuantos[c.rawValue])
    }

    /// Lee y sube con quien lo lee (bloquea: desde una `Task` aparte).
    static func cargar(_ abrir: () throws -> Modelo3dApple, motor: MotorMetal) throws -> ModeloEnTarjeta {
        ModeloEnTarjeta(modelo: try abrir(), motor: motor)
    }
}

/// **El estado del visor 3D**: el modelo, el filtro, lo elegido y la caja de
/// seccion (en fracciones de la caja del modelo, de 0 a 1).
@MainActor
final class VisorModelo3D: ObservableObject {
    @Published private(set) var datos: ModeloEnTarjeta?
    @Published private(set) var error: String?
    @Published private(set) var niveles: [Nivel3d] = []
    @Published private(set) var categorias: [Categoria3d] = []
    @Published private(set) var filtro = Filtro3d(nivelesOcultos: [], tiposOcultos: [], aislado: nil)
    @Published private(set) var elegido: InfoElemento3d?
    @Published var aristas = true { didSet { vista?.repintar() } }
    @Published var conCorte = false { didSet { vista?.repintar() } }
    /// La caja de seccion: minimo y maximo de cada eje, de 0 a 1.
    @Published var corte: [Double] = [0, 0, 0, 1, 1, 1] { didSet { vista?.repintar() } }
    /// El panel (lo elegido, la caja, niveles y categorias).
    @Published var verPanel = false
    /// Las vistas guardadas del croquis (1 a 9).
    @Published private(set) var vistas: [String] = []
    let titulo: String
    let ruta: String
    /// Quien lee el modelo (fuera del hilo principal).
    private let abrir: @Sendable () throws -> Modelo3dApple
    weak var vista: VistaModeloMetal?

    /// Con `nucleo`, el modelo leido queda en su cache (`cache/bim`, como
    /// Windows): la segunda vez se abre casi al momento.
    init(ruta: String, titulo: String, nucleo: Nucleo?) {
        self.ruta = ruta
        self.titulo = titulo
        if let nucleo {
            abrir = { try nucleo.abrirModelo(ruta: ruta) }
        } else {
            abrir = { try Modelo3dApple.abrir(ruta: ruta) }
        }
    }

    /// El fondo: claro salvo en el modo noche comun (o el del croquis).
    var claro: Bool { !ModoNoche.compartido.activo }

    func cargar() {
        guard datos == nil, error == nil else { return }
        guard let motor = MotorMetal.compartido else {
            error = "Este iPad no tiene una tarjeta gráfica con Metal."
            return
        }
        let abrir = self.abrir
        Task {
            let hecho = await Task.detached(priority: .userInitiated) { () -> Result<ModeloEnTarjeta, Error> in
                Result { try ModeloEnTarjeta.cargar(abrir, motor: motor) }
            }.value
            switch hecho {
            case let .success(d):
                self.datos = d
                self.niveles = d.modelo.niveles()
                self.categorias = d.modelo.categorias()
                self.vistas = d.modelo.vistasGuardadas()
                if d.resumen.vacio { self.error = "El modelo no tiene nada que se pueda ver." }
                self.vista?.alLlegarElModelo()
            case let .failure(e):
                self.error = "No se pudo abrir el modelo: \(e)"
            }
        }
    }

    /// La caja de seccion en coordenadas del modelo (o `nil` sin corte).
    var cajaDeCorte: Caja3d? {
        guard conCorte, let c = datos?.resumen.caja, corte.count == 6 else { return nil }
        func eje(_ a: Float, _ b: Float, _ t: Double) -> Float { a + (b - a) * Float(t) }
        return Caja3d(x0: eje(c.x0, c.x1, min(corte[0], corte[3])), y0: eje(c.y0, c.y1, min(corte[1], corte[4])),
                      z0: eje(c.z0, c.z1, min(corte[2], corte[5])), x1: eje(c.x0, c.x1, max(corte[0], corte[3])),
                      y1: eje(c.y0, c.y1, max(corte[1], corte[4])), z1: eje(c.z0, c.z1, max(corte[2], corte[5])))
    }

    // MARK: Filtros

    func verNivel(_ i: UInt32, _ ver: Bool) {
        var f = filtro
        f.nivelesOcultos.removeAll { $0 == i }
        if !ver { f.nivelesOcultos.append(i) }
        filtrar(f)
    }

    func verCategoria(_ tipo: String, _ ver: Bool) {
        var f = filtro
        f.tiposOcultos.removeAll { $0 == tipo }
        if !ver { f.tiposOcultos.append(tipo) }
        filtrar(f)
    }

    func aislar() {
        var f = filtro
        f.aislado = f.aislado == nil ? elegido?.id : nil
        filtrar(f)
    }

    func verTodo() { filtrar(Filtro3d(nivelesOcultos: [], tiposOcultos: [], aislado: nil)) }

    /// Rehace los indices fuera del hilo principal; los vertices se quedan.
    private func filtrar(_ f: Filtro3d) {
        guard let d = datos else { return }
        filtro = f
        Task {
            let i = await Task.detached(priority: .userInitiated) { d.modelo.visibles(filtro: f) }.value
            guard self.filtro == f else { return } // ya se pidio otro
            d.poner(i)
            self.vista?.repintar()
        }
    }

    // MARK: Elegir

    func elegir(_ e: Eleccion3d?) {
        elegido = e.flatMap { datos?.modelo.elemento(id: $0.elemento) }
        if elegido != nil { verPanel = true }
        vista?.repintar()
    }

    /// Suelta lo aislado; si no, lo elegido.
    func soltar() {
        if filtro.aislado != nil { aislar() } else if elegido != nil { elegir(nil) }
    }

    func encuadrar() { vista?.encuadrar() }
    func planta() { vista?.planta() }
    func irAVista(_ i: Int) { vista?.irAVista(i) }
}

// MARK: - La vista de Metal

final class VistaModeloMetal: MTKView, UIGestureRecognizerDelegate {
    let visor: VisorModelo3D
    private let motor: MotorMetal
    private let muestras: Int
    private(set) var camara = Camara3d(objetivo: Punto3d(x: 0, y: 0, z: 0), rumbo: -0.8, altura: 0.5, dist: 10)
    var radio = 10.0
    private var encuadrado = false
    let encima = CapaEncimaModelo()
    /// El punto sobre el que se gira mientras se arrastra (Windows lo ensena).
    var pivoteVisible: Punto3d?

    private let arrastre = UIPanGestureRecognizer()
    private let pellizco = UIPinchGestureRecognizer()
    private let toque = UITapGestureRecognizer()
    private let dobleToque = UITapGestureRecognizer()
    /// Girando: el pivote; o moviendo un asa.
    private var giro: Punto3d?
    private var asa: (cara: Int, p: CGPoint, corte: [Double])?
    private var ultimoArrastre: CGPoint?
    private var dedosArrastre = 0
    private var fijoPellizco: Punto3d?

    init(visor: VisorModelo3D, motor: MotorMetal) {
        self.visor = visor
        self.motor = motor
        muestras = motor.muestras
        super.init(frame: CGRect(x: 0, y: 0, width: 800, height: 600), device: motor.dispositivo)
        colorPixelFormat = .bgra8Unorm
        depthStencilPixelFormat = .depth32Float
        clearDepth = 1
        sampleCount = muestras
        isPaused = true
        enableSetNeedsDisplay = true
        autoResizeDrawable = true
        isMultipleTouchEnabled = true
        encima.vista = self
        encima.frame = bounds
        encima.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        addSubview(encima)

        arrastre.addTarget(self, action: #selector(alArrastrar(_:)))
        arrastre.maximumNumberOfTouches = 2
        pellizco.addTarget(self, action: #selector(alPellizcar(_:)))
        toque.addTarget(self, action: #selector(alTocar(_:)))
        dobleToque.addTarget(self, action: #selector(alDobleToque(_:)))
        dobleToque.numberOfTapsRequired = 2
        toque.require(toFail: dobleToque)
        for g in [arrastre, pellizco, toque, dobleToque] as [UIGestureRecognizer] {
            g.delegate = self
            addGestureRecognizer(g)
        }
        // Con raton o trackpad, la rueda mueve.
        arrastre.allowedScrollTypesMask = .continuous
        visor.vista = self
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    func repintar() {
        setNeedsDisplay()
        encima.setNeedsDisplay()
    }

    private func repintarYa() {
        draw()
        encima.setNeedsDisplay()
    }

    var escala: Double {
        bounds.width > 0 && drawableSize.width > 0 ? Double(drawableSize.width) / Double(bounds.width) : Double(contentScaleFactor)
    }

    var ancho: UInt32 { UInt32(max(drawableSize.width > 0 ? drawableSize.width : bounds.width * contentScaleFactor, 1)) }
    var alto: UInt32 { UInt32(max(drawableSize.height > 0 ? drawableSize.height : bounds.height * contentScaleFactor, 1)) }

    /// Claro u oscuro: el del croquis si lo dice; si no, el modo noche.
    var claro: Bool {
        if ModoNoche.compartido.activo { return false }
        return visor.datos?.croquis?.claro ?? visor.claro
    }

    func alLlegarElModelo() {
        guard let d = visor.datos else { return }
        let c = d.resumen.caja
        radio = max(Double(hypot(hypot(c.x1 - c.x0, c.y1 - c.y0), c.z1 - c.z0)) / 2, 1e-3)
        encuadrado = false
        encuadrar()
    }

    func encuadrar() {
        guard let d = visor.datos, bounds.width > 0 else { return }
        if let c = d.croquis, let caja = c.encuadre {
            camara = camaraEncuadrarDesde(caja: caja, ancho: ancho, alto: alto, rumbo: c.rumbo, altura: c.altura)
        } else {
            camara = camaraEncuadrar(caja: d.resumen.cajaUtil, ancho: ancho, alto: alto)
        }
        encuadrado = true
        repintar()
    }

    /// La planta (desde arriba), encuadrada.
    func planta() {
        guard let d = visor.datos else { return }
        let caja = visor.cajaDeCorte ?? d.croquis?.encuadre ?? d.resumen.cajaUtil
        camara = camaraEncuadrarDesde(caja: caja, ancho: ancho, alto: alto, rumbo: -Double.pi / 2, altura: 1.5697)
        encuadrado = true
        repintar()
    }

    /// La vista guardada `i` del croquis, como la mira el movil.
    func irAVista(_ i: Int) {
        guard i >= 0, let d = visor.datos,
              let c = d.modelo.camaraDeVista(indice: UInt32(i), ancho: ancho, alto: alto, escala: escala) else { return }
        camara = c
        encuadrado = true
        repintar()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        if !encuadrado { encuadrar() }
        repintar()
    }

    // MARK: Pintar

    /// Lo que el sombreador 3D sabe de la vista (`Vista3d`, 176 bytes).
    static func vista(_ c: Camara3d, ancho: UInt32, alto: UInt32, radio: Double, corte: Caja3d?,
                      elegido: UInt32?, aislado: UInt32?, claro: Bool, radioPuntos: Float) -> PalabrasUniforme {
        var u = PalabrasUniforme()
        for x in camaraMatriz(camara: c, ancho: ancho, alto: alto, radio: radio) { u.f(x) }
        let l = luzDeCamara(c)
        u.f4(l.x, l.y, l.z, 0)
        let o = camaraOjo(camara: c)
        u.f4(Float(o.x), Float(o.y), Float(o.z), 1)
        u.f4(corte == nil ? 0 : 1, 0, radioPuntos, 0)
        u.f4(corte?.x0 ?? 0, corte?.y0 ?? 0, corte?.z0 ?? 0, 0)
        u.f4(corte?.x1 ?? 0, corte?.y1 ?? 0, corte?.z1 ?? 0, 0)
        u.u(elegido.map { $0 + 1 } ?? 0)
        u.f(Float(claro ? 1 : 0))
        u.f(Float(ancho))
        u.f(Float(alto))
        u.u(aislado.map { $0 + 1 } ?? 0)
        u.f(Float(0))
        u.f(Float(0))
        u.f(Float(0))
        return u
    }

    private func tuberia(_ v: String, _ f: String, mezcla: Bool) throws -> MTLRenderPipelineState {
        try motor.tuberia(fuente: SombreadoresCAD.modelo, claveFuente: "modelo", vertice: v, fragmento: f,
                          color: colorPixelFormat, profundidad: depthStencilPixelFormat, muestras: muestras, mezcla: mezcla)
    }

    override func draw(_ rect: CGRect) {
        let claro = self.claro
        if let f = visor.datos?.croquis?.fondo, !ModoNoche.compartido.activo {
            clearColor = MTLClearColor(red: Double(f.r), green: Double(f.g), blue: Double(f.b), alpha: 1)
        } else {
            clearColor = claro ? MTLClearColor(red: 0.955, green: 0.958, blue: 0.965, alpha: 1)
                : MTLClearColor(red: 0.13, green: 0.15, blue: 0.19, alpha: 1)
        }
        guard let pasada = currentRenderPassDescriptor, let dibujable = currentDrawable,
              let cb = motor.cola.makeCommandBuffer(),
              let enc = cb.makeRenderCommandEncoder(descriptor: pasada) else { return }
        if let d = visor.datos, let vb = d.vertices {
            do {
                try pintar(d, vb, enc, claro: claro)
            } catch {
                NSLog("PixPin 3D: \(error)")
            }
        }
        enc.endEncoding()
        cb.present(dibujable)
        cb.commit()
    }

    private func pintar(_ d: ModeloEnTarjeta, _ vb: MTLBuffer, _ enc: MTLRenderCommandEncoder, claro: Bool) throws {
        let corte = visor.cajaDeCorte
        let u = VistaModeloMetal.vista(camara, ancho: ancho, alto: alto, radio: radio, corte: corte,
                                       elegido: visor.elegido?.id, aislado: visor.filtro.aislado, claro: claro,
                                       radioPuntos: Float(4.5 * escala))
        let largo = u.bytes
        u.palabras.withUnsafeBytes { b in
            if let base = b.baseAddress {
                enc.setVertexBytes(base, length: largo, index: 1)
                enc.setFragmentBytes(base, length: largo, index: 0)
            }
        }
        enc.setCullMode(.none)
        enc.setVertexBuffer(vb, offset: 0, index: 0)
        let escribe = motor.profundidad(escribe: true), lee = motor.profundidad(escribe: false)
        func dibujar(_ c: ModeloEnTarjeta.Clase, _ tipo: MTLPrimitiveType) {
            guard let par = d.bufer(c) else { return }
            let (ib, n) = par
            enc.drawIndexedPrimitives(type: tipo, indexCount: n, indexType: .uint32, indexBuffer: ib, indexBufferOffset: 0)
        }
        // 1. Lo opaco, un poco hacia atras para que las aristas queden encima.
        enc.setRenderPipelineState(try tuberia("vs_3d", "ps_3d", mezcla: false))
        enc.setDepthStencilState(escribe)
        enc.setDepthBias(800, slopeScale: 1.5, clamp: 0)
        dibujar(.opacos, .triangle)
        // 2. Las aristas, encima.
        enc.setDepthBias(0, slopeScale: 0, clamp: 0)
        if visor.aristas {
            enc.setRenderPipelineState(try tuberia("vs_3d", "ps_arista", mezcla: true))
            enc.setDepthStencilState(lee)
            dibujar(.aristas, .line)
        }
        // 2a. Las tapas de la caja de seccion.
        if corte != nil {
            enc.setRenderPipelineState(try tuberia("vs_3d", "ps_tapa", mezcla: false))
            enc.setDepthStencilState(escribe)
            enc.setDepthBias(800, slopeScale: 1.5, clamp: 0)
            dibujar(.opacos, .triangle)
            enc.setDepthBias(0, slopeScale: 0, clamp: 0)
        }
        // 2b. Las rayas sueltas (curvas de nivel, ejes) y los puntos.
        enc.setRenderPipelineState(try tuberia("vs_3d", "ps_linea", mezcla: true))
        enc.setDepthStencilState(escribe)
        dibujar(.lineas, .line)
        enc.setRenderPipelineState(try tuberia("vs_punto", "ps_punto", mezcla: false))
        dibujar(.puntos, .triangle)
        // 3. Los vidrios, al final y sin tapar.
        enc.setRenderPipelineState(try tuberia("vs_3d", "ps_vidrio", mezcla: true))
        enc.setDepthStencilState(lee)
        dibujar(.transparentes, .triangle)
    }

    // MARK: Gestos

    /// El pixel (desde arriba a la izquierda, como `camara_rayo`).
    private func pixel(_ p: CGPoint) -> (Double, Double) {
        (Double(p.x) * escala, Double(p.y) * escala)
    }

    /// Lo que hay bajo un punto de la vista.
    private func elegir(_ p: CGPoint) -> Eleccion3d? {
        guard let d = visor.datos else { return nil }
        let (x, y) = pixel(p)
        let r = camaraRayo(camara: camara, x: x, y: y, ancho: ancho, alto: alto)
        return d.modelo.elegir(origen: camaraOjo(camara: camara), direccion: r, filtro: visor.filtro, corte: visor.cajaDeCorte)
    }

    /// El punto bajo el dedo o, sin nada, el del rayo a la distancia del objetivo.
    private func puntoBajo(_ p: CGPoint) -> Punto3d {
        if let e = elegir(p) { return e.punto }
        let (x, y) = pixel(p)
        let r = camaraRayo(camara: camara, x: x, y: y, ancho: ancho, alto: alto)
        let o = camaraOjo(camara: camara)
        return Punto3d(x: o.x + r.x * camara.dist, y: o.y + r.y * camara.dist, z: o.z + r.z * camara.dist)
    }

    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otro: UIGestureRecognizer) -> Bool {
        (g === pellizco && otro === arrastre) || (g === arrastre && otro === pellizco)
    }

    /// Un dedo gira alrededor de lo tocado (o mueve un asa de la caja); dos
    /// dedos (o la rueda del trackpad) mueven. Un solo gesto para los dos:
    /// al poner o quitar un dedo cambia lo que hace, sin saltos.
    @objc private func alArrastrar(_ g: UIPanGestureRecognizer) {
        let t = g.translation(in: self)
        let dedos = g.numberOfTouches
        switch g.state {
        case .began:
            let inicio = CGPoint(x: g.location(in: self).x - t.x, y: g.location(in: self).y - t.y)
            if dedos == 1, let cara = asaBajo(inicio) {
                asa = (cara, inicio, visor.corte)
            }
            empezar(dedos: dedos, en: g.location(in: self), t)
        case .changed:
            if let a = asa {
                moverAsa(a, hasta: CGPoint(x: a.p.x + t.x, y: a.p.y + t.y))
                return
            }
            if dedos != dedosArrastre {
                empezar(dedos: dedos, en: g.location(in: self), t)
                return
            }
            let a = ultimoArrastre ?? t
            let dx = Double(t.x - a.x) * escala, dy = Double(t.y - a.y) * escala
            ultimoArrastre = t
            if dedos == 1, let pivote = giro {
                // Media vuelta por el lado corto de la vista, sea del tamano que sea.
                let k = Double.pi / Double(max(min(ancho, alto), 1))
                camara = camaraGirar(camara: camara, dRumbo: -dx * k, dAltura: dy * k, pivote: pivote)
                pivoteVisible = pivote
            } else {
                camara = camaraMover(camara: camara, dx: dx, dy: dy, alto: alto)
                pivoteVisible = nil
            }
            encuadrado = true
            repintarYa()
        default:
            giro = nil
            asa = nil
            ultimoArrastre = nil
            pivoteVisible = nil
            encima.setNeedsDisplay()
        }
    }

    private func empezar(dedos: Int, en p: CGPoint, _ t: CGPoint) {
        dedosArrastre = dedos
        ultimoArrastre = t
        giro = dedos == 1 ? puntoBajo(p) : nil
    }

    /// Pellizcar: acercar hacia lo que hay entre los dedos.
    @objc private func alPellizcar(_ g: UIPinchGestureRecognizer) {
        switch g.state {
        case .began:
            fijoPellizco = puntoBajo(g.location(in: self))
            fallthrough
        case .changed:
            guard g.scale > 0, let fijo = fijoPellizco else { return }
            camara = camaraZoom(camara: camara, factor: 1 / Double(g.scale), fijo: fijo)
            g.scale = 1
            encuadrado = true
            repintarYa()
        default:
            fijoPellizco = nil
        }
    }

    @objc private func alTocar(_ g: UITapGestureRecognizer) {
        visor.elegir(elegir(g.location(in: self)))
    }

    @objc private func alDobleToque(_ g: UITapGestureRecognizer) { encuadrar() }

    // MARK: La caja de seccion en la pantalla

    /// Un punto del modelo en la vista (puntos, y hacia abajo); `nil` si
    /// queda detras del ojo.
    func aPantalla(_ p: Punto3d) -> CGPoint? {
        let m = camaraMatriz(camara: camara, ancho: ancho, alto: alto, radio: radio)
        guard m.count == 16 else { return nil }
        let v = [p.x, p.y, p.z, 1.0]
        var c = [0.0, 0.0, 0.0, 0.0]
        for i in 0 ..< 4 { for j in 0 ..< 4 { c[i] += Double(m[j * 4 + i]) * v[j] } }
        guard c[3] > 1e-9 else { return nil }
        return CGPoint(x: (c[0] / c[3] + 1) / 2 * bounds.width, y: (1 - (c[1] / c[3] + 1) / 2) * bounds.height)
    }

    /// Las seis caras de la caja de seccion: su centro, su eje y si es la
    /// de abajo de ese eje.
    func carasDelCorte() -> [(centro: Punto3d, eje: Int, minimo: Bool)] {
        guard let k = visor.cajaDeCorte else { return [] }
        let cx = Double(k.x0 + k.x1) / 2, cy = Double(k.y0 + k.y1) / 2, cz = Double(k.z0 + k.z1) / 2
        return [
            (Punto3d(x: Double(k.x0), y: cy, z: cz), 0, true), (Punto3d(x: cx, y: Double(k.y0), z: cz), 1, true),
            (Punto3d(x: cx, y: cy, z: Double(k.z0)), 2, true), (Punto3d(x: Double(k.x1), y: cy, z: cz), 0, false),
            (Punto3d(x: cx, y: Double(k.y1), z: cz), 1, false), (Punto3d(x: cx, y: cy, z: Double(k.z1)), 2, false),
        ]
    }

    /// Lo que mide la caja del modelo en el eje `e`.
    private func largo(_ e: Int) -> Double {
        guard let c = visor.datos?.resumen.caja else { return 1 }
        return max([Double(c.x1 - c.x0), Double(c.y1 - c.y0), Double(c.z1 - c.z0)][e], 1e-6)
    }

    /// La flecha de una cara en la pantalla: hacia donde crece su eje
    /// (puntos por unidad del modelo).
    func flecha(_ cara: (centro: Punto3d, eje: Int, minimo: Bool)) -> CGVector? {
        let l = largo(cara.eje) * 0.1
        var q = cara.centro
        switch cara.eje { case 0: q.x += l; case 1: q.y += l; default: q.z += l }
        guard let a = aPantalla(cara.centro), let b = aPantalla(q) else { return nil }
        return CGVector(dx: (b.x - a.x) / l, dy: (b.y - a.y) / l)
    }

    /// El asa bajo `p` (su numero de cara), si la caja esta puesta (a un
    /// dedo de distancia).
    func asaBajo(_ p: CGPoint) -> Int? {
        var mejor: (Int, CGFloat)?
        for (i, c) in carasDelCorte().enumerated() {
            guard let q = aPantalla(c.centro) else { continue }
            let d = hypot(q.x - p.x, q.y - p.y)
            if d < 24, d < (mejor?.1 ?? .infinity) { mejor = (i, d) }
        }
        return mejor?.0
    }

    /// Arrastrando un asa: su cara sigue al dedo a lo largo de su eje.
    private func moverAsa(_ a: (cara: Int, p: CGPoint, corte: [Double]), hasta p: CGPoint) {
        let caras = carasDelCorte()
        guard a.cara < caras.count, let f = flecha(caras[a.cara]) else { return }
        let n2 = f.dx * f.dx + f.dy * f.dy
        guard n2 > 1e-12 else { return }
        let unidades = (Double(p.x - a.p.x) * f.dx + Double(p.y - a.p.y) * f.dy) / n2
        let e = caras[a.cara].eje
        let delta = unidades / largo(e)
        var c = a.corte
        let (lo, hi) = (min(c[e], c[e + 3]), max(c[e], c[e + 3]))
        if caras[a.cara].minimo {
            c[e] = min(max(lo + delta, 0), hi - 0.01); c[e + 3] = hi
        } else {
            c[e + 3] = max(min(hi + delta, 1), lo + 0.01); c[e] = lo
        }
        visor.corte = c
    }
}

/// Pinta el punto de giro y la caja de seccion con sus asas. No recibe los
/// toques: los atiende la vista de Metal.
final class CapaEncimaModelo: UIView {
    weak var vista: VistaModeloMetal?

    static let azul = UIColor(red: 0, green: 0x78 / 255.0, blue: 0xD4 / 255.0, alpha: 1)

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        isUserInteractionEnabled = false
        contentMode = .redraw
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override func draw(_ rect: CGRect) {
        guard let v = vista, let ctx = UIGraphicsGetCurrentContext() else { return }
        if v.visor.conCorte, let k = v.visor.cajaDeCorte { caja(v, k, ctx) }
        if let p = v.pivoteVisible, let q = v.aPantalla(p) {
            ctx.setFillColor(UIColor.white.cgColor)
            ctx.fillEllipse(in: CGRect(x: q.x - 6, y: q.y - 6, width: 12, height: 12))
            ctx.setFillColor(UIColor.systemOrange.cgColor)
            ctx.fillEllipse(in: CGRect(x: q.x - 4, y: q.y - 4, width: 8, height: 8))
        }
    }

    private func caja(_ v: VistaModeloMetal, _ k: Caja3d, _ ctx: CGContext) {
        let xs = [Double(k.x0), Double(k.x1)], ys = [Double(k.y0), Double(k.y1)], zs = [Double(k.z0), Double(k.z1)]
        func p(_ i: Int, _ j: Int, _ l: Int) -> CGPoint? { v.aPantalla(Punto3d(x: xs[i], y: ys[j], z: zs[l])) }
        var aristas: [(CGPoint?, CGPoint?)] = []
        for a in 0 ..< 2 {
            for b in 0 ..< 2 {
                aristas.append((p(0, a, b), p(1, a, b)))
                aristas.append((p(a, 0, b), p(a, 1, b)))
                aristas.append((p(a, b, 0), p(a, b, 1)))
            }
        }
        ctx.setStrokeColor(CapaEncimaModelo.azul.withAlphaComponent(0.85).cgColor)
        ctx.setLineWidth(1.2)
        for case let (a?, b?) in aristas {
            ctx.move(to: a)
            ctx.addLine(to: b)
        }
        ctx.strokePath()
        // Un asa por cara: el punto y sus dos flechas a lo largo del eje.
        for c in v.carasDelCorte() {
            guard let q = v.aPantalla(c.centro), let f = v.flecha(c) else { continue }
            let n = max(hypot(f.dx, f.dy), 1e-9)
            let d = CGVector(dx: f.dx / n, dy: f.dy / n)
            ctx.setStrokeColor(CapaEncimaModelo.azul.cgColor)
            ctx.setLineWidth(1.8)
            for s in [-1.0, 1.0] {
                let punta = CGPoint(x: q.x + d.dx * 22 * s, y: q.y + d.dy * 22 * s)
                ctx.move(to: CGPoint(x: q.x + d.dx * 10 * s, y: q.y + d.dy * 10 * s))
                ctx.addLine(to: punta)
                let ax = -d.dy, ay = d.dx
                ctx.move(to: punta)
                ctx.addLine(to: CGPoint(x: punta.x - d.dx * 6 * s + ax * 5, y: punta.y - d.dy * 6 * s + ay * 5))
                ctx.move(to: punta)
                ctx.addLine(to: CGPoint(x: punta.x - d.dx * 6 * s - ax * 5, y: punta.y - d.dy * 6 * s - ay * 5))
            }
            ctx.strokePath()
            ctx.setFillColor(UIColor.white.cgColor)
            ctx.fillEllipse(in: CGRect(x: q.x - 8, y: q.y - 8, width: 16, height: 16))
            ctx.setFillColor(CapaEncimaModelo.azul.cgColor)
            ctx.fillEllipse(in: CGRect(x: q.x - 6, y: q.y - 6, width: 12, height: 12))
        }
    }
}

// MARK: - SwiftUI

struct LienzoMetalModelo: UIViewRepresentable {
    let visor: VisorModelo3D
    let motor: MotorMetal
    /// Solo para que SwiftUI repinte al cambiar el modo noche.
    let oscuro: Bool

    func makeUIView(context _: Context) -> VistaModeloMetal { VistaModeloMetal(visor: visor, motor: motor) }
    func updateUIView(_ v: VistaModeloMetal, context _: Context) { v.repintar() }
}

/// **El visor de un modelo 3D** (IFC, Revit, LandXML, puntos...) a pantalla
/// completa: la barra comun arriba, el modelo en Metal y el panel flotante
/// (lo elegido, la caja de seccion, niveles y categorias).
public struct Visor3DIPad: View {
    let url: URL
    let titulo: String
    let nucleo: Nucleo
    @StateObject private var visor: VisorModelo3D
    @ObservedObject private var noche = ModoNoche.compartido

    public init(url: URL, titulo: String, nucleo: Nucleo) {
        self.url = url
        self.titulo = titulo
        self.nucleo = nucleo
        _visor = StateObject(wrappedValue: VisorModelo3D(ruta: url.path, titulo: titulo, nucleo: nucleo))
    }

    public var body: some View {
        ZStack(alignment: .top) {
            Color(white: noche.activo ? 0.15 : 0.96).ignoresSafeArea()
            contenido.ignoresSafeArea()
            BarraVisorIPad(tipo: "Modelo 3D", icono: "cube", titulo: titulo) { botones }
            if visor.verPanel && visor.datos != nil {
                PanelFlotantePlano(alto: 520) { panel }
            }
        }
        .onAppear { visor.cargar() }
        .onChange(of: noche.activo) { _, _ in visor.vista?.repintar() }
    }

    @ViewBuilder private var botones: some View {
        Group {
            Button { visor.encuadrar() } label: { Image(systemName: "viewfinder") }
                .keyboardShortcut("f", modifiers: [])
                .help("Ver el modelo entero (F)")
            Button { visor.planta() } label: { Image(systemName: "square.grid.3x3.topleft.filled") }
                .keyboardShortcut("p", modifiers: [])
                .help("Planta: desde arriba (P)")
            Button { visor.aristas.toggle() } label: { Image(systemName: "cube.transparent") }
                .foregroundStyle(visor.aristas ? Paleta.v2.acento : Color.primary)
                .keyboardShortcut("e", modifiers: [])
                .help("Aristas (E)")
            Button { visor.conCorte.toggle() } label: { Image(systemName: "square.split.bottomrightquarter") }
                .foregroundStyle(visor.conCorte ? Paleta.v2.acento : Color.primary)
                .keyboardShortcut("c", modifiers: [])
                .help("Caja de sección (C)")
            if visor.elegido != nil || visor.filtro.aislado != nil {
                Button { visor.aislar() } label: { Image(systemName: "scope") }
                    .foregroundStyle(visor.filtro.aislado != nil ? Paleta.v2.acento : Color.primary)
                    .keyboardShortcut("i", modifiers: [])
                    .help(visor.filtro.aislado == nil ? "Aislar lo elegido (I)" : "Ver todo (I)")
            }
            if !visor.vistas.isEmpty {
                Menu {
                    ForEach(Array(visor.vistas.prefix(9).enumerated()), id: \.offset) { i, n in
                        Button("\(i + 1)  \(n)") { visor.irAVista(i) }
                    }
                } label: { Image(systemName: "camera.viewfinder") }
                    .help("Las vistas guardadas en el móvil")
            }
            Button { visor.verPanel.toggle() } label: { Image(systemName: "sidebar.right") }
                .foregroundStyle(visor.verPanel ? Paleta.v2.acento : Color.primary)
                .keyboardShortcut("l", modifiers: [])
                .help("Niveles y categorías (L)")
        }
        .disabled(visor.datos == nil)
    }

    @ViewBuilder private var contenido: some View {
        if let error = visor.error {
            ContentUnavailableView("No se puede ver el modelo", systemImage: "exclamationmark.triangle",
                                   description: Text(error))
        } else if let motor = MotorMetal.compartido {
            ZStack {
                LienzoMetalModelo(visor: visor, motor: motor, oscuro: noche.activo)
                if visor.datos == nil { ProgressView("Leyendo el modelo…") }
            }
        } else {
            ContentUnavailableView("Sin Metal", systemImage: "cpu",
                                   description: Text("Este iPad no tiene una tarjeta gráfica con Metal."))
        }
    }

    private var panel: some View {
        List {
            if let e = visor.elegido {
                Section {
                    LabeledContent("Categoría", value: e.categoria)
                    if !e.nombre.isEmpty { LabeledContent("Nombre", value: e.nombre) }
                    if let n = e.nivel { LabeledContent("Nivel", value: n) }
                    LabeledContent("Clase", value: e.tipo)
                    if e.volumen > 0 {
                        LabeledContent(e.cerrado ? "Volumen" : "Volumen (aprox.)", value: String(format: "%.3f", e.volumen))
                    }
                    if e.planta > 0 { LabeledContent("Área en planta", value: String(format: "%.3f", e.planta)) }
                    if e.alzado > 0 { LabeledContent("Área en alzado", value: String(format: "%.3f", e.alzado)) }
                    LabeledContent("Superficie", value: String(format: "%.3f", e.superficie))
                    Button(visor.filtro.aislado == nil ? "Aislar" : "Ver todo") { visor.aislar() }
                } header: {
                    HStack {
                        Text("Elegido")
                        Spacer()
                        Button { visor.elegir(nil) } label: { Image(systemName: "xmark.circle.fill") }
                            .buttonStyle(.borderless)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            if visor.conCorte {
                Section("Caja de sección") {
                    ForEach(0 ..< 3, id: \.self) { k in
                        let eje = ["X", "Y", "Z"][k]
                        LabeledContent("\(eje) desde") { Slider(value: $visor.corte[k], in: 0 ... 1) }
                        LabeledContent("\(eje) hasta") { Slider(value: $visor.corte[k + 3], in: 0 ... 1) }
                    }
                }
            }
            if !visor.niveles.isEmpty {
                Section("Niveles") {
                    ForEach(visor.niveles, id: \.indice) { n in
                        Toggle(isOn: Binding(get: { !visor.filtro.nivelesOcultos.contains(n.indice) },
                                             set: { visor.verNivel(n.indice, $0) })) {
                            Text("\(n.nombre) · \(String(format: "%.2f", n.cota)) m").lineLimit(1)
                        }
                    }
                }
            }
            if !visor.categorias.isEmpty {
                Section("Categorías") {
                    ForEach(visor.categorias, id: \.tipo) { c in
                        Toggle(isOn: Binding(get: { !visor.filtro.tiposOcultos.contains(c.tipo) },
                                             set: { visor.verCategoria(c.tipo, $0) })) {
                            Text("\(c.nombre) (\(c.elementos))").lineLimit(1)
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
    }
}
