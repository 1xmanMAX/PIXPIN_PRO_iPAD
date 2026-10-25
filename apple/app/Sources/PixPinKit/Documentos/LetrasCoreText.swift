// Las letras de Word y PowerPoint con Core Text: el mismo sitio mide (para
// que `pixpin-office` maquete, `MedidorCoreText`) y pinta (`PintorOffice`),
// asi lo que se maqueta y lo que se ve son la misma letra con los mismos
// avances.
//
// Rendimiento: cada letra (familia, negrita, cursiva) se crea una vez y se
// guarda; el glifo de cada caracter tambien. Rust pide los avances una sola
// vez por caracter y letra (`pixpin-nucleo/src/office.rs`), asi que maquetar
// un documento entero son unas pocas llamadas.

import CoreText
import Foundation
import PixPinCore

/// Las letras ya creadas y sus glifos. Se usa desde el hilo que maqueta y
/// desde el que pinta: todo bajo un cerrojo.
final class LetrasCoreText: @unchecked Sendable {
    static let compartidas = LetrasCoreText()

    private struct Clave: Hashable {
        let familia: String
        let negrita: Bool
        let cursiva: Bool
    }

    private let cerrojo = NSLock()
    /// La letra a tamano 1000 (se escala al pintar con `CTFontCreateCopyWithAttributes`).
    private var letras: [Clave: CTFont] = [:]
    private var glifos: [Clave: [UInt32: (CTFont, CGGlyph)]] = [:]
    private var instaladas: [String: Bool] = [:]

    /// La sustituta de las familias de Windows que el Mac no trae.
    static func sustituta(de familia: String) -> String {
        switch familia.lowercased() {
        case "calibri", "calibri light", "segoe ui", "aptos", "aptos display", "candara", "corbel":
            return "Helvetica Neue"
        case "cambria", "cambria math", "constantia", "book antiqua":
            return "Georgia"
        case "consolas", "lucida console", "cascadia code", "cascadia mono":
            return "Menlo"
        case "symbol", "wingdings", "wingdings 2", "wingdings 3":
            return "Apple Symbols"
        default:
            return familia
        }
    }

    /// La letra a tamano 1000.
    func letra(familia: String, negrita: Bool, cursiva: Bool) -> CTFont {
        let k = Clave(familia: familia, negrita: negrita, cursiva: cursiva)
        cerrojo.lock()
        defer { cerrojo.unlock() }
        if let f = letras[k] { return f }
        let nombre = existeSinCerrojo(familia) ? familia : LetrasCoreText.sustituta(de: familia)
        var f = CTFontCreateWithName(nombre as CFString, 1000, nil)
        var rasgos: CTFontSymbolicTraits = []
        if negrita { rasgos.insert(.traitBold) }
        if cursiva { rasgos.insert(.traitItalic) }
        if !rasgos.isEmpty, let con = CTFontCreateCopyWithSymbolicTraits(f, 1000, nil, rasgos, rasgos) {
            f = con
        }
        letras[k] = f
        return f
    }

    /// Si la familia esta instalada en este Mac.
    func existe(_ familia: String) -> Bool {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return existeSinCerrojo(familia)
    }

    private func existeSinCerrojo(_ familia: String) -> Bool {
        if let e = instaladas[familia] { return e }
        let f = CTFontCreateWithName(familia as CFString, 12, nil)
        let real = (CTFontCopyFamilyName(f) as String).lowercased()
        let e = real == familia.lowercased()
        instaladas[familia] = e
        return e
    }

    /// El glifo de un caracter en esa letra (o en la que lo tenga, si ella
    /// no), a tamano 1000.
    func glifo(_ c: Unicode.Scalar, familia: String, negrita: Bool, cursiva: Bool) -> (CTFont, CGGlyph) {
        let base = letra(familia: familia, negrita: negrita, cursiva: cursiva)
        let k = Clave(familia: familia, negrita: negrita, cursiva: cursiva)
        cerrojo.lock()
        if let g = glifos[k]?[c.value] {
            cerrojo.unlock()
            return g
        }
        cerrojo.unlock()
        let unidades = Array(String(Character(c)).utf16)
        var g = [CGGlyph](repeating: 0, count: unidades.count)
        var fuente = base
        if !CTFontGetGlyphsForCharacters(base, unidades, &g, unidades.count) || g[0] == 0 {
            let cadena = String(Character(c)) as CFString
            fuente = CTFontCreateForString(base, cadena, CFRange(location: 0, length: unidades.count))
            g = [CGGlyph](repeating: 0, count: unidades.count)
            _ = CTFontGetGlyphsForCharacters(fuente, unidades, &g, unidades.count)
        }
        let hecho = (fuente, g[0])
        cerrojo.lock()
        glifos[k, default: [:]][c.value] = hecho
        cerrojo.unlock()
        return hecho
    }

    /// El avance de un caracter a tamano 1000.
    func avance(_ c: Unicode.Scalar, familia: String, negrita: Bool, cursiva: Bool) -> Float {
        let (f, g) = glifo(c, familia: familia, negrita: negrita, cursiva: cursiva)
        var glifo = g
        var adelanto = CGSize.zero
        CTFontGetAdvancesForGlyphs(f, .horizontal, &glifo, &adelanto, 1)
        return Float(adelanto.width)
    }

    /// Suelta todo (aviso de memoria): se rehace al volver a pedir.
    func vaciar() {
        cerrojo.lock()
        letras = [:]
        glifos = [:]
        cerrojo.unlock()
    }
}

/// **Quien mide las letras para `pixpin-office`**: Core Text. Rust lo llama
/// desde el hilo que maqueta (nunca el principal).
final class MedidorCoreText: MedidorLetras, @unchecked Sendable {
    private let letras = LetrasCoreText.compartidas

    func avances(caracteres: String, familia: String, negrita: Bool, cursiva: Bool) -> [Float] {
        caracteres.unicodeScalars.map { letras.avance($0, familia: familia, negrita: negrita, cursiva: cursiva) }
    }

    func metricas(familia: String, negrita: Bool, cursiva: Bool) -> MetricasLetra {
        let f = letras.letra(familia: familia, negrita: negrita, cursiva: cursiva)
        return MetricasLetra(ascenso: Float(CTFontGetAscent(f)), descenso: Float(CTFontGetDescent(f)),
                             hueco: Float(CTFontGetLeading(f)))
    }

    func existe(familia: String) -> Bool {
        letras.existe(familia)
    }
}
