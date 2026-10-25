// Una pagina de Word o PowerPoint maquetada por `pixpin-office` (Rust), leida
// de su bufer plano (`PaginaOffice`, formato en
// `pixpin-nucleo/src/office.rs`) y pintada con Core Graphics.
//
// Para leerla en el lector, cada pagina se pinta UNA vez en un PDF de una
// hoja (vectores: nitida a cualquier aumento, sin mapas de bits) y PDFKit
// hace lo demas (desplazar, acercar, buscar, elegir texto). Asi un Word es
// un PDF mas para el lector y la tinta va igual en los dos.

import CoreGraphics
import CoreText
import Foundation
import ImageIO
import PixPinCore

/// Una orden de la pagina, ya leida.
public enum OrdenOffice: Equatable {
    case texto(x: CGFloat, y: CGFloat, texto: Int, letra: Int, tam: CGFloat, color: [CGFloat],
               subrayado: Int, tachado: Bool, espaciado: CGFloat, extraEspacio: CGFloat, ancho: CGFloat)
    case rect(CGRect, relleno: [CGFloat]?, borde: [CGFloat]?, grosor: CGFloat, discontinuo: Int)
    case linea(CGPoint, CGPoint, color: [CGFloat], grosor: CGFloat, discontinuo: Int)
    case imagen(Int, CGRect, recorte: [CGFloat]?, opacidad: CGFloat)
    case trayecto(CGPath, relleno: [CGFloat]?, borde: [CGFloat]?, grosor: CGFloat, discontinuo: Int,
                  sombras: [(CGPath, Int)])
    case grupo(CGAffineTransform, recorte: CGRect?)
    case finDeGrupo

    public static func == (a: OrdenOffice, b: OrdenOffice) -> Bool {
        switch (a, b) {
        case let (.texto(x1, y1, t1, l1, _, _, _, _, _, _, _), .texto(x2, y2, t2, l2, _, _, _, _, _, _, _)):
            return x1 == x2 && y1 == y2 && t1 == t2 && l1 == l2
        case let (.rect(r1, _, _, _, _), .rect(r2, _, _, _, _)): return r1 == r2
        case let (.linea(a1, b1, _, _, _), .linea(a2, b2, _, _, _)): return a1 == a2 && b1 == b2
        case let (.imagen(i1, r1, _, _), .imagen(i2, r2, _, _)): return i1 == i2 && r1 == r2
        case let (.trayecto(p1, _, _, _, _, _), .trayecto(p2, _, _, _, _, _)): return p1 == p2
        case let (.grupo(t1, r1), .grupo(t2, r2)): return t1 == t2 && r1 == r2
        case (.finDeGrupo, .finDeGrupo): return true
        default: return false
        }
    }
}

public enum ErrorPaginaOffice: Error, Equatable {
    case version(Int)
    case corta
}

/// La version del bufer que se entiende (`VERSION_PAGINA_OFFICE`).
public let versionPaginaOffice = 1

/// **Lee el bufer de una pagina** en una pasada (una copia de memoria).
public func leerPaginaOffice(_ p: PaginaOffice) throws -> [OrdenOffice] {
    let n = p.datos.count / MemoryLayout<Float>.size
    var f = [Float](repeating: 0, count: n)
    _ = f.withUnsafeMutableBytes { p.datos.copyBytes(to: $0) }
    guard n >= 1 else { throw ErrorPaginaOffice.corta }
    guard Int(f[0]) == versionPaginaOffice else { throw ErrorPaginaOffice.version(Int(f[0])) }
    var i = 1
    func toma() throws -> CGFloat {
        guard i < n else { throw ErrorPaginaOffice.corta }
        defer { i += 1 }
        return CGFloat(f[i])
    }
    func color() throws -> [CGFloat] { [try toma(), try toma(), try toma(), try toma()] }
    func opcional() throws -> [CGFloat]? {
        let hay = try toma() != 0
        let c = try color()
        return hay ? c : nil
    }
    var v: [OrdenOffice] = []
    while i < n {
        let codigo = Int(try toma())
        switch codigo {
        case 1:
            let x = try toma(), y = try toma(), t = Int(try toma()), l = Int(try toma()), tam = try toma()
            let c = try color()
            let sub = Int(try toma()), tach = try toma() != 0
            let esp = try toma(), extra = try toma(), ancho = try toma()
            v.append(.texto(x: x, y: y, texto: t, letra: l, tam: tam, color: c, subrayado: sub,
                            tachado: tach, espaciado: esp, extraEspacio: extra, ancho: ancho))
        case 2:
            let r = CGRect(x: try toma(), y: try toma(), width: try toma(), height: try toma())
            let relleno = try opcional()
            let borde = try opcional()
            let grosor = try toma(), disc = Int(try toma())
            v.append(.rect(r, relleno: relleno, borde: borde, grosor: grosor, discontinuo: disc))
        case 3:
            let a = CGPoint(x: try toma(), y: try toma()), b = CGPoint(x: try toma(), y: try toma())
            let c = try color()
            v.append(.linea(a, b, color: c, grosor: try toma(), discontinuo: Int(try toma())))
        case 4:
            let idx = Int(try toma())
            let r = CGRect(x: try toma(), y: try toma(), width: try toma(), height: try toma())
            let hay = try toma() != 0
            let rec = [try toma(), try toma(), try toma(), try toma()]
            v.append(.imagen(idx, r, recorte: hay ? rec : nil, opacidad: try toma()))
        case 5:
            let figuras = Int(try toma())
            let relleno = try opcional()
            let borde = try opcional()
            let grosor = try toma(), disc = Int(try toma())
            let todo = CGMutablePath()
            var sombras: [(CGPath, Int)] = []
            for _ in 0..<max(0, figuras) {
                let cerrada = try toma() != 0
                _ = try toma() // rellenar: lo decide el relleno de la forma
                _ = try toma() // trazar
                let sombra = Int(try toma())
                let camino = CGMutablePath()
                camino.move(to: CGPoint(x: try toma(), y: try toma()))
                let tramos = Int(try toma())
                for _ in 0..<max(0, tramos) {
                    if try toma() == 0 {
                        camino.addLine(to: CGPoint(x: try toma(), y: try toma()))
                    } else {
                        let c1 = CGPoint(x: try toma(), y: try toma())
                        let c2 = CGPoint(x: try toma(), y: try toma())
                        camino.addCurve(to: CGPoint(x: try toma(), y: try toma()), control1: c1, control2: c2)
                    }
                }
                if cerrada { camino.closeSubpath() }
                if sombra == 0 {
                    todo.addPath(camino)
                } else {
                    sombras.append((camino, sombra))
                }
            }
            v.append(.trayecto(todo, relleno: relleno, borde: borde, grosor: grosor, discontinuo: disc,
                               sombras: sombras))
        case 6:
            let t = CGAffineTransform(a: try toma(), b: try toma(), c: try toma(), d: try toma(),
                                      tx: try toma(), ty: try toma())
            let hay = try toma() != 0
            let r = CGRect(x: try toma(), y: try toma(), width: try toma(), height: try toma())
            v.append(.grupo(t, recorte: hay ? r : nil))
        case 7:
            v.append(.finDeGrupo)
        default:
            // Una orden que no se conoce: no se sabe cuanto mide, se para.
            return v
        }
    }
    return v
}

/// Pinta una pagina en un contexto con la Y hacia ABAJO (el de la pagina).
public struct PintorOffice {
    public let pagina: PaginaOffice
    public let ordenes: [OrdenOffice]
    /// Los bytes de una imagen por su id (los pide al documento de Rust).
    public var imagen: (String) -> Data?

    public init(pagina: PaginaOffice, imagen: @escaping (String) -> Data? = { _ in nil }) throws {
        self.pagina = pagina
        ordenes = try leerPaginaOffice(pagina)
        self.imagen = imagen
    }

    private static func cg(_ c: [CGFloat]) -> CGColor {
        CGColor(srgbRed: c[0], green: c[1], blue: c[2], alpha: c[3])
    }

    private static func discontinuo(_ ctx: CGContext, _ d: Int, _ g: CGFloat) {
        let g = max(g, 0.5)
        switch d {
        case 1: ctx.setLineDash(phase: 0, lengths: [g * 4, g * 3])
        case 2: ctx.setLineDash(phase: 0, lengths: [g, g * 2])
        case 3: ctx.setLineDash(phase: 0, lengths: [g * 4, g * 2, g, g * 2])
        default: ctx.setLineDash(phase: 0, lengths: [])
        }
    }

    /// Pinta todo. `ctx` ya con la Y hacia abajo y en puntos de la pagina.
    public func pintar(en ctx: CGContext) {
        if let f = pagina.fondo, f.count == 4 {
            ctx.setFillColor(PintorOffice.cg(f.map { CGFloat($0) }))
            ctx.fill(CGRect(x: 0, y: 0, width: CGFloat(pagina.ancho), height: CGFloat(pagina.alto)))
        }
        var imagenes: [Int: CGImage] = [:]
        for o in ordenes {
            switch o {
            case let .texto(x, y, t, l, tam, color, sub, tach, esp, extra, ancho):
                guard t < pagina.textos.count, l < pagina.letras.count else { continue }
                texto(ctx, pagina.textos[t], pagina.letras[l], x: x, y: y, tam: tam, color: color,
                      espaciado: esp, extra: extra)
                if sub > 0 || tach {
                    ctx.saveGState()
                    ctx.setStrokeColor(PintorOffice.cg(color))
                    ctx.setLineWidth(max(tam / 18, 0.5) * (sub == 3 ? 2 : 1))
                    if sub > 0 {
                        PintorOffice.discontinuo(ctx, sub == 4 ? 2 : (sub == 5 ? 1 : 0), tam / 18)
                        ctx.move(to: CGPoint(x: x, y: y + tam * 0.12))
                        ctx.addLine(to: CGPoint(x: x + ancho, y: y + tam * 0.12))
                        if sub == 2 {
                            ctx.move(to: CGPoint(x: x, y: y + tam * 0.2))
                            ctx.addLine(to: CGPoint(x: x + ancho, y: y + tam * 0.2))
                        }
                        ctx.strokePath()
                    }
                    if tach {
                        ctx.setLineDash(phase: 0, lengths: [])
                        ctx.move(to: CGPoint(x: x, y: y - tam * 0.3))
                        ctx.addLine(to: CGPoint(x: x + ancho, y: y - tam * 0.3))
                        ctx.strokePath()
                    }
                    ctx.restoreGState()
                }
            case let .rect(r, relleno, borde, grosor, disc):
                if let relleno {
                    ctx.setFillColor(PintorOffice.cg(relleno))
                    ctx.fill(r)
                }
                if let borde, grosor > 0 {
                    ctx.saveGState()
                    ctx.setStrokeColor(PintorOffice.cg(borde))
                    ctx.setLineWidth(grosor)
                    PintorOffice.discontinuo(ctx, disc, grosor)
                    ctx.stroke(r)
                    ctx.restoreGState()
                }
            case let .linea(a, b, color, grosor, disc):
                ctx.saveGState()
                ctx.setStrokeColor(PintorOffice.cg(color))
                ctx.setLineWidth(max(grosor, 0.25))
                PintorOffice.discontinuo(ctx, disc, grosor)
                ctx.move(to: a)
                ctx.addLine(to: b)
                ctx.strokePath()
                ctx.restoreGState()
            case let .imagen(idx, r, recorte, opacidad):
                guard idx < pagina.imagenes.count else { continue }
                if imagenes[idx] == nil, let datos = imagen(pagina.imagenes[idx]),
                   let fuente = CGImageSourceCreateWithData(datos as CFData, nil) {
                    imagenes[idx] = CGImageSourceCreateImageAtIndex(fuente, 0, nil)
                }
                guard var bitmap = imagenes[idx] else { continue }
                if let rc = recorte {
                    let w = CGFloat(bitmap.width), h = CGFloat(bitmap.height)
                    let px = CGRect(x: rc[0] * w, y: rc[1] * h, width: w * (1 - rc[0] - rc[2]),
                                    height: h * (1 - rc[1] - rc[3]))
                    if px.width >= 1, px.height >= 1 { bitmap = bitmap.cropping(to: px.integral) ?? bitmap }
                }
                ctx.saveGState()
                ctx.setAlpha(opacidad)
                // En un contexto con la Y hacia abajo, la foto se voltea.
                ctx.translateBy(x: r.minX, y: r.maxY)
                ctx.scaleBy(x: 1, y: -1)
                ctx.interpolationQuality = .high
                ctx.draw(bitmap, in: CGRect(origin: .zero, size: r.size))
                ctx.restoreGState()
            case let .trayecto(camino, relleno, borde, grosor, disc, sombras):
                if let relleno {
                    ctx.addPath(camino)
                    ctx.setFillColor(PintorOffice.cg(relleno))
                    ctx.fillPath(using: .evenOdd)
                    for (s, clase) in sombras {
                        // Las caras del cubo y del bisel: mas oscuras o mas
                        // claras que el relleno.
                        let k: CGFloat = [1: 0.6, 2: 0.8, 3: 1.4, 4: 1.2][clase] ?? 1
                        let c = relleno.prefix(3).map { min(max($0 * k, 0), 1) } + [relleno[3]]
                        ctx.addPath(s)
                        ctx.setFillColor(PintorOffice.cg(Array(c)))
                        ctx.fillPath(using: .evenOdd)
                    }
                }
                if let borde, grosor > 0 {
                    ctx.saveGState()
                    ctx.addPath(camino)
                    for (s, _) in sombras { ctx.addPath(s) }
                    ctx.setStrokeColor(PintorOffice.cg(borde))
                    ctx.setLineWidth(grosor)
                    ctx.setLineJoin(.round)
                    PintorOffice.discontinuo(ctx, disc, grosor)
                    ctx.strokePath()
                    ctx.restoreGState()
                }
            case let .grupo(t, recorte):
                ctx.saveGState()
                ctx.concatenate(t)
                if let recorte { ctx.clip(to: recorte) }
            case .finDeGrupo:
                ctx.restoreGState()
            }
        }
    }

    /// Un trozo de texto con los mismos avances con que se maqueto
    /// (`MedidorCoreText`: los del glifo, sin ajuste entre pares ni
    /// ligaduras), mas el interletrado y el justificado de cada espacio. Va
    /// por `CTLine` para que el PDF lleve el texto (buscar, elegir, copiar).
    private func texto(_ ctx: CGContext, _ s: String, _ l: LetraOffice, x: CGFloat, y: CGFloat, tam: CGFloat,
                       color: [CGFloat], espaciado: CGFloat, extra: CGFloat) {
        guard tam > 0, !s.isEmpty else { return }
        let base = LetrasCoreText.compartidas.letra(familia: l.familia, negrita: l.negrita, cursiva: l.cursiva)
        let letra = CTFontCreateCopyWithAttributes(base, tam, nil, nil)
        let atributos: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): letra,
            NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String): true,
            // Un `kern` puesto (aunque sea 0) quita el ajuste entre pares.
            .kern: espaciado,
            .ligature: 0,
        ]
        let cadena = NSMutableAttributedString(string: s, attributes: atributos)
        if extra != 0 {
            let ns = s as NSString
            var i = 0
            while i < ns.length {
                let c = ns.character(at: i)
                if c == 0x20 || c == 0xA0 {
                    cadena.addAttribute(.kern, value: espaciado + extra, range: NSRange(location: i, length: 1))
                }
                i += 1
            }
        }
        let linea = CTLineCreateWithAttributedString(cadena)
        ctx.saveGState()
        ctx.setFillColor(PintorOffice.cg(color))
        // La Y del contexto va hacia abajo: el texto se dibuja volteado
        // alrededor de su linea base para que salga derecho.
        ctx.translateBy(x: x, y: y)
        ctx.scaleBy(x: 1, y: -1)
        ctx.textMatrix = .identity
        ctx.textPosition = .zero
        CTLineDraw(linea, ctx)
        ctx.restoreGState()
    }

    /// **Un PDF de una hoja con la pagina pintada** (vectores). `nil` si no
    /// se pudo crear.
    public func pdf() -> Data? {
        let datos = NSMutableData()
        var caja = CGRect(x: 0, y: 0, width: CGFloat(pagina.ancho), height: CGFloat(pagina.alto))
        guard caja.width > 0, caja.height > 0,
              let consumidor = CGDataConsumer(data: datos as CFMutableData),
              let ctx = CGContext(consumer: consumidor, mediaBox: &caja, nil)
        else { return nil }
        ctx.beginPDFPage(nil)
        ctx.saveGState()
        ctx.translateBy(x: 0, y: caja.height)
        ctx.scaleBy(x: 1, y: -1)
        pintar(en: ctx)
        ctx.restoreGState()
        ctx.endPDFPage()
        ctx.closePDF()
        return datos as Data
    }
}
