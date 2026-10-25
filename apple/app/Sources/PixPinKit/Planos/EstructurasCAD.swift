// Los buferes de los planos y modelos 3D tal como llegan de Rust
// (`pixpin-nucleo/src/planos.rs` y `modelos3d.rs`): palabras de 4 bytes en
// little-endian, una estructura detras de otra, con la disposicion de los
// `#[repr(C)]` de `pixpin_cad`. Metal los sube tal cual; aqui solo se leen
// los pocos que mira la CPU (los tramos, para dibujar solo lo que se ve, y
// todo al imprimir).
//
// Las estructuras de Swift con campos de 4 bytes van en el orden en que se
// declaran y sin relleno: las pruebas lo comprueban campo a campo
// (`MemoryLayout.offset(of:)`) contra los numeros de Rust.

import CoreGraphics
import Foundation
import PixPinCore

/// `pixpin_cad::modelo::Vertice` (12 bytes). Color RGBA, rojo en el byte
/// bajo; alfa 0 es «el color 7» (blanco en oscuro, negro en claro).
public struct VerticeCAD: Equatable {
    public var x: Float
    public var y: Float
    public var color: UInt32
}

/// `VerticeTrama` (16 bytes).
public struct VerticeTramaCAD: Equatable {
    public var x: Float
    public var y: Float
    public var color: UInt32
    public var trama: UInt32
}

/// `Trama` (36 bytes): un sombreado con patron.
public struct TramaCAD: Equatable {
    public var centroX: Float
    public var centroY: Float
    public var inversa0: Float
    public var inversa1: Float
    public var inversa2: Float
    public var inversa3: Float
    public var escala: Float
    public var desde: UInt32
    public var cuantas: UInt32
}

/// `Familia` (64 bytes): rayas paralelas de un patron.
public struct FamiliaCAD {
    public var baseX: Float
    public var baseY: Float
    public var dirX: Float
    public var dirY: Float
    public var paso: Float
    public var corrimiento: Float
    public var largo: Float
    public var n: UInt32
    public var trazos: (Float, Float, Float, Float, Float, Float, Float, Float)
}

/// `Letra` (32 bytes): una instancia de una letra.
public struct LetraCAD: Equatable {
    public var x: Float
    public var y: Float
    public var m0: Float
    public var m1: Float
    public var m2: Float
    public var m3: Float
    public var color: UInt32
    public var glifo: UInt32
}

/// `Arco` (32 bytes): un circulo o un arco exacto.
public struct ArcoCAD: Equatable {
    public var centroX: Float
    public var centroY: Float
    public var radio: Float
    public var inicio: Float
    public var barrido: Float
    public var color: UInt32
    public var relleno0: UInt32
    public var relleno1: UInt32
}

/// `Tramo` (32 bytes): un trozo seguido de un bufer que se dibuja de una vez.
public struct TramoCAD: Equatable {
    public var desde: UInt32
    public var cuantos: UInt32
    public var x0: Float
    public var y0: Float
    public var x1: Float
    public var y1: Float
    /// Lo que mide lo que lleva: por debajo de un pixel no se dibuja.
    public var tamano: Float
    /// En las letras: cuantos vertices tiene la mayor (bit alto: de rayas).
    public var clase: UInt32
}

/// `pixpin_cad::modelo3d::V3` (24 bytes).
public struct V3CAD: Equatable {
    public var x: Float
    public var y: Float
    public var z: Float
    public var normal: UInt32
    public var color: UInt32
    public var elemento: UInt32
}

/// Los pasos de cada bufer en Rust (lo que comprueban las pruebas).
public enum PasosCAD {
    public static let vertice = 12
    public static let verticeTrama = 16
    public static let trama = 36
    public static let familia = 64
    public static let letra = 32
    public static let arco = 32
    public static let tramo = 32
    public static let v3 = 24
    public static let malla = 8
    public static let glifo = 8
    /// El bit alto de `glifos[1]` y de `TramoCAD.clase`: la letra es de rayas.
    public static let glifoDeRayas: UInt32 = 0x8000_0000
    /// El indice que corta una tira de lineas.
    public static let corte: UInt32 = 0xFFFF_FFFF
}

/// Copia `datos` en un `[T]` sin suponer que esten alineados.
public func leerEstructuras<T>(_ datos: Data, como _: T.Type) -> [T] {
    let paso = MemoryLayout<T>.stride
    let n = datos.count / paso
    guard n > 0 else { return [] }
    return [T](unsafeUninitializedCapacity: n) { destino, cuantos in
        datos.withUnsafeBytes { origen in
            if let o = origen.baseAddress, let d = destino.baseAddress {
                UnsafeMutableRawPointer(d).copyMemory(from: o, byteCount: n * paso)
            }
        }
        cuantos = n
    }
}

/// Una caja en `f32` (coordenadas de un grupo).
public struct CajaF: Equatable {
    public var x0: Float, y0: Float, x1: Float, y1: Float

    public init(x0: Float, y0: Float, x1: Float, y1: Float) {
        self.x0 = x0
        self.y0 = y0
        self.x1 = x1
        self.y1 = y1
    }

    func corta(_ t: TramoCAD) -> Bool {
        t.x0 <= x1 && t.x1 >= x0 && t.y0 <= y1 && t.y1 >= y0
    }
}

/// **Los tramos que se ven** (por sitio y por tamano), juntando los
/// seguidos: `gpu::visibles` de Windows. Van de mayor a menor, asi que en
/// cuanto uno es pequeno, los demas tambien.
public func tramosVisibles(_ tramos: [TramoCAD], en caja: CajaF, minimo: Float) -> [(desde: UInt32, cuantos: UInt32)] {
    var out: [(desde: UInt32, cuantos: UInt32)] = []
    for t in tramos {
        if t.tamano < minimo { break }
        if !caja.corta(t) { continue }
        if let ultimo = out.last, ultimo.desde + ultimo.cuantos == t.desde {
            out[out.count - 1].cuantos += t.cuantos
        } else {
            out.append((t.desde, t.cuantos))
        }
    }
    return out
}

// MARK: - Colores (los del sombreador, para imprimir en papel)

/// Un color RGBA del plano (0...1).
public struct ColorCAD: Equatable {
    public var r: Double, g: Double, b: Double, a: Double

    public init(r: Double, g: Double, b: Double, a: Double) {
        self.r = r
        self.g = g
        self.b = b
        self.a = a
    }

    public init(_ c: UInt32) {
        r = Double(c & 255) / 255
        g = Double((c >> 8) & 255) / 255
        b = Double((c >> 16) & 255) / 255
        a = Double((c >> 24) & 255) / 255
    }

    public var cg: CGColor { CGColor(srgbRed: r, green: g, blue: b, alpha: a) }

    /// **Sobre papel blanco** (`tinta` del sombreador con fondo claro): el
    /// color 7 sale negro y los colores que no se leerian en claro se
    /// oscurecen sin perder su tono.
    public static func enPapel(_ c: UInt32) -> ColorCAD {
        var k = ColorCAD(c)
        if k.a == 0 { return ColorCAD(r: 0, g: 0, b: 0, a: 1) }
        let luz = 0.2126 * k.r + 0.7152 * k.g + 0.0722 * k.b
        if luz > 0.62 {
            let f = 0.5 / luz
            k.r *= f
            k.g *= f
            k.b *= f
        }
        return k
    }
}

// MARK: - La camara del plano (2D)

/// **Lo que ve el visor de planos**: el punto del dibujo en el centro y lo
/// que mide un pixel. Los pixeles van desde abajo a la izquierda (como una
/// `NSView` sin voltear), asi que la y del dibujo y la de la pantalla suben
/// las dos.
public struct CamaraPlano: Equatable {
    public var centroX: Double
    public var centroY: Double
    /// Unidades del dibujo por pixel.
    public var px: Double

    public init(centroX: Double, centroY: Double, px: Double) {
        self.centroX = centroX
        self.centroY = centroY
        self.px = px
    }

    /// La caja entera en `ancho` x `alto` pixeles, con un margen.
    public static func encuadrar(_ c: CajaPlano, ancho: Double, alto: Double) -> CamaraPlano {
        let w = max(c.x1 - c.x0, 1e-9), h = max(c.y1 - c.y0, 1e-9)
        let px = max(w / max(ancho, 1), h / max(alto, 1)) * 1.08
        return CamaraPlano(centroX: (c.x0 + c.x1) / 2, centroY: (c.y0 + c.y1) / 2, px: px)
    }

    /// Del pixel `(x, y)` (desde abajo a la izquierda) al dibujo.
    public func aDibujo(x: Double, y: Double, ancho: Double, alto: Double) -> PuntoPlano {
        PuntoPlano(x: centroX + (x - ancho / 2) * px, y: centroY + (y - alto / 2) * px)
    }

    /// Del dibujo al pixel.
    public func aPixel(_ p: PuntoPlano, ancho: Double, alto: Double) -> CGPoint {
        CGPoint(x: (p.x - centroX) / px + ancho / 2, y: (p.y - centroY) / px + alto / 2)
    }

    /// Acercar `factor` (>1 acerca) dejando quieto el pixel `(x, y)`.
    public func zoom(_ factor: Double, x: Double, y: Double, ancho: Double, alto: Double) -> CamaraPlano {
        guard factor > 0, factor.isFinite else { return self }
        let fijo = aDibujo(x: x, y: y, ancho: ancho, alto: alto)
        var c = self
        c.px = min(max(px / factor, 1e-9), 1e12)
        c.centroX = fijo.x - (x - ancho / 2) * c.px
        c.centroY = fijo.y - (y - alto / 2) * c.px
        return c
    }

    /// Mover `dx`, `dy` pixeles (el plano sigue al dedo).
    public func mover(dx: Double, dy: Double) -> CamaraPlano {
        var c = self
        c.centroX -= dx * px
        c.centroY -= dy * px
        return c
    }

    /// La caja que se ve, en el dibujo.
    public func caja(ancho: Double, alto: Double) -> CajaPlano {
        CajaPlano(x0: centroX - ancho / 2 * px, y0: centroY - alto / 2 * px,
                  x1: centroX + ancho / 2 * px, y1: centroY + alto / 2 * px)
    }
}

// MARK: - La camara 3D (los ejes, para la luz)

/// Derecha, arriba y adelante de la camara (`Orbita::ejes` de Rust): la luz
/// del sombreador 3D va con ellos, como en Windows.
public func ejesDeCamara(_ c: Camara3d) -> (derecha: SIMD3<Double>, arriba: SIMD3<Double>, adelante: SIMD3<Double>) {
    let ca = cos(c.altura), sa = sin(c.altura)
    let f = SIMD3<Double>(-ca * cos(c.rumbo), -ca * sin(c.rumbo), -sa)
    var r = SIMD3<Double>(f.y, -f.x, 0) // f x (0,0,1)
    if (r * r).sum() < 1e-12 { r = SIMD3<Double>(-sin(c.rumbo), cos(c.rumbo), 0) }
    r /= ((r * r).sum()).squareRoot()
    let u = SIMD3<Double>(r.y * f.z - r.z * f.y, r.z * f.x - r.x * f.z, r.x * f.y - r.y * f.x)
    return (r, u, f)
}

/// La luz del visor 3D de Windows: algo por detras del ojo, arriba y a la
/// derecha.
public func luzDeCamara(_ c: Camara3d) -> SIMD3<Float> {
    let (r, u, f) = ejesDeCamara(c)
    var l = f * -0.5 + u * 0.75 + r * 0.35
    l /= ((l * l).sum()).squareRoot()
    return SIMD3<Float>(Float(l.x), Float(l.y), Float(l.z))
}

/// **Las palabras de un uniforme**: floats y enteros de 4 bytes, uno detras
/// de otro, para `setVertexBytes`. Asi el orden no depende de como coloque
/// Swift una estructura.
public struct PalabrasUniforme {
    public private(set) var palabras: [UInt32] = []

    public init() {}

    public mutating func f(_ v: Float) { palabras.append(v.bitPattern) }
    public mutating func f(_ v: Double) { palabras.append(Float(v).bitPattern) }
    public mutating func u(_ v: UInt32) { palabras.append(v) }
    public mutating func f4(_ a: Float, _ b: Float, _ c: Float, _ d: Float) {
        f(a)
        f(b)
        f(c)
        f(d)
    }

    public var bytes: Int { palabras.count * 4 }
}
