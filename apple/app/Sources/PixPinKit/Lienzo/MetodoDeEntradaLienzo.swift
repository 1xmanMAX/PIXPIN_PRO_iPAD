// **El metodo de entrada en el texto del lienzo** (japones, chino, coreano,
// el acento mantenido, el dictado): `UITextInput`, como el
// `NSTextInputClient` del Mac (`EditorLienzoView.swift`). Lo que se compone
// se pinta subrayado junto al cursor y la ventanita de candidatos del
// sistema sale **en su sitio**, junto al cursor del texto
// (`EditorLienzo.cajaDelCursor`), no en una esquina. Al confirmarse entra en
// el texto con `teclear`, como lo tecleado.
//
// El «documento» de `UITextInput` es solo lo que se compone: el texto de
// verdad vive en el motor de Rust, que es quien lo edita.

import PixPinCore
import UIKit

/// Una posicion dentro de lo que se compone (en unidades UTF-16).
final class PosicionLienzo: UITextPosition {
    let i: Int
    init(_ i: Int) { self.i = i }
}

/// Un tramo de lo que se compone.
final class RangoLienzo: UITextRange {
    let desde: PosicionLienzo
    let hasta: PosicionLienzo
    init(_ a: Int, _ b: Int) {
        desde = PosicionLienzo(min(a, b))
        hasta = PosicionLienzo(max(a, b))
    }

    override var start: UITextPosition { desde }
    override var end: UITextPosition { hasta }
    override var isEmpty: Bool { desde.i == hasta.i }
}

extension VistaEditorLienzoIPad: UITextInput {
    private var largoMarcado: Int { (marcado as NSString).length }

    private func indice(_ p: UITextPosition) -> Int {
        min(max((p as? PosicionLienzo)?.i ?? largoMarcado, 0), largoMarcado)
    }

    // MARK: Lo que se compone

    public var markedTextRange: UITextRange? {
        marcado.isEmpty ? nil : RangoLienzo(0, largoMarcado)
    }

    public func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
        marcado = markedText ?? ""
        setNeedsDisplay()
    }

    /// Se confirma lo compuesto: entra en el texto.
    public func unmarkText() {
        let texto = marcado
        marcado = ""
        setNeedsDisplay()
        guard !texto.isEmpty, modelo.editor.escribiendo() else { return }
        if modelo.editor.teclear(texto: texto) { trasCambio() }
    }

    public var selectedTextRange: UITextRange? {
        get { RangoLienzo(largoMarcado, largoMarcado) }
        set {}
    }

    public func text(in range: UITextRange) -> String? {
        let a = indice(range.start), b = indice(range.end)
        guard a <= b else { return nil }
        return (marcado as NSString).substring(with: NSRange(location: a, length: b - a))
    }

    public func replace(_ range: UITextRange, withText text: String) {
        insertText(text)
    }

    // MARK: Posiciones

    public var beginningOfDocument: UITextPosition { PosicionLienzo(0) }
    public var endOfDocument: UITextPosition { PosicionLienzo(largoMarcado) }

    public func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
        RangoLienzo(indice(fromPosition), indice(toPosition))
    }

    public func position(from position: UITextPosition, offset: Int) -> UITextPosition? {
        let i = indice(position) + offset
        return (0 ... largoMarcado).contains(i) ? PosicionLienzo(i) : nil
    }

    public func position(from position: UITextPosition, in direction: UITextLayoutDirection,
                         offset: Int) -> UITextPosition? {
        switch direction {
        case .left, .up: return self.position(from: position, offset: -offset)
        default: return self.position(from: position, offset: offset)
        }
    }

    public func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
        let a = indice(position), b = indice(other)
        return a < b ? .orderedAscending : (a > b ? .orderedDescending : .orderedSame)
    }

    public func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {
        indice(toPosition) - indice(from)
    }

    public var tokenizer: UITextInputTokenizer { tokenizadorDeEntrada }

    public func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
        switch direction {
        case .left, .up: return range.start
        default: return range.end
        }
    }

    public func characterRange(byExtending position: UITextPosition,
                               in direction: UITextLayoutDirection) -> UITextRange? {
        let i = indice(position)
        switch direction {
        case .left, .up: return RangoLienzo(0, i)
        default: return RangoLienzo(i, largoMarcado)
        }
    }

    public func baseWritingDirection(for position: UITextPosition,
                                     in direction: UITextStorageDirection) -> NSWritingDirection {
        .natural
    }

    public func setBaseWritingDirection(_ writingDirection: NSWritingDirection, for range: UITextRange) {}

    // MARK: Donde se ve (la ventanita de candidatos)

    /// **El cursor del texto en la vista**: su raya en el mundo, pasada a la
    /// vista; sin cursor, el centro.
    var rectanguloDelCursor: CGRect {
        guard let c = modelo.editor.cajaDelCursor() else {
            return CGRect(x: bounds.midX, y: bounds.midY, width: 2, height: 20)
        }
        let x0 = (CGFloat(c.x0) - origen.x) * zoom, y0 = (CGFloat(c.y0) - origen.y) * zoom
        let x1 = (CGFloat(c.x1) - origen.x) * zoom, y1 = (CGFloat(c.y1) - origen.y) * zoom
        return CGRect(x: min(x0, x1), y: min(y0, y1), width: max(2, abs(x1 - x0)), height: max(8, abs(y1 - y0)))
    }

    public func firstRect(for range: UITextRange) -> CGRect {
        let c = rectanguloDelCursor
        let ancho = marcadoMedido.width
        return CGRect(x: c.minX, y: c.minY, width: max(c.width, ancho), height: c.height)
    }

    public func caretRect(for position: UITextPosition) -> CGRect {
        let c = rectanguloDelCursor
        return CGRect(x: c.minX + marcadoMedido.width, y: c.minY, width: 2, height: c.height)
    }

    public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] { [] }

    public func closestPosition(to point: CGPoint) -> UITextPosition? { endOfDocument }

    public func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? { range.end }

    public func characterRange(at point: CGPoint) -> UITextRange? { nil }

    // MARK: Pintarlo

    /// Los atributos con que se ensena lo que se compone: del alto del
    /// renglon del cursor, subrayado.
    private var atributosDelMarcado: [NSAttributedString.Key: Any] {
        let alto = rectanguloDelCursor.height
        return [
            .font: UIFont.systemFont(ofSize: max(10, min(60, alto * 0.75))),
            .foregroundColor: UIColor.label,
            .underlineStyle: NSUnderlineStyle.single.rawValue,
            .backgroundColor: UIColor.systemBackground.withAlphaComponent(0.85),
        ]
    }

    private var marcadoMedido: CGSize {
        marcado.isEmpty ? .zero : (marcado as NSString).size(withAttributes: atributosDelMarcado)
    }

    /// Lo que se compone, subrayado junto al cursor.
    func pintarMarcado(_ ctx: CGContext) {
        guard !marcado.isEmpty else { return }
        let c = rectanguloDelCursor
        let tam = marcadoMedido
        (marcado as NSString).draw(at: CGPoint(x: c.minX + 1, y: c.midY - tam.height / 2),
                                   withAttributes: atributosDelMarcado)
    }
}
