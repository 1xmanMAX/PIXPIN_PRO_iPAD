// **Lo que ensenan dentro el mosaico y la lupa**, y pintar una hoja entera
// del lienzo en cualquier `CGContext` (un mapa de bits, lo que se comparte o
// se imprime, la foto de una zona).
//
// Port del `TapadoLienzo.swift` del Mac (sin el fondo fijo, que en el iPad
// aun no hay): Rust pone en el plano, en su sitio del orden de pintado, una
// imagen con un `id_objeto` que no es de ninguna foto (`editor_tapado.rs`),
// y esta clase la resuelve: pide a Rust lo de dentro (`contenidoDeTapado`) y
// lo pinta con el mismo `PintorLienzo`:
//
// - el mosaico, en cuadros: la escena de debajo a un mapa de un pixel por
//   cuadro, agrandado sin suavizar (o suavizado, con «desenfoque»);
// - la lupa, agrandada: la escena otra vez, con la matriz de la lupa y
//   recortada a su cristal.
//
// Lo pintado se guarda por `id_objeto` y se rehace cuando cambia la version
// del editor o el zoom (la lupa).

import CoreGraphics
import Foundation
import ImageIO
import PixPinCore
import UniformTypeIdentifiers

@MainActor
final class PintorDeTapados {
    private let editor: EditorLienzo
    /// Las fotos del lienzo, para lo que se ve dentro.
    private let fotos: (UInt64) -> CGImage?
    private var hechos: [UInt64: (version: UInt64, lado: Int, imagen: CGImage)] = [:]

    /// El lado mas largo de lo pintado dentro de una lupa.
    static let ladoMaximo = 2048

    init(editor: EditorLienzo, fotos: @escaping (UInt64) -> CGImage?) {
        self.editor = editor
        self.fotos = fotos
    }

    /// Si `id` es de un mosaico o una lupa y no de una foto.
    nonisolated static func esTapado(_ id: UInt64) -> Bool { PintorLienzo.esTapado(id) }

    /// **La imagen de `id`** con la version `version` del editor y `escala`
    /// pixeles por unidad del mundo.
    func imagen(_ id: UInt64, version: UInt64, escala: CGFloat) -> CGImage? {
        let lado = max(1, Int((escala * 4).rounded()))
        if let h = hechos[id], h.version == version, h.lado == lado { return h.imagen }
        guard let c = editor.contenidoDeTapado(idObjeto: id),
              let img = PintorDeTapados.pintar(c, escala: escala, fotos: fotos) else {
            hechos[id] = nil
            return nil
        }
        hechos[id] = (version, lado, img)
        if hechos.count > 64 { hechos = hechos.filter { $0.value.version == version } }
        return img
    }

    /// Pinta lo de dentro de `c` (ver `ContenidoTapado` en Rust).
    static func pintar(_ c: ContenidoTapado, escala: CGFloat,
                       fotos: @escaping (UInt64) -> CGImage?) -> CGImage? {
        guard c.ancho > 0, c.alto > 0, let leido = try? leerLienzo(c.plano) else { return nil }
        let ancho = CGFloat(c.ancho), alto = CGFloat(c.alto)
        let pintor = PintorLienzo(lienzo: leido, imagen: fotos)
        if c.grano > 0 {
            // **El mosaico**: un pixel por cuadro, y despues agrandado.
            let cols = min(512, max(1, Int((ancho / CGFloat(c.grano)).rounded(.up))))
            let filas = min(512, max(1, Int((alto / CGFloat(c.grano)).rounded(.up))))
            guard let chico = MapaLienzo.mapa(cols, filas) else { return nil }
            chico.translateBy(x: 0, y: CGFloat(filas))
            chico.scaleBy(x: CGFloat(cols) / ancho, y: -CGFloat(filas) / alto)
            chico.translateBy(x: -CGFloat(c.x), y: -CGFloat(c.y))
            chico.setFillColor(leido.fondo.cg)
            chico.fill(CGRect(x: CGFloat(c.x), y: CGFloat(c.y), width: ancho, height: alto))
            let mundo = CGRect(x: CGFloat(c.x), y: CGFloat(c.y), width: ancho, height: alto)
            pintor.pintar(en: chico, limites: mundo)
            guard let cuadros = chico.makeImage() else { return nil }
            // Con «desenfoque», la mancha: lo chico estirado y suavizado.
            if c.desenfoque { return cuadros }
            let k = max(1, min(8, ladoMaximo / max(cols, filas)))
            guard let grande = MapaLienzo.mapa(cols * k, filas * k) else { return cuadros }
            grande.interpolationQuality = .none
            grande.draw(cuadros, in: CGRect(x: 0, y: 0, width: cols * k, height: filas * k))
            return grande.makeImage()
        }
        // **La lupa**: la escena con su matriz, recortada a su cristal.
        let k = min(escala, CGFloat(ladoMaximo) / max(ancho, alto))
        let w = max(1, Int((ancho * k).rounded())), h = max(1, Int((alto * k).rounded()))
        guard let ctx = MapaLienzo.mapa(w, h) else { return nil }
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: CGFloat(w) / ancho, y: -CGFloat(h) / alto)
        ctx.translateBy(x: -CGFloat(c.x), y: -CGFloat(c.y))
        var puntos: [CGPoint] = []
        var i = 0
        while i + 1 < c.contorno.count {
            puntos.append(CGPoint(x: CGFloat(c.contorno[i]), y: CGFloat(c.contorno[i + 1])))
            i += 2
        }
        if puntos.count >= 3 {
            ctx.addLines(between: puntos)
            ctx.closePath()
            ctx.clip()
        }
        let cristal = CGRect(x: CGFloat(c.x), y: CGFloat(c.y), width: ancho, height: alto)
        ctx.setFillColor(leido.fondo.cg)
        ctx.fill(cristal)
        // Un punto q del dibujo cae en centro + (q - foco) * aumento.
        let a = CGFloat(c.aumento)
        ctx.translateBy(x: cristal.midX, y: cristal.midY)
        ctx.scaleBy(x: a, y: a)
        ctx.translateBy(x: -CGFloat(c.focoX), y: -CGFloat(c.focoY))
        let mirado = CGRect(x: CGFloat(c.focoX) - ancho / a / 2, y: CGFloat(c.focoY) - alto / a / 2,
                            width: ancho / a, height: alto / a)
        pintor.pintar(en: ctx, limites: mirado)
        return ctx.makeImage()
    }
}

/// **Pintar una hoja del lienzo** (lo compartido, lo impreso, la foto de una
/// zona) en un `CGContext` cualquiera.
@MainActor
enum PintorDeHojas {
    /// Pinta `plano` con la caja del mundo `caja` llenando `destino` (en las
    /// unidades de `ctx`, con Y hacia abajo ya puesta por quien llama), con
    /// el papel debajo salvo `transparente`.
    static func pintar(_ plano: LienzoPlano, caja: CGRect, en ctx: CGContext, destino: CGRect,
                       transparente: Bool, tapados: PintorDeTapados?, version: UInt64,
                       fotos: @escaping (UInt64) -> CGImage?) {
        guard caja.width > 0, caja.height > 0, let leido = try? leerLienzo(plano) else { return }
        ctx.saveGState()
        ctx.clip(to: destino)
        if !transparente {
            ctx.setFillColor(leido.fondo.cg)
            ctx.fill(destino)
        }
        let k = min(destino.width / caja.width, destino.height / caja.height)
        ctx.translateBy(x: destino.midX - caja.midX * k, y: destino.midY - caja.midY * k)
        ctx.scaleBy(x: k, y: k)
        let escala = max(2, k)
        PintorLienzo(lienzo: leido) { id in
            // La foto primero: su id puede tener los bits del mosaico.
            if let f = fotos(id) { return f }
            if PintorDeTapados.esTapado(id) { return tapados?.imagen(id, version: version, escala: escala) }
            return nil
        }.pintar(en: ctx, limites: caja)
        ctx.restoreGState()
    }

    /// **La hoja como imagen** a `escala` pixeles por unidad (como mucho
    /// 8192 de lado, el `LADO_MAXIMO_PNG` de Windows).
    static func imagen(_ plano: LienzoPlano, caja: CGRect, escala: CGFloat, transparente: Bool,
                       tapados: PintorDeTapados?, version: UInt64,
                       fotos: @escaping (UInt64) -> CGImage?) -> CGImage? {
        let k = min(escala, 8192 / max(caja.width, caja.height, 1))
        let w = max(1, Int((caja.width * k).rounded())), h = max(1, Int((caja.height * k).rounded()))
        guard let ctx = MapaLienzo.mapa(w, h) else { return nil }
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: 1, y: -1)
        pintar(plano, caja: caja, en: ctx, destino: CGRect(x: 0, y: 0, width: w, height: h),
               transparente: transparente, tapados: tapados, version: version, fotos: fotos)
        return ctx.makeImage()
    }

    /// Una imagen como PNG.
    nonisolated static func png(_ img: CGImage) -> Data? {
        let datos = NSMutableData()
        guard let d = CGImageDestinationCreateWithData(datos, UTType.png.identifier as CFString, 1, nil) else {
            return nil
        }
        CGImageDestinationAddImage(d, img, nil)
        return CGImageDestinationFinalize(d) ? datos as Data : nil
    }

    /// **Recortada en redondo y con su filo** (`zona::en_redondo_y_con_filo`
    /// de Windows): las esquinas redondas y un filo dentro, negro sobre papel
    /// claro y blanco sobre papel de noche.
    static func enRedondo(_ img: CGImage, papelDeNoche: Bool) -> CGImage? {
        let w = img.width, h = img.height
        guard let ctx = MapaLienzo.mapa(w, h) else { return nil }
        let corto = CGFloat(min(w, h))
        let radio = min(36, max(10, corto * 0.05))
        let filo = min(5, max(2, corto * 0.006))
        let caja = CGRect(x: 0, y: 0, width: w, height: h)
        let forma = CGPath(roundedRect: caja, cornerWidth: radio, cornerHeight: radio, transform: nil)
        ctx.addPath(forma)
        ctx.clip()
        ctx.draw(img, in: caja)
        ctx.addPath(forma)
        ctx.setLineWidth(filo * 2)
        ctx.setStrokeColor(papelDeNoche ? CGColor(gray: 1, alpha: 1)
                           : CGColor(red: 30 / 255, green: 30 / 255, blue: 30 / 255, alpha: 1))
        ctx.strokePath()
        return ctx.makeImage()
    }
}
