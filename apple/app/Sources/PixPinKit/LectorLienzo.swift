// El lienzo que llega de Rust como UN solo bufer (`LienzoPlano`), leido en
// una pasada y convertido en figuras listas para pintar.
//
// Formato: `apple/nucleo/pixpin-nucleo/src/lienzo.rs` (version 1). Todo son
// `Float` little-endian; los textos y las imagenes van en dos tablas aparte,
// citados por su indice.
//
// Rendimiento (§4bis del plan):
// - una sola copia de memoria del bufer (`copyBytes`), nada de leer numero a
//   numero desde `Data`;
// - cada `CGPath` se construye UNA vez al leer y se reutiliza en cada
//   fotograma: pintar es solo recorrer el arreglo.

import CoreGraphics
import Foundation
import PixPinCore

/// Un color RGBA en 0...1, tal cual viene del bufer.
public struct ColorLienzo: Equatable, Sendable {
    public var r: CGFloat, g: CGFloat, b: CGFloat, a: CGFloat
    public var cg: CGColor { CGColor(srgbRed: r, green: g, blue: b, alpha: a) }
}

/// Lo que se pinta, ya con sus trazados hechos.
public enum FiguraLienzo {
    /// Rellenar con la regla par-impar (poligonos, rellenos de figuras).
    case relleno(CGPath, ColorLienzo)
    /// La tinta a mano: curvas por puntos medios y regla *winding*.
    case tinta(CGPath, ColorLienzo)
    /// Linea abierta. `estilo`: 0 solido, 1 discontinuo, 2 punteado, 3 raya y punto.
    case linea(CGPath, ColorLienzo, grosor: CGFloat, estilo: Int)
    /// Oscurecer todo menos el hueco.
    case velo(hueco: CGPath, ColorLienzo)
    /// Texto con la esquina de su caja en `origen`.
    case texto(String, origen: CGPoint, tam: CGFloat, anchoMax: CGFloat,
               familia: String, negrita: Bool, cursiva: Bool, ColorLienzo)
    /// El numero de una cota: girado `angulo` (radianes) alrededor de
    /// `centro`, con halo.
    case rotulo(String, origen: CGPoint, tam: CGFloat, familia: String,
                ColorLienzo, halo: ColorLienzo, grosorHalo: CGFloat,
                centro: CGPoint, angulo: CGFloat)
    /// Un bitmap por su id (`ImagenDelLienzo`).
    case imagen(id: UInt64, caja: CGRect, opacidad: CGFloat, angulo: CGFloat, recorte: CGRect?,
                natural: CGSize?)
}

/// El lienzo entero, listo para pintar.
public struct LienzoLeido {
    public var fondo: ColorLienzo
    /// Lo dibujado, en unidades del mundo; `nil` si esta vacio.
    public var caja: CGRect?
    public var figuras: [FiguraLienzo]
}

public enum ErrorLectorLienzo: Error, Equatable {
    case version(Int)
    case corto
    case largoDistinto(esperado: Int, real: Int)
}

/// La version del formato que este lector entiende.
public let versionLienzoPlano = 1

/// Lee el bufer entero. Una orden de codigo desconocido se salta (su
/// `largo` dice cuanto), asi un nucleo mas nuevo no rompe una app vieja.
public func leerLienzo(_ plano: LienzoPlano) throws -> LienzoLeido {
    let n = plano.datos.count / MemoryLayout<Float>.size
    var f = [Float](repeating: 0, count: n)
    _ = f.withUnsafeMutableBytes { plano.datos.copyBytes(to: $0) }
    guard n >= 12 else { throw ErrorLectorLienzo.corto }
    let version = Int(f[0])
    guard version == versionLienzoPlano else { throw ErrorLectorLienzo.version(version) }
    let cabecera = Int(f[1])
    let cuantas = Int(f[2])
    let total = Int(f[11])
    guard total == n else { throw ErrorLectorLienzo.largoDistinto(esperado: total, real: n) }
    func color(_ i: Int) -> ColorLienzo {
        ColorLienzo(r: CGFloat(f[i]), g: CGFloat(f[i + 1]), b: CGFloat(f[i + 2]), a: CGFloat(f[i + 3]))
    }
    let fondo = color(3)
    let caja: CGRect? = f[7].isNaN ? nil : CGRect(
        x: CGFloat(f[7]), y: CGFloat(f[8]),
        width: CGFloat(f[9] - f[7]), height: CGFloat(f[10] - f[8]))

    func texto(_ i: Int) -> String {
        let k = Int(f[i])
        return k >= 0 && k < plano.textos.count ? plano.textos[k] : ""
    }
    /// `n, x0 y0 x1 y1 ...` desde `i`.
    func puntos(_ i: Int) -> [CGPoint] {
        let cuantos = Int(f[i])
        var p = [CGPoint]()
        p.reserveCapacity(cuantos)
        var j = i + 1
        for _ in 0..<cuantos {
            p.append(CGPoint(x: CGFloat(f[j]), y: CGFloat(f[j + 1])))
            j += 2
        }
        return p
    }

    var figuras = [FiguraLienzo]()
    figuras.reserveCapacity(cuantas)
    var i = cabecera
    while i + 1 < n {
        let codigo = Int(f[i])
        let largo = Int(f[i + 1])
        guard largo >= 2, i + largo <= n else { throw ErrorLectorLienzo.corto }
        let d = i + 2
        switch codigo {
        case 1, 4:
            figuras.append(.relleno(poligono(puntos(d + 4)), color(d)))
        case 2:
            figuras.append(.tinta(tinta(puntos(d + 4)), color(d)))
        case 3:
            figuras.append(.linea(polilinea(puntos(d + 6)), color(d),
                                  grosor: CGFloat(f[d + 4]), estilo: Int(f[d + 5])))
        case 5:
            figuras.append(.velo(hueco: poligono(puntos(d + 4)), color(d)))
        case 6:
            let caras = Int(f[d + 8])
            figuras.append(.texto(texto(d + 9),
                                  origen: CGPoint(x: CGFloat(f[d + 4]), y: CGFloat(f[d + 5])),
                                  tam: CGFloat(f[d + 6]), anchoMax: CGFloat(f[d + 7]),
                                  familia: texto(d + 10),
                                  negrita: caras & 1 != 0, cursiva: caras & 2 != 0, color(d)))
        case 7:
            figuras.append(.rotulo(texto(d + 15),
                                   origen: CGPoint(x: CGFloat(f[d + 8]), y: CGFloat(f[d + 9])),
                                   tam: CGFloat(f[d + 10]), familia: texto(d + 16),
                                   color(d), halo: color(d + 4), grosorHalo: CGFloat(f[d + 11]),
                                   centro: CGPoint(x: CGFloat(f[d + 12]), y: CGFloat(f[d + 13])),
                                   angulo: CGFloat(f[d + 14])))
        case 8:
            let k = Int(f[d + 6])
            let id = k >= 0 && k < plano.imagenes.count ? plano.imagenes[k] : 0
            let conRecorte = f[d + 7] == 1
            figuras.append(.imagen(
                id: id,
                caja: CGRect(x: CGFloat(f[d]), y: CGFloat(f[d + 1]),
                             width: CGFloat(f[d + 2]), height: CGFloat(f[d + 3])),
                opacidad: CGFloat(f[d + 4]), angulo: CGFloat(f[d + 5]),
                recorte: conRecorte ? CGRect(x: CGFloat(f[d + 8]), y: CGFloat(f[d + 9]),
                                             width: CGFloat(f[d + 10]), height: CGFloat(f[d + 11])) : nil,
                natural: conRecorte ? CGSize(width: CGFloat(f[d + 12]), height: CGFloat(f[d + 13])) : nil))
        default:
            break // codigo de una version mas nueva: se salta entero
        }
        i += largo
    }
    return LienzoLeido(fondo: fondo, caja: caja, figuras: figuras)
}

func poligono(_ p: [CGPoint]) -> CGPath {
    let c = CGMutablePath()
    if !p.isEmpty { c.addLines(between: p); c.closeSubpath() }
    return c
}

func polilinea(_ p: [CGPoint]) -> CGPath {
    let c = CGMutablePath()
    if !p.isEmpty { c.addLines(between: p) }
    return c
}

/// El contorno de un trazo a mano como en Excalidraw (`pixpin_render::tinta`):
/// curvas cuadraticas que pasan por los puntos medios, cerrado.
func tinta(_ p: [CGPoint]) -> CGPath {
    let c = CGMutablePath()
    guard p.count > 2 else { return poligono(p) }
    func medio(_ a: CGPoint, _ b: CGPoint) -> CGPoint { CGPoint(x: (a.x + b.x) / 2, y: (a.y + b.y) / 2) }
    c.move(to: medio(p[0], p[1]))
    for k in 1..<p.count {
        let siguiente = p[(k + 1) % p.count]
        c.addQuadCurve(to: medio(p[k], siguiente), control: p[k])
    }
    c.addQuadCurve(to: medio(p[0], p[1]), control: p[0])
    c.closeSubpath()
    return c
}
