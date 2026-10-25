// La tarjeta grafica de los visores de planos y modelos: un dispositivo y
// una cola para toda la app, los sombreadores compilados una vez desde su
// texto y las tuberias guardadas por clave (son pocas: un tope no hace
// falta, pero se cuentan).
//
// `MTLCreateSystemDefaultDevice()` puede dar `nil` (una maquina virtual de
// la CI sin tarjeta): entonces `MotorMetal.compartido` es `nil` y los
// visores lo dicen en vez de pintar.

import Foundation
import Metal

/// Lo que puede fallar al preparar la tarjeta.
public enum ErrorMotorMetal: Error, Equatable {
    case sinFuncion(String)
    case tuberia(String)
}

public final class MotorMetal: @unchecked Sendable {
    /// El de la app, o `nil` si no hay tarjeta.
    public static let compartido: MotorMetal? = MotorMetal()

    public let dispositivo: MTLDevice
    public let cola: MTLCommandQueue
    private let cerrojo = NSLock()
    private var bibliotecas: [String: MTLLibrary] = [:]
    private var tuberias: [String: MTLRenderPipelineState] = [:]
    private var profundidades: [String: MTLDepthStencilState] = [:]

    /// `nil` sin tarjeta.
    public init?(dispositivo: MTLDevice? = MTLCreateSystemDefaultDevice()) {
        guard let d = dispositivo, let q = d.makeCommandQueue() else { return nil }
        self.dispositivo = d
        cola = q
        q.label = "PixPin planos"
    }

    /// Cuantas tuberias hay guardadas.
    public var cuantasTuberias: Int {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return tuberias.count
    }

    /// La biblioteca de `fuente`, compilada la primera vez.
    public func biblioteca(_ fuente: String, clave: String) throws -> MTLLibrary {
        cerrojo.lock()
        if let b = bibliotecas[clave] {
            cerrojo.unlock()
            return b
        }
        cerrojo.unlock()
        let b = try dispositivo.makeLibrary(source: fuente, options: MTLCompileOptions())
        cerrojo.lock()
        bibliotecas[clave] = b
        cerrojo.unlock()
        return b
    }

    /// **Una tuberia**, hecha la primera vez y guardada por todo lo que la
    /// distingue.
    public func tuberia(fuente: String, claveFuente: String, vertice: String, fragmento: String,
                        color: MTLPixelFormat, profundidad: MTLPixelFormat, muestras: Int,
                        mezcla: Bool) throws -> MTLRenderPipelineState {
        let clave = "\(claveFuente)|\(vertice)|\(fragmento)|\(color.rawValue)|\(profundidad.rawValue)|\(muestras)|\(mezcla)"
        cerrojo.lock()
        if let t = tuberias[clave] {
            cerrojo.unlock()
            return t
        }
        cerrojo.unlock()
        let b = try biblioteca(fuente, clave: claveFuente)
        guard let vf = b.makeFunction(name: vertice) else { throw ErrorMotorMetal.sinFuncion(vertice) }
        guard let ff = b.makeFunction(name: fragmento) else { throw ErrorMotorMetal.sinFuncion(fragmento) }
        let d = MTLRenderPipelineDescriptor()
        d.label = "\(vertice)+\(fragmento)"
        d.vertexFunction = vf
        d.fragmentFunction = ff
        d.rasterSampleCount = muestras
        d.depthAttachmentPixelFormat = profundidad
        let c = d.colorAttachments[0]!
        c.pixelFormat = color
        if mezcla {
            c.isBlendingEnabled = true
            c.rgbBlendOperation = .add
            c.alphaBlendOperation = .add
            c.sourceRGBBlendFactor = .sourceAlpha
            c.destinationRGBBlendFactor = .oneMinusSourceAlpha
            c.sourceAlphaBlendFactor = .one
            c.destinationAlphaBlendFactor = .oneMinusSourceAlpha
        }
        let t: MTLRenderPipelineState
        do {
            t = try dispositivo.makeRenderPipelineState(descriptor: d)
        } catch {
            throw ErrorMotorMetal.tuberia("\(vertice)+\(fragmento): \(error)")
        }
        cerrojo.lock()
        tuberias[clave] = t
        cerrojo.unlock()
        return t
    }

    /// La prueba de profundidad: `escribe` o solo lee (vidrios y aristas).
    public func profundidad(escribe: Bool) -> MTLDepthStencilState? {
        let clave = escribe ? "escribe" : "lee"
        cerrojo.lock()
        defer { cerrojo.unlock() }
        if let p = profundidades[clave] { return p }
        let d = MTLDepthStencilDescriptor()
        d.depthCompareFunction = .lessEqual
        d.isDepthWriteEnabled = escribe
        let p = dispositivo.makeDepthStencilState(descriptor: d)
        profundidades[clave] = p
        return p
    }

    /// **Sube unos bytes de Rust** a un bufer compartido con la CPU (en
    /// Apple Silicon la memoria es la misma: no hay otra copia). `nil` si
    /// esta vacio o no cabe.
    public func bufer(_ datos: Data, nombre: String = "") -> MTLBuffer? {
        guard !datos.isEmpty, datos.count <= dispositivo.maxBufferLength else { return nil }
        let b = datos.withUnsafeBytes { p -> MTLBuffer? in
            guard let base = p.baseAddress else { return nil }
            return dispositivo.makeBuffer(bytes: base, length: datos.count, options: .storageModeShared)
        }
        b?.label = nombre
        return b
    }

    /// Cuantas muestras por pixel (antialias): 4 si se puede.
    public var muestras: Int { dispositivo.supportsTextureSampleCount(4) ? 4 : 1 }
}
