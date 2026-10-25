// Pinta un `LienzoLeido` en un `CGContext` con el eje Y hacia abajo (el del
// mundo de Excalidraw y el de una `UIView`).
//
// Port del `PintorLienzo.swift` del Mac: lo mismo, con UIKit en vez de
// AppKit para el texto (`UIGraphicsPushContext` en vez de
// `NSGraphicsContext`). No guarda estado ni crea trazados: los `CGPath` ya
// vienen hechos del lector. Las imagenes se piden a `imagen(id)`, que la
// vista cachea.
//
// Se puede usar fuera del hilo principal (las miniaturas del chat): no toca
// nada de la interfaz, solo `UIFont`, Core Text y el dibujo de cadenas, que
// en UIKit son seguros en cualquier hilo.
//
// Las letras: las ocho del lienzo van dentro de la app y se pintan con su
// interlineado fijo (`LetrasLienzo.swift`).

import CoreGraphics
import PixPinCore
import UIKit

/// **Los mapas de grafito ya hechos imagen** (`grafito_plano.rs`): su id
/// lleva el bit alto y cambia cuando cambia el mapa, asi que se piden una
/// vez al nucleo.
enum GrafitosPintados {
    static let bit: UInt64 = 1 << 61
    nonisolated(unsafe) private static let cache: NSCache<NSNumber, CGImage> = {
        let c = NSCache<NSNumber, CGImage>()
        c.totalCostLimit = 128 << 20
        return c
    }()

    static func imagen(_ id: UInt64) -> CGImage? {
        guard id & bit != 0, id >> 62 == 0 else { return nil }
        let k = NSNumber(value: id)
        if let img = cache.object(forKey: k) { return img }
        guard let m = mapaDeGrafito(id: id), m.ancho > 0, m.alto > 0,
              m.rgba.count >= Int(m.ancho) * Int(m.alto) * 4,
              let prov = CGDataProvider(data: m.rgba as CFData),
              let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let img = CGImage(width: Int(m.ancho), height: Int(m.alto), bitsPerComponent: 8, bitsPerPixel: 32,
                                bytesPerRow: Int(m.ancho) * 4, space: espacio,
                                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue),
                                provider: prov, decode: nil, shouldInterpolate: true, intent: .defaultIntent)
        else { return nil }
        cache.setObject(img, forKey: k, cost: m.rgba.count)
        return img
    }
}

public struct PintorLienzo {
    public var lienzo: LienzoLeido
    /// El bitmap de una imagen del lienzo, ya decodificado (o `nil`).
    public var imagen: (UInt64) -> CGImage?

    public init(lienzo: LienzoLeido, imagen: @escaping (UInt64) -> CGImage? = { _ in nil }) {
        self.lienzo = lienzo
        self.imagen = imagen
    }

    /// Si `id` es de un mosaico o una lupa (bits 63 y 62, ver
    /// `editor_tapado.rs`) y no de una foto.
    static func esTapado(_ id: UInt64) -> Bool { id >> 62 != 0 }

    /// Pinta todo. `limites`: lo visible, en unidades del mundo (para el
    /// velo, que oscurece «todo lo demas»).
    public func pintar(en ctx: CGContext, limites: CGRect) {
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
                // Las ocho letras del lienzo, renglon a renglon con su
                // interlineado, como Windows (`LetrasLienzo`).
                if LetrasLienzo.pintar(texto, origen: origen, tam: tam, anchoMax: anchoMax, familia: familia,
                                       negrita: negrita, cursiva: cursiva, color: color.cg, en: ctx) {
                    continue
                }
                let atributos = PintorLienzo.atributos(tam: tam, familia: familia, negrita: negrita,
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
                var conHalo = PintorLienzo.atributos(tam: tam, familia: familia, negrita: false,
                                                    cursiva: false, color: halo)
                conHalo[.strokeColor] = UIColor(cgColor: halo.cg)
                conHalo[.strokeWidth] = grosorHalo / max(tam, 1) * 100
                (texto as NSString).draw(at: origen, withAttributes: conHalo)
                (texto as NSString).draw(at: origen, withAttributes: PintorLienzo.atributos(
                    tam: tam, familia: familia, negrita: false, cursiva: false, color: color))
                UIGraphicsPopContext()
                ctx.restoreGState()
            case let .imagen(id, caja, opacidad, angulo, recorte, natural):
                ctx.saveGState()
                ctx.setAlpha(opacidad)
                ctx.translateBy(x: caja.midX, y: caja.midY)
                ctx.rotate(by: angulo)
                var local = CGRect(x: -caja.width / 2, y: -caja.height / 2,
                                   width: caja.width, height: caja.height)
                // Lo de dentro de un mosaico o una lupa un pelo mas grande:
                // el borde suavizado dejaba asomar la banda maciza de debajo
                // como un filo gris.
                if PintorLienzo.esTapado(id) { local = local.insetBy(dx: -0.75, dy: -0.75) }
                if var bitmap = imagen(id) ?? GrafitosPintados.imagen(id) {
                    if let r = recorte, let nat = natural, nat.width > 0, nat.height > 0 {
                        // El recorte viene en las medidas naturales de la foto.
                        let sx = CGFloat(bitmap.width) / nat.width
                        let sy = CGFloat(bitmap.height) / nat.height
                        let px = CGRect(x: r.minX * sx, y: r.minY * sy, width: r.width * sx, height: r.height * sy)
                        bitmap = bitmap.cropping(to: px.integral) ?? bitmap
                    }
                    // En un contexto con Y hacia abajo, la foto se voltea.
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
        // La letra de verdad (las del lienzo van dentro de la app) o la del
        // sistema, con su cara.
        LetrasLienzo.atributos(familia: familia, tam: tam, negrita: negrita, cursiva: cursiva, color: color.cg)
    }
}

/// **Un lienzo leido pintado en un mapa de bits** (la miniatura del chat, la
/// foto de una hoja): `caja` del mundo encajada en `ancho` x `alto` pixeles
/// con `margen`, el papel debajo. Sin estado: vale en cualquier hilo.
enum MapaLienzo {
    /// Un mapa RGBA vacio de `w` x `h`.
    static func mapa(_ w: Int, _ h: Int) -> CGContext? {
        CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                  space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
    }

    /// Pinta `lienzo` llenando `ancho` x `alto` (como mucho 4 veces su
    /// tamano, como la burbuja del Mac). `nil` si no se pudo.
    static func imagen(_ lienzo: LienzoLeido, ancho: Int, alto: Int, margen: CGFloat,
                       fotos: @escaping (UInt64) -> CGImage?) -> CGImage? {
        guard ancho > 0, alto > 0, let ctx = mapa(ancho, alto) else { return nil }
        let bounds = CGRect(x: 0, y: 0, width: ancho, height: alto)
        // Y hacia abajo, como el mundo.
        ctx.translateBy(x: 0, y: CGFloat(alto))
        ctx.scaleBy(x: 1, y: -1)
        ctx.setFillColor(lienzo.fondo.cg)
        ctx.fill(bounds)
        let caja = lienzo.caja ?? bounds
        let escala = min((bounds.width - margen * 2) / max(caja.width, 1),
                         (bounds.height - margen * 2) / max(caja.height, 1), 4 * bounds.width / 220)
        // Centrado en el hueco.
        let dx = (bounds.width - caja.width * escala) / 2 - caja.minX * escala
        let dy = (bounds.height - caja.height * escala) / 2 - caja.minY * escala
        ctx.translateBy(x: dx, y: dy)
        ctx.scaleBy(x: escala, y: escala)
        let visible = CGRect(x: -dx / escala, y: -dy / escala,
                             width: bounds.width / escala, height: bounds.height / escala)
        PintorLienzo(lienzo: lienzo, imagen: fotos).pintar(en: ctx, limites: visible)
        return ctx.makeImage()
    }
}

/// **Lo que la zona del lienzo presta al resto de la app** al arrancar: el
/// pintor para los ojos del asistente (`OjosDelAsistente.pintor`). Lo llama
/// la app al arrancar; el editor y la miniatura lo llaman tambien por si
/// acaso (se hace una vez).
@MainActor
public enum LienzoDelIPad {
    private static var activado = false

    public static func activar() {
        guard !activado else { return }
        activado = true
        OjosDelAsistente.pintor = { lienzo, ctx, limites, imagen in
            PintorLienzo(lienzo: lienzo, imagen: imagen).pintar(en: ctx, limites: limites)
        }
    }
}
