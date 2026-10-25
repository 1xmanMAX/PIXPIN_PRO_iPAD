// Pinta la tinta de una hoja (un `LienzoLeido` del `EditorLienzo` que da
// `Nucleo.anotarHoja`) en un `CGContext` con la Y hacia abajo, el de una
// `UIView`. Es el `PintorLienzo` del Mac sin AppKit y sin lo que no sale en
// la tinta de un documento (los mapas de grafito y los tapados): las fotos
// se piden a `imagen(id)` y, si no hay, se deja un hueco gris.
//
// No crea trazados: los `CGPath` ya vienen hechos del lector del bufer.

import CoreGraphics
import PixPinCore
import UIKit

struct PintorTintaHoja {
    var lienzo: LienzoLeido
    var imagen: (UInt64) -> CGImage? = { _ in nil }

    /// Pinta todo. `limites`: lo visible, en unidades de la hoja (para el
    /// velo, que oscurece «todo lo demas»).
    func pintar(en ctx: CGContext, limites: CGRect) {
        for figura in lienzo.figuras {
            switch figura {
            case let .relleno(path, color):
                ctx.addPath(path)
                ctx.setFillColor(color.cg)
                ctx.fillPath(using: .evenOdd)
            case let .tinta(path, color):
                ctx.addPath(path)
                ctx.setFillColor(color.cg)
                ctx.fillPath(using: .winding)
            case let .linea(path, color, grosor, estilo):
                ctx.saveGState()
                ctx.addPath(path)
                ctx.setStrokeColor(color.cg)
                ctx.setLineWidth(grosor)
                ctx.setLineCap(.round)
                ctx.setLineJoin(.round)
                switch estilo {
                case 1: ctx.setLineDash(phase: 0, lengths: [grosor * 4, grosor * 3])
                case 2: ctx.setLineDash(phase: 0, lengths: [0.01, grosor * 2.5])
                case 3:
                    let hueco = grosor * 2 + 2
                    ctx.setLineDash(phase: 0, lengths: [grosor * 5 + 4, hueco, 0.01, hueco])
                default: break
                }
                ctx.strokePath()
                ctx.restoreGState()
            case let .velo(hueco, color):
                ctx.addRect(limites.insetBy(dx: -1, dy: -1))
                ctx.addPath(hueco)
                ctx.setFillColor(color.cg)
                ctx.fillPath(using: .evenOdd)
            case let .texto(texto, origen, tam, anchoMax, familia, negrita, cursiva, color):
                let atributos = PintorTintaHoja.atributos(tam: tam, familia: familia, negrita: negrita,
                                                          cursiva: cursiva, color: color)
                let ancho = anchoMax > 0 && anchoMax < 1e6 ? anchoMax : .greatestFiniteMagnitude
                UIGraphicsPushContext(ctx)
                (texto as NSString).draw(
                    with: CGRect(x: origen.x, y: origen.y, width: ancho, height: .greatestFiniteMagnitude),
                    options: [.usesLineFragmentOrigin], attributes: atributos, context: nil)
                UIGraphicsPopContext()
            case let .rotulo(texto, origen, tam, familia, color, halo, grosorHalo, centro, angulo):
                ctx.saveGState()
                ctx.translateBy(x: centro.x, y: centro.y)
                ctx.rotate(by: angulo)
                ctx.translateBy(x: -centro.x, y: -centro.y)
                UIGraphicsPushContext(ctx)
                var conHalo = PintorTintaHoja.atributos(tam: tam, familia: familia, negrita: false,
                                                        cursiva: false, color: halo)
                conHalo[.strokeColor] = UIColor(cgColor: halo.cg)
                conHalo[.strokeWidth] = grosorHalo / max(tam, 1) * 100
                (texto as NSString).draw(at: origen, withAttributes: conHalo)
                (texto as NSString).draw(at: origen, withAttributes: PintorTintaHoja.atributos(
                    tam: tam, familia: familia, negrita: false, cursiva: false, color: color))
                UIGraphicsPopContext()
                ctx.restoreGState()
            case let .imagen(id, caja, opacidad, angulo, recorte, natural):
                ctx.saveGState()
                ctx.setAlpha(opacidad)
                ctx.translateBy(x: caja.midX, y: caja.midY)
                ctx.rotate(by: angulo)
                let local = CGRect(x: -caja.width / 2, y: -caja.height / 2, width: caja.width, height: caja.height)
                if var bitmap = imagen(id) {
                    if let r = recorte, let nat = natural, nat.width > 0, nat.height > 0 {
                        let sx = CGFloat(bitmap.width) / nat.width
                        let sy = CGFloat(bitmap.height) / nat.height
                        let px = CGRect(x: r.minX * sx, y: r.minY * sy, width: r.width * sx, height: r.height * sy)
                        bitmap = bitmap.cropping(to: px.integral) ?? bitmap
                    }
                    // Con la Y hacia abajo, la foto se voltea.
                    ctx.translateBy(x: 0, y: local.maxY + local.minY)
                    ctx.scaleBy(x: 1, y: -1)
                    ctx.interpolationQuality = .high
                    ctx.draw(bitmap, in: local)
                } else {
                    ctx.setFillColor(gray: 0.5, alpha: 0.15)
                    ctx.fill(local)
                }
                ctx.restoreGState()
            }
        }
    }

    static func atributos(tam: CGFloat, familia: String, negrita: Bool, cursiva: Bool,
                          color: ColorLienzo) -> [NSAttributedString.Key: Any] {
        var fuente = UIFont(name: familia, size: tam) ?? UIFont.systemFont(ofSize: tam)
        var rasgos: UIFontDescriptor.SymbolicTraits = []
        if negrita { rasgos.insert(.traitBold) }
        if cursiva { rasgos.insert(.traitItalic) }
        if !rasgos.isEmpty, let d = fuente.fontDescriptor.withSymbolicTraits(rasgos) {
            fuente = UIFont(descriptor: d, size: tam)
        }
        return [.font: fuente, .foregroundColor: UIColor(cgColor: color.cg)]
    }
}
