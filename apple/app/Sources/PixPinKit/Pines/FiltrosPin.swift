// Lo que se le hace a la foto de un pin antes de ensenarla: los filtros
// (5 gris, 6 invertir, 7 y 8 brillo, 0 restaurar) y el giro (R, ⇧R) y los
// volteos (H, V). Es `pixpin-codec/src/filtros.rs` de Windows con las mismas
// cifras.
//
// **Nunca tocan el original**: se rehace lo que se ve desde la foto de
// siempre con lo puesto, asi que 0 restaura de verdad y subir y bajar el
// brillo diez veces devuelve la imagen exacta.

import CoreGraphics
import Foundation

/// Los filtros de un pin (los de Windows: gris, invertido, pasos de brillo).
struct FiltrosPin: Equatable {
    var gris = false
    var invertido = false
    /// Pasos de un 10 %; negativo oscurece.
    var brillo = 0

    /// Lo que sube o baja cada paso: un 10 % de 255, **sumado** (multiplicar
    /// dejaria el negro en negro para siempre).
    static let pasoBrillo = 26
    /// Diez pasos llevan de negro a blanco.
    static let pasosMaximos = 10

    var sonNeutros: Bool { !gris && !invertido && brillo == 0 }

    func conBrillo(_ pasos: Int) -> FiltrosPin {
        var f = self
        f.brillo = min(max(brillo + pasos, -Self.pasosMaximos), Self.pasosMaximos)
        return f
    }

    /// Un canal: primero gris, luego invertir y el brillo al final (al reves,
    /// subir el brillo oscureceria lo invertido).
    func canal(_ valor: UInt8, luma: UInt8) -> UInt8 {
        var v = gris ? Int(luma) : Int(valor)
        if invertido { v = 255 - v }
        v += brillo * Self.pasoBrillo
        return UInt8(min(max(v, 0), 255))
    }

    /// Luminancia con los pesos del ojo (Rec. 601), no la media.
    static func luminancia(_ r: UInt8, _ g: UInt8, _ b: UInt8) -> UInt8 {
        UInt8(min((Int(r) * 299 + Int(g) * 587 + Int(b) * 114) / 1000, 255))
    }
}

/// Giro (cuartos de vuelta a la derecha) y volteos.
struct GiroPin: Equatable {
    var cuartos = 0
    var volteoH = false
    var volteoV = false

    var esNeutro: Bool { cuartos % 4 == 0 && !volteoH && !volteoV }

    /// Gira `pasos` cuartos (1 derecha, 3 izquierda).
    mutating func girar(_ pasos: Int) {
        cuartos = ((cuartos + pasos) % 4 + 4) % 4
    }
}

enum ReglasFiltros {
    /// El marco tras girar un cuarto de vuelta: ancho y alto intercambiados
    /// alrededor del centro, para que el pin no salte de sitio.
    static func marcoGirado(_ r: CGRect) -> CGRect {
        CGRect(x: (r.midX - r.height / 2).rounded(), y: (r.midY - r.width / 2).rounded(),
               width: r.height, height: r.width)
    }

    /// La foto que se ve: `original` con los filtros y luego el giro. Si no
    /// hay nada que hacer, la misma (sin copiar).
    static func aplicar(_ original: CGImage, filtros: FiltrosPin, giro: GiroPin) -> CGImage {
        var imagen = original
        if !filtros.sonNeutros, let f = filtrar(imagen, filtros) { imagen = f }
        if !giro.esNeutro, let g = girar(imagen, giro) { imagen = g }
        return imagen
    }

    /// Los filtros pixel a pixel sobre una copia RGBA. El alfa no se toca
    /// (filtrarlo dejaria un halo en los bordes suaves).
    static func filtrar(_ imagen: CGImage, _ f: FiltrosPin) -> CGImage? {
        let ancho = imagen.width, alto = imagen.height
        guard ancho > 0, alto > 0, let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: ancho * 4,
                                  space: espacio, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let datos = ctx.data
        else { return nil }
        ctx.draw(imagen, in: CGRect(x: 0, y: 0, width: ancho, height: alto))
        let p = datos.bindMemory(to: UInt8.self, capacity: ancho * alto * 4)
        for i in stride(from: 0, to: ancho * alto * 4, by: 4) {
            let a = p[i + 3]
            if a == 0 { continue }
            var r = p[i], g = p[i + 1], b = p[i + 2]
            if a < 255 {
                // Premultiplicado: se quita el alfa, se filtra y se vuelve a poner.
                r = UInt8(min(Int(r) * 255 / Int(a), 255))
                g = UInt8(min(Int(g) * 255 / Int(a), 255))
                b = UInt8(min(Int(b) * 255 / Int(a), 255))
            }
            let luma = FiltrosPin.luminancia(r, g, b)
            r = f.canal(r, luma: luma)
            g = f.canal(g, luma: luma)
            b = f.canal(b, luma: luma)
            if a < 255 {
                r = UInt8(Int(r) * Int(a) / 255)
                g = UInt8(Int(g) * Int(a) / 255)
                b = UInt8(Int(b) * Int(a) / 255)
            }
            p[i] = r
            p[i + 1] = g
            p[i + 2] = b
        }
        return ctx.makeImage()
    }

    /// Gira y voltea una copia.
    static func girar(_ imagen: CGImage, _ giro: GiroPin) -> CGImage? {
        let w = CGFloat(imagen.width), h = CGFloat(imagen.height)
        let tumbada = giro.cuartos % 2 == 1
        let ancho = tumbada ? imagen.height : imagen.width
        let alto = tumbada ? imagen.width : imagen.height
        guard let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: espacio,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
                                      | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        ctx.interpolationQuality = .none
        // Al centro, girar (a la derecha es negativo con la Y hacia arriba) y
        // voltear lo ya girado: H voltea lo que se ve, gire como gire.
        ctx.translateBy(x: CGFloat(ancho) / 2, y: CGFloat(alto) / 2)
        ctx.scaleBy(x: giro.volteoH ? -1 : 1, y: giro.volteoV ? -1 : 1)
        ctx.rotate(by: -CGFloat(giro.cuartos) * .pi / 2)
        ctx.draw(imagen, in: CGRect(x: -w / 2, y: -h / 2, width: w, height: h))
        return ctx.makeImage()
    }
}
