// **Las hojas en negativo (N) y en oscuro (luna/sol)**, como el lector de
// Windows (`lector_pdf.rs`, `pixpin_render::oscuro`):
//
// - **Negativo**: la hoja y sus colores al reves, como en los pines.
// - **Oscuro**: la luz invertida y el **tono conservado** (el `invert(1)
//   hue-rotate(180deg)` del movil): lo blanco pasa a un gris muy oscuro, lo
//   negro a un gris muy claro, y una raya roja sigue roja.
//
// Solo cambian las hojas: la tinta conserva sus colores y se adapta al
// papel oscuro como sobre cualquier papel (`tintaSobrePapel`, la cuenta del
// motor). Lo pinta la propia hoja (`HojaLector`, la subclase de `PDFPage`
// que da el delegado del documento) con modos de fusion de Quartz: el
// oscuro es invertir con la diferencia contra el gris claro y devolver el
// tono con la fusion `.hue` de la hoja de siempre.

import UIKit
import PDFKit
import PixPinCore

/// Como se ven las hojas de un documento.
public struct TonoHojas: Equatable, Sendable {
    public var negativo = false
    public var oscuro = false

    public init(negativo: Bool = false, oscuro: Bool = false) {
        self.negativo = negativo
        self.oscuro = oscuro
    }

    /// El papel que queda (contra el se adapta la tinta), o `nil` si es el
    /// blanco de siempre.
    public var papel: ColorLienzo? {
        switch (oscuro, negativo) {
        case (false, false): nil
        case (false, true): ColorLienzo(r: 0, g: 0, b: 0, a: 1)
        case (true, false): TonoHojas.papelOscuro
        // El negativo del oscuro vuelve a ser claro.
        case (true, true): ColorLienzo(r: 1 - TonoHojas.papelOscuro.r, g: 1 - TonoHojas.papelOscuro.g,
                                       b: 1 - TonoHojas.papelOscuro.b, a: 1)
        }
    }

    /// Lo blanco del documento en oscuro (`oscuro::PAPEL` de Windows).
    static let papelOscuro = ColorLienzo(r: 0x1F / 255, g: 0x1F / 255, b: 0x22 / 255, a: 1)
}

/// **El tono de cada documento abierto**, que la hoja mira al pintarse.
/// PDFKit pinta las hojas en sus propios hilos: va con cerrojo.
final class TonosDeDocumentos: @unchecked Sendable {
    static let compartidos = TonosDeDocumentos()
    private let cerrojo = NSLock()
    private var tonos: [ObjectIdentifier: TonoHojas] = [:]

    func tono(de doc: PDFDocument?) -> TonoHojas {
        guard let doc else { return TonoHojas() }
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return tonos[ObjectIdentifier(doc)] ?? TonoHojas()
    }

    func poner(_ t: TonoHojas, a doc: PDFDocument) {
        cerrojo.lock()
        tonos[ObjectIdentifier(doc)] = t == TonoHojas() ? nil : t
        cerrojo.unlock()
    }

    func olvidar(_ doc: PDFDocument) {
        cerrojo.lock()
        tonos[ObjectIdentifier(doc)] = nil
        cerrojo.unlock()
    }
}

/// **Una hoja del lector**: la de siempre, en negativo o en oscuro.
final class HojaLector: PDFPage {
    override func draw(with box: PDFDisplayBox, to context: CGContext) {
        let t = TonosDeDocumentos.compartidos.tono(de: document)
        guard t.negativo || t.oscuro else {
            super.draw(with: box, to: context)
            return
        }
        // Antes de transformar, el contexto es la hoja tal como se ve
        // (girada), con el cero abajo a la izquierda.
        let b = bounds(for: box)
        let girada = rotation % 180 != 0
        let hoja = CGRect(x: 0, y: 0, width: girada ? b.height : b.width, height: girada ? b.width : b.height)
        HojaLector.pintar(t, en: context, hoja: hoja) { super.draw(with: box, to: $0) }
    }

    /// Pinta `contenido` en `ctx` con el tono `t` dentro de `hoja`. Aparte
    /// para probarlo sin PDFKit.
    static func pintar(_ t: TonoHojas, en ctx: CGContext, hoja: CGRect, contenido: (CGContext) -> Void) {
        ctx.saveGState()
        // Un papel debajo: lo transparente de la hoja tambien se invierte.
        ctx.setFillColor(CGColor(gray: 1, alpha: 1))
        ctx.fill(hoja)
        contenido(ctx)
        if t.oscuro {
            // Lo blanco queda en el papel oscuro y lo negro en su contrario
            // (casi la tinta clara de Windows).
            let c = TonoHojas.papelOscuro
            ctx.setBlendMode(.difference)
            ctx.setFillColor(CGColor(srgbRed: 1 - c.r, green: 1 - c.g, blue: 1 - c.b, alpha: 1))
            ctx.fill(hoja)
            // El tono de antes, encima de la luz invertida.
            ctx.setBlendMode(.hue)
            ctx.beginTransparencyLayer(auxiliaryInfo: nil)
            ctx.setBlendMode(.normal)
            contenido(ctx)
            ctx.endTransparencyLayer()
        }
        if t.negativo {
            ctx.setBlendMode(.difference)
            ctx.setFillColor(CGColor(gray: 1, alpha: 1))
            ctx.fill(hoja)
        }
        ctx.restoreGState()
    }
}

/// El delegado que hace que las hojas de un documento sean `HojaLector`.
final class DelegadoHojas: NSObject, PDFDocumentDelegate {
    static let compartido = DelegadoHojas()

    func classForPage() -> AnyClass { HojaLector.self }
}

extension LienzoLeido {
    /// **La tinta sobre otro papel**: cada color, adaptado como lo adapta el
    /// motor (`contraste::adaptar`). `cache` guarda lo ya calculado.
    func sobrePapel(_ papel: ColorLienzo, cache: inout [ColorLienzo: ColorLienzo]) -> LienzoLeido {
        let p: [Float] = [Float(papel.r), Float(papel.g), Float(papel.b)]
        func c(_ x: ColorLienzo) -> ColorLienzo {
            if let y = cache[x] { return y }
            let v = tintaSobrePapel(color: [Float(x.r), Float(x.g), Float(x.b), Float(x.a)], papel: p)
            let y = v.count == 4 ? ColorLienzo(r: CGFloat(v[0]), g: CGFloat(v[1]), b: CGFloat(v[2]), a: CGFloat(v[3])) : x
            cache[x] = y
            return y
        }
        var otro = self
        otro.figuras = figuras.map { f in
            switch f {
            case let .relleno(path, col): .relleno(path, c(col))
            case let .tinta(path, col): .tinta(path, c(col))
            case let .linea(path, col, grosor, estilo): .linea(path, c(col), grosor: grosor, estilo: estilo)
            case let .texto(t, origen, tam, anchoMax, familia, negrita, cursiva, col):
                .texto(t, origen: origen, tam: tam, anchoMax: anchoMax, familia: familia, negrita: negrita,
                       cursiva: cursiva, c(col))
            case let .rotulo(t, origen, tam, familia, col, halo, grosorHalo, centro, angulo):
                .rotulo(t, origen: origen, tam: tam, familia: familia, c(col), halo: c(halo),
                        grosorHalo: grosorHalo, centro: centro, angulo: angulo)
            case .velo, .imagen: f
            }
        }
        return otro
    }
}

extension ColorLienzo: Hashable {
    public func hash(into h: inout Hasher) {
        h.combine(r)
        h.combine(g)
        h.combine(b)
        h.combine(a)
    }
}
