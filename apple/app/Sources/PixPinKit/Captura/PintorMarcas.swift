// Lo que se pinta encima de una captura (las herramientas de la barra de
// Windows, en su orden: lapiz, flecha, rectangulo, texto y mosaico) y como se
// pinta. El mismo dibujo sirve para la vista del editor (dentro de un
// `Canvas`) y para **quemarlo en la imagen** al acabar (un
// `UIGraphicsImageRenderer` a los pixeles de la foto), asi lo que sale es
// exactamente lo que se ve.
//
// Las marcas van en pixeles de la foto, no de la pantalla: el editor
// convierte el dedo (o el Apple Pencil) a la foto al dibujar.

import CoreGraphics
import CoreImage
import CoreImage.CIFilterBuiltins
import UIKit

/// Las herramientas, en el orden de Windows (1 a 5 con el teclado).
enum HerramientaCaptura: Int, CaseIterable, Identifiable {
    case lapiz, flecha, rectangulo, texto, mosaico

    var id: Int { rawValue }

    var nombre: String {
        switch self {
        case .lapiz: "Lápiz"
        case .flecha: "Flecha"
        case .rectangulo: "Rectángulo"
        case .texto: "Texto"
        case .mosaico: "Mosaico"
        }
    }

    var icono: String {
        switch self {
        case .lapiz: "pencil"
        case .flecha: "arrow.right"
        case .rectangulo: "square"
        case .texto: "textformat"
        case .mosaico: "eye.slash"
        }
    }

    /// La tecla del teclado del iPad (la cifra de Windows).
    var tecla: Character { Character("\(rawValue + 1)") }
}

/// Los colores de la barra de Windows (rojo, amarillo, verde, azul, negro).
enum ColoresCaptura {
    static let todos: [UInt32] = [0xFF3B30, 0xFFCC00, 0x34C759, 0x0A84FF, 0x1C1C1E]

    static func uiColor(_ hex: UInt32) -> UIColor {
        UIColor(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
                blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
    }
}

/// Los tres grosores de la barra, en puntos de pantalla.
enum GrosoresCaptura {
    static let todos: [CGFloat] = [3, 6, 10]
}

/// Una marca encima de la foto, en pixeles de la foto.
struct MarcaCaptura: Identifiable, Equatable {
    let id: UUID
    var herramienta: HerramientaCaptura
    var color: UInt32
    /// En pixeles de la foto.
    var grosor: CGFloat
    /// El trazo del lapiz; en las demas, el primero y el ultimo cuentan.
    var puntos: [CGPoint]
    var texto: String = ""

    init(herramienta: HerramientaCaptura, color: UInt32, grosor: CGFloat, puntos: [CGPoint], texto: String = "") {
        id = UUID()
        self.herramienta = herramienta
        self.color = color
        self.grosor = grosor
        self.puntos = puntos
        self.texto = texto
    }

    /// El rectangulo del primero al ultimo punto (rectangulo y mosaico).
    var caja: CGRect {
        guard let a = puntos.first, let b = puntos.last else { return .null }
        return CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
    }

    /// Si vale la pena guardarla (un toque suelto con la flecha no).
    var tieneCuerpo: Bool {
        switch herramienta {
        case .lapiz:
            return !puntos.isEmpty
        case .texto:
            return !texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case .flecha:
            guard let a = puntos.first, let b = puntos.last else { return false }
            return hypot(b.x - a.x, b.y - a.y) >= 4
        case .rectangulo, .mosaico:
            return caja.width >= 3 && caja.height >= 3
        }
    }
}

enum PintorMarcas {
    /// La letra del texto: la de la barra por cinco (pequena, mediana,
    /// grande), en pixeles de la foto.
    static func tamanoLetra(grosor: CGFloat) -> CGFloat {
        max(10, grosor * 5)
    }

    /// Pinta `marcas` en `cg` (coordenadas de la foto, con el origen arriba
    /// a la izquierda, como UIKit). `pixelada` es la foto entera ya hecha
    /// mosaico: cada marca de mosaico ensena su trozo.
    static func pintar(_ marcas: [MarcaCaptura], en cg: CGContext, tamano: CGSize, pixelada: CGImage?) {
        UIGraphicsPushContext(cg)
        defer { UIGraphicsPopContext() }
        for m in marcas {
            let color = ColoresCaptura.uiColor(m.color)
            cg.saveGState()
            cg.setLineCap(.round)
            cg.setLineJoin(.round)
            cg.setLineWidth(m.grosor)
            cg.setStrokeColor(color.cgColor)
            cg.setFillColor(color.cgColor)
            switch m.herramienta {
            case .lapiz:
                if m.puntos.count == 1, let p = m.puntos.first {
                    cg.fillEllipse(in: CGRect(x: p.x - m.grosor / 2, y: p.y - m.grosor / 2, width: m.grosor, height: m.grosor))
                } else if let primero = m.puntos.first {
                    cg.beginPath()
                    cg.move(to: primero)
                    // Curvas por los puntos medios: el trazo sale suave.
                    for i in 1 ..< m.puntos.count {
                        let a = m.puntos[i - 1], b = m.puntos[i]
                        let medio = CGPoint(x: (a.x + b.x) / 2, y: (a.y + b.y) / 2)
                        cg.addQuadCurve(to: medio, control: a)
                    }
                    if let ultimo = m.puntos.last { cg.addLine(to: ultimo) }
                    cg.strokePath()
                }
            case .flecha:
                guard let a = m.puntos.first, let b = m.puntos.last else { break }
                pintarFlecha(desde: a, hasta: b, grosor: m.grosor, en: cg)
            case .rectangulo:
                let r = m.caja
                guard !r.isNull else { break }
                cg.stroke(r)
            case .texto:
                guard let p = m.puntos.first else { break }
                let letra = UIFont.systemFont(ofSize: tamanoLetra(grosor: m.grosor), weight: .semibold)
                let texto = NSAttributedString(string: m.texto, attributes: [.font: letra, .foregroundColor: color])
                texto.draw(at: p)
            case .mosaico:
                let r = m.caja
                guard !r.isNull else { break }
                cg.clip(to: r)
                if let pixelada {
                    UIImage(cgImage: pixelada).draw(in: CGRect(origin: .zero, size: tamano))
                } else {
                    cg.setFillColor(UIColor.gray.cgColor)
                    cg.fill(r)
                }
            }
            cg.restoreGState()
        }
    }

    /// La linea y la punta rellena (la punta crece con el grosor).
    static func pintarFlecha(desde a: CGPoint, hasta b: CGPoint, grosor: CGFloat, en cg: CGContext) {
        let largo = hypot(b.x - a.x, b.y - a.y)
        guard largo > 0 else { return }
        let punta = min(max(grosor * 4, 12), largo * 0.6)
        let angulo = atan2(b.y - a.y, b.x - a.x)
        let abertura: CGFloat = .pi / 7
        let izq = CGPoint(x: b.x - punta * cos(angulo - abertura), y: b.y - punta * sin(angulo - abertura))
        let der = CGPoint(x: b.x - punta * cos(angulo + abertura), y: b.y - punta * sin(angulo + abertura))
        // La linea acaba en la base de la punta (si no, asoma por delante).
        let base = CGPoint(x: b.x - punta * 0.8 * cos(angulo), y: b.y - punta * 0.8 * sin(angulo))
        cg.beginPath()
        cg.move(to: a)
        cg.addLine(to: base)
        cg.strokePath()
        cg.beginPath()
        cg.move(to: b)
        cg.addLine(to: izq)
        cg.addLine(to: der)
        cg.closePath()
        cg.fillPath()
    }

    /// **Quema** las marcas en la foto: una imagen nueva a sus pixeles.
    static func quemar(_ base: CGImage, marcas: [MarcaCaptura], pixelada: CGImage?) -> CGImage {
        guard !marcas.isEmpty else { return base }
        let tamano = CGSize(width: base.width, height: base.height)
        let formato = UIGraphicsImageRendererFormat()
        formato.scale = 1
        formato.opaque = false
        let hecha = UIGraphicsImageRenderer(size: tamano, format: formato).image { ctx in
            UIImage(cgImage: base).draw(in: CGRect(origin: .zero, size: tamano))
            pintar(marcas, en: ctx.cgContext, tamano: tamano, pixelada: pixelada)
        }
        return hecha.cgImage ?? base
    }

    /// El lado de cada cuadro del mosaico: unos 60 cuadros a lo largo del
    /// lado mayor, y nunca menos de 8 pixeles.
    static func ladoMosaico(_ tamano: CGSize) -> CGFloat {
        max(8, (max(tamano.width, tamano.height) / 60).rounded())
    }

    /// La foto entera hecha mosaico (CoreImage, `CIPixellate`).
    static func pixelar(_ base: CGImage) -> CGImage? {
        let entrada = CIImage(cgImage: base)
        let filtro = CIFilter.pixellate()
        filtro.inputImage = entrada.clampedToExtent()
        filtro.scale = Float(ladoMosaico(CGSize(width: base.width, height: base.height)))
        filtro.center = .zero
        guard let salida = filtro.outputImage?.cropped(to: entrada.extent) else { return nil }
        return CIContext(options: [.cacheIntermediates: false]).createCGImage(salida, from: entrada.extent)
    }
}
