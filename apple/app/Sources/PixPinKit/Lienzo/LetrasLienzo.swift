// **Las ocho letras del lienzo** (`FUENTES` de `pixpin-motor2d/src/texto.rs`,
// las de `crates/pixpin-render/letras` de Windows): Excalifont, Nunito,
// Lilita One, Comic Shanns, Work Sans, Fraunces, Courier New y Caveat.
//
// Las siete que el iPad no trae van dentro de la app (`xcode/App/Fuentes`,
// apuntadas en `UIAppFonts`); Courier New ya viene con iOS. Por si la app se
// monto sin esa clave (o en las pruebas), se registran tambien aqui con Core
// Text la primera vez que se pinta un texto.
//
// Como Windows (`pixpin_render::letras`) y como el SVG del motor, un texto
// de una letra del catalogo se pinta **renglon a renglon con el interlineado
// fijo de su familia** y la letra centrada en su renglon (`Fuente::linea_base`
// del motor): asi cae en el mismo sitio que en Windows y en el movil, y el
// cursor del motor coincide con lo pintado. Lo que no es del catalogo (la
// letra del sistema de los rotulos, «Segoe UI»...) sigue con la letra del
// sistema y el interlineado de su fuente.
//
// La negrita y la cursiva: la cara de verdad si la familia la trae (Nunito y
// Caveat traen negrita; Courier New, todas) y si no, de mentira, como hace
// DirectWrite (`DWRITE_FONT_SIMULATIONS`): un trazo alrededor de la letra
// para la negrita y la letra inclinada para la cursiva.
//
// Se puede usar en cualquier hilo (las miniaturas se pintan fuera del
// principal): Core Text lo es y la cache va con su cerrojo.

import CoreGraphics
import CoreText
import Foundation
import UIKit

public enum LetrasLienzo {
    /// Una letra del catalogo con sus medidas, en veces el tamano de letra
    /// (las mismas cifras que `FUENTES` del motor).
    public struct Letra: Equatable, Sendable {
        /// El nombre de la familia, el que guarda `Figura::Texto::familia`.
        public let nombre: String
        /// La distancia entre renglones (`lineHeight` de Excalidraw).
        public let interlineado: CGFloat
        public let ascenso: CGFloat
        public let descenso: CGFloat

        /// **Donde cae la linea base del primer renglon** desde arriba de la
        /// caja: la letra centrada en su renglon (`Fuente::linea_base`).
        public var lineaBase: CGFloat { ascenso + (interlineado - (ascenso + descenso)) / 2 }
    }

    /// Las ocho, en el orden del panel.
    public static let catalogo: [Letra] = [
        Letra(nombre: "Excalifont", interlineado: 1.25, ascenso: 0.886, descenso: 0.374),
        Letra(nombre: "Nunito", interlineado: 1.25, ascenso: 1.011, descenso: 0.353),
        Letra(nombre: "Lilita One", interlineado: 1.15, ascenso: 0.923, descenso: 0.22),
        Letra(nombre: "Comic Shanns", interlineado: 1.25, ascenso: 1.167, descenso: 0.564),
        Letra(nombre: "Work Sans", interlineado: 1.25, ascenso: 0.93, descenso: 0.243),
        Letra(nombre: "Fraunces", interlineado: 1.25, ascenso: 0.978, descenso: 0.255),
        Letra(nombre: "Courier New", interlineado: 1.25, ascenso: 0.833, descenso: 0.3),
        Letra(nombre: "Caveat", interlineado: 1.25, ascenso: 0.96, descenso: 0.3),
    ]

    /// **Los ficheros de las letras dentro de la app**: lo que va en la
    /// clave `UIAppFonts` del `Info.plist`.
    public static let ficheros = [
        "Excalifont-Regular.ttf", "Nunito-Regular.ttf", "Nunito-Bold.ttf", "LilitaOne-Regular.ttf",
        "ComicShanns-Regular.ttf", "WorkSans-Regular.ttf", "Fraunces-SemiBold.ttf",
        "Caveat-Medium.ttf", "Caveat-Bold.ttf",
    ]

    /// La letra del catalogo con ese nombre, sin distinguir mayusculas
    /// (`fuente_por_nombre`): «Comic Shanns 2» es como la llamaba el movil
    /// en sus primeras versiones. `nil` si no es del catalogo.
    public static func letra(de familia: String) -> Letra? {
        var n = familia.trimmingCharacters(in: .whitespaces)
        if n.caseInsensitiveCompare("Comic Shanns 2") == .orderedSame { n = "Comic Shanns" }
        return catalogo.first { $0.nombre.caseInsensitiveCompare(n) == .orderedSame }
    }

    // MARK: Registrar y crear las letras

    private static let cerrojo = NSLock()
    nonisolated(unsafe) private static var registradas = false
    nonisolated(unsafe) private static var hechas: [String: Cara] = [:]

    /// Una letra ya creada, con lo que hay que fingir.
    struct Cara {
        let fuente: CTFont
        /// No hay cara negrita: se pinta con un trazo alrededor.
        let falsaNegrita: Bool
    }

    /// Registra en este proceso las letras de la app que el sistema aun no
    /// conozca (una vez). Con `UIAppFonts` puesto, ya estan y no hace nada.
    public static func registrar() {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        registrarSinCerrojo()
    }

    private static func registrarSinCerrojo() {
        guard !registradas else { return }
        registradas = true
        for f in ficheros {
            let tronco = (f as NSString).deletingPathExtension
            // El nombre PostScript es el del fichero: si ya responde, esta.
            let ya = CTFontCreateWithName(tronco as CFString, 12, nil)
            if (CTFontCopyPostScriptName(ya) as String) == tronco { continue }
            let url = Bundle.main.url(forResource: tronco, withExtension: "ttf")
                ?? Bundle.main.url(forResource: tronco, withExtension: "ttf", subdirectory: "Fuentes")
            if let url { CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil) }
        }
    }

    /// Si la familia esta en este iPad (instalada o de la app).
    public static func existe(_ familia: String) -> Bool {
        registrar()
        let d = CTFontDescriptorCreateWithAttributes([kCTFontFamilyNameAttribute: familia] as CFDictionary)
        let f = CTFontCreateWithFontDescriptor(d, 12, nil)
        return (CTFontCopyFamilyName(f) as String).caseInsensitiveCompare(familia) == .orderedSame
    }

    /// **La letra para pintar** `familia` a `tam` con su cara. Lo que no esta
    /// en este iPad cae en la del sistema.
    static func cara(familia: String, tam: CGFloat, negrita: Bool, cursiva: Bool) -> Cara {
        let nombre = letra(de: familia)?.nombre ?? familia
        let clave = "\(nombre)|\(tam)|\(negrita)|\(cursiva)"
        cerrojo.lock()
        defer { cerrojo.unlock() }
        if let c = hechas[clave] { return c }
        registrarSinCerrojo()
        let d = CTFontDescriptorCreateWithAttributes([kCTFontFamilyNameAttribute: nombre] as CFDictionary)
        var base = CTFontCreateWithFontDescriptor(d, tam, nil)
        if (CTFontCopyFamilyName(base) as String).caseInsensitiveCompare(nombre) != .orderedSame {
            base = CTFontCreateUIFontForLanguage(.system, tam, nil) ?? CTFontCreateWithName("Helvetica" as CFString, tam, nil)
        }
        var pedidos: CTFontSymbolicTraits = []
        if negrita { pedidos.insert(.traitBold) }
        if cursiva { pedidos.insert(.traitItalic) }
        var fuente = base
        var tiene: CTFontSymbolicTraits = []
        if !pedidos.isEmpty {
            // Primero las dos; si no hay, la que haya.
            if let f = CTFontCreateCopyWithSymbolicTraits(base, tam, nil, pedidos, pedidos) {
                fuente = f
            } else if negrita, let f = CTFontCreateCopyWithSymbolicTraits(base, tam, nil, .traitBold, .traitBold) {
                fuente = f
            } else if cursiva,
                      let f = CTFontCreateCopyWithSymbolicTraits(base, tam, nil, .traitItalic, .traitItalic) {
                fuente = f
            }
            tiene = CTFontGetSymbolicTraits(fuente)
        }
        if cursiva, !tiene.contains(.traitItalic) {
            // La cursiva de mentira: la letra inclinada unos 12 grados.
            var inclinada = CGAffineTransform(a: 1, b: 0, c: 0.21, d: 1, tx: 0, ty: 0)
            fuente = CTFontCreateCopyWithAttributes(fuente, tam, &inclinada, nil)
        }
        let c = Cara(fuente: fuente, falsaNegrita: negrita && !tiene.contains(.traitBold))
        if hechas.count > 256 { hechas.removeAll() }
        hechas[clave] = c
        return c
    }

    /// El grosor del trazo de la negrita de mentira, en % del tamano de
    /// letra (negativo: rellena y traza, como `NSAttributedString`).
    static let grosorFalsaNegrita: CGFloat = -4

    /// **Los atributos de UIKit** para pintar con `NSString.draw` (los
    /// rotulos de las cotas y lo que no es del catalogo).
    static func atributos(familia: String, tam: CGFloat, negrita: Bool, cursiva: Bool,
                          color: CGColor) -> [NSAttributedString.Key: Any] {
        let c = cara(familia: familia, tam: tam, negrita: negrita, cursiva: cursiva)
        // `CTFont` y `UIFont` son la misma cosa (puente sin coste).
        let fuente = unsafeBitCast(c.fuente, to: UIFont.self)
        var a: [NSAttributedString.Key: Any] = [.font: fuente, .foregroundColor: UIColor(cgColor: color)]
        if c.falsaNegrita {
            a[.strokeWidth] = grosorFalsaNegrita
            a[.strokeColor] = UIColor(cgColor: color)
        }
        return a
    }

    // MARK: Pintar

    /// **Los renglones de `texto`** tal como se pintan: los saltos de linea
    /// y, si `anchoMax` es un ancho de verdad, partidos para que quepan
    /// (como en Windows, por palabras). Un renglon vacio cuenta.
    static func renglones(_ texto: String, fuente: CTFont, anchoMax: CGFloat,
                          atributos: [NSAttributedString.Key: Any]) -> [CTLine] {
        let partir = anchoMax > 0 && anchoMax < 1e6
        var lineas: [CTLine] = []
        for renglon in texto.components(separatedBy: "\n") {
            let cadena = NSAttributedString(string: renglon, attributes: atributos)
            guard partir, cadena.length > 0 else {
                lineas.append(CTLineCreateWithAttributedString(cadena))
                continue
            }
            let tipografo = CTTypesetterCreateWithAttributedString(cadena)
            var inicio = 0
            while inicio < cadena.length {
                var n = CTTypesetterSuggestLineBreak(tipografo, inicio, Double(anchoMax))
                if n <= 0 { n = cadena.length - inicio }
                lineas.append(CTTypesetterCreateLine(tipografo, CFRange(location: inicio, length: n)))
                inicio += n
            }
        }
        return lineas
    }

    /// **Pinta un texto de una letra del catalogo** en `ctx` (con la Y hacia
    /// abajo, la del mundo): `origen` es la esquina de arriba a la izquierda
    /// de su caja. `false` si la familia no es del catalogo (lo pinta quien
    /// llama, como siempre).
    @discardableResult
    static func pintar(_ texto: String, origen: CGPoint, tam: CGFloat, anchoMax: CGFloat, familia: String,
                       negrita: Bool, cursiva: Bool, color: CGColor, en ctx: CGContext) -> Bool {
        guard let letra = letra(de: familia), tam > 0 else { return false }
        let c = cara(familia: letra.nombre, tam: tam, negrita: negrita, cursiva: cursiva)
        var atributos: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): c.fuente,
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): color,
        ]
        if c.falsaNegrita {
            atributos[NSAttributedString.Key(kCTStrokeWidthAttributeName as String)] = grosorFalsaNegrita
            atributos[NSAttributedString.Key(kCTStrokeColorAttributeName as String)] = color
        }
        let lineas = renglones(texto, fuente: c.fuente, anchoMax: anchoMax, atributos: atributos)
        ctx.saveGState()
        // El texto de Core Text va con la Y hacia arriba: se le da la vuelta.
        ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        for (fila, linea) in lineas.enumerated() {
            let y = origen.y + (CGFloat(fila) * letra.interlineado + letra.lineaBase) * tam
            ctx.textPosition = CGPoint(x: origen.x, y: y)
            CTLineDraw(linea, ctx)
        }
        ctx.restoreGState()
        return true
    }
}
