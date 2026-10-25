// **Los ojos del asistente** (E1, `asistente/ojos.rs` de Windows): el
// lienzo pintado para que el modelo lo vea. Rust copia la zona y la
// empaqueta como el `LienzoPlano` de siempre, con la rejilla y las
// etiquetas `#id` ya colocadas; aqui se pinta en un bitmap sin ventana y
// sale en PNG.
//
// En el Mac se pinta con su `PintorLienzo`. En el iPad el pintor del lienzo
// es de la zona del lienzo: lo presta con `OjosDelAsistente.pintor` (pinta
// el `LienzoLeido` en un `CGContext` con el eje Y hacia abajo). Sin el, se
// ve el papel con la rejilla y las etiquetas (el modelo sabe al menos donde
// esta cada cosa).
//
// Lo llama el hilo del turno (nunca el principal): un `CGContext` de mapa
// de bits no necesita ventana ni el hilo principal.

import CoreGraphics
import CoreText
import Foundation
import ImageIO
import PixPinCore
import UIKit
import UniformTypeIdentifiers

extension ManosDelAsistente {
    func pintarVista(vista: VistaParaPintar) -> Data? {
        OjosDelAsistente.pintar(vista)
    }
}

public enum OjosDelAsistente {
    /// **El pintor del lienzo** (lo pone la zona del lienzo): pinta
    /// `lienzo` dentro de `limites` (coordenadas del mundo) en `ctx`, ya
    /// escalado y con el eje Y hacia abajo; `imagen` da la foto de cada
    /// objeto. Se llama fuera del hilo principal.
    nonisolated(unsafe) public static var pintor: ((_ lienzo: LienzoLeido, _ ctx: CGContext, _ limites: CGRect,
                                                    _ imagen: @escaping (UInt64) -> CGImage?) -> Void)?

    /// Pinta la vista y la devuelve en PNG (`nil` si no se pudo).
    static func pintar(_ v: VistaParaPintar) -> Data? {
        let w = Int(v.ancho), h = Int(v.alto)
        guard w > 0, h > 0, w <= 8192, h <= 8192,
              let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: espacio,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        // El eje Y hacia abajo, como el mundo del lienzo.
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: 1, y: -1)
        let leido = try? leerLienzo(v.plano)
        // Un papel transparente se ve blanco, como en el editor.
        ctx.setFillColor(CGColor(gray: 1, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        if let fondo = leido?.fondo.cg, fondo.alpha > 0.01 {
            ctx.setFillColor(fondo)
            ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        }
        if let leido, let pintor {
            var fotos: [UInt64: CGImage] = [:]
            for i in v.imagenes {
                if let f = foto(i.ruta) { fotos[i.idObjeto] = f }
            }
            let s = CGFloat(v.escala)
            let zona = CGRect(x: CGFloat(v.x0), y: CGFloat(v.y0), width: CGFloat(v.x1 - v.x0), height: CGFloat(v.y1 - v.y0))
            ctx.saveGState()
            ctx.scaleBy(x: s, y: s)
            ctx.translateBy(x: -zona.minX, y: -zona.minY)
            pintor(leido, ctx, zona) { fotos[$0] }
            ctx.restoreGState()
        }
        pintarMarcas(v, en: ctx)
        guard let imagen = ctx.makeImage() else { return nil }
        let datos = NSMutableData()
        guard let destino = CGImageDestinationCreateWithData(datos, UTType.png.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destino, imagen, nil)
        return CGImageDestinationFinalize(destino) ? datos as Data : nil
    }

    /// La rejilla (gris claro) con sus numeros y las etiquetas `#id` (sobre
    /// amarillo), en pixeles de la imagen (`pintar_marcas` de Windows). El
    /// texto con Core Text (sin UIKit: este no es el hilo principal).
    private static func pintarMarcas(_ v: VistaParaPintar, en ctx: CGContext) {
        ctx.setStrokeColor(CGColor(red: 0.45, green: 0.5, blue: 0.58, alpha: 0.28))
        ctx.setLineWidth(1)
        for r in v.rayas {
            ctx.move(to: CGPoint(x: CGFloat(r.x0), y: CGFloat(r.y0)))
            ctx.addLine(to: CGPoint(x: CGFloat(r.x1), y: CGFloat(r.y1)))
        }
        ctx.strokePath()
        guard !v.rotulos.isEmpty else { return }
        let letra = CTFontCreateUIFontForLanguage(.system, 11, nil) ?? CTFontCreateWithName("Helvetica" as CFString, 11, nil)
        for r in v.rotulos {
            let color = r.etiqueta ? CGColor(gray: 0.08, alpha: 1) : CGColor(red: 0.25, green: 0.28, blue: 0.33, alpha: 1)
            let fondo = r.etiqueta ? CGColor(red: 1, green: 0.86, blue: 0.25, alpha: 0.92) : CGColor(gray: 1, alpha: 0.75)
            let atributos: [NSAttributedString.Key: Any] = [
                NSAttributedString.Key(kCTFontAttributeName as String): letra,
                NSAttributedString.Key(kCTForegroundColorAttributeName as String): color,
            ]
            let linea = CTLineCreateWithAttributedString(NSAttributedString(string: r.texto, attributes: atributos))
            var subida: CGFloat = 0, bajada: CGFloat = 0, espacio: CGFloat = 0
            let ancho = CGFloat(CTLineGetTypographicBounds(linea, &subida, &bajada, &espacio))
            let alto = subida + bajada
            let caja = CGRect(x: CGFloat(r.x) - 2, y: CGFloat(r.y) - 1, width: ancho + 4, height: alto + 2)
            ctx.setFillColor(fondo)
            ctx.fill(caja)
            // Core Text pinta con el eje Y hacia arriba: se da la vuelta
            // solo para la linea.
            ctx.saveGState()
            ctx.textMatrix = .identity
            ctx.translateBy(x: CGFloat(r.x), y: CGFloat(r.y) + subida)
            ctx.scaleBy(x: 1, y: -1)
            ctx.textPosition = .zero
            CTLineDraw(linea, ctx)
            ctx.restoreGState()
        }
    }

    /// Una foto del lienzo, reducida a lo que hace falta para la vista.
    private static func foto(_ ruta: String) -> CGImage? {
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: 2048,
        ]
        guard let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: ruta) as CFURL, nil) else { return nil }
        return CGImageSourceCreateThumbnailAtIndex(src, 0, opciones as CFDictionary)
    }
}
