// **Escuchar una nota** (Windows, `leer_en_voz.rs`; el Mac,
// `VozDeLaNota.swift`): la voz de la app (`LeerEnVoz`) leyendo el texto de
// una nota o de un `.md`, desde el renglon del cursor, con lo que suena en
// ambar y la palabra que se dice encima, como en el lector. Las marcas del
// Markdown (`#`, `**`, `[ ]`…) no se dicen.

import UIKit

@MainActor
final class VozDeLaNotaIPad: LectorEnganchado {
    /// La que suena: la voz solo la tiene debil.
    private static var actual: VozDeLaNotaIPad?

    private weak var vista: UITextView?
    /// Los trozos de la nota que se leen, en UTF-16 del texto.
    let rangos: [NSRange]
    private var pintado: [NSRange] = []

    static let ambar = UIColor(red: 1, green: 0.78, blue: 0.2, alpha: 0.28)
    static let ambarPalabra = UIColor(red: 1, green: 0.7, blue: 0.1, alpha: 0.55)

    init(vista: UITextView?, rangos: [NSRange]) {
        self.vista = vista
        self.rangos = rangos
    }

    /// **Los trozos de una nota**: cada renglon con algo que decir, cortado
    /// por las frases (`VozAlta.cortes`).
    static func rangos(de texto: String) -> [NSRange] {
        let ns = texto as NSString
        var salida: [NSRange] = []
        ns.enumerateSubstrings(in: NSRange(location: 0, length: ns.length), options: .byParagraphs) { s, r, _, _ in
            guard let s else { return }
            for c in VozAlta.cortes(sinMarcas(s), tope: VozAlta.tope) {
                salida.append(NSRange(location: r.location + c.location, length: c.length))
            }
        }
        return salida
    }

    /// **Sin las marcas del Markdown**, del mismo largo: lo que no se dice
    /// se hace espacio (asi las palabras caen en su sitio del texto).
    static func sinMarcas(_ s: String) -> String {
        // # * _ ` > ~ | [ ] $ y los guiones de las listas («- », «+ »)
        let marcas: Set<UInt16> = [0x23, 0x2A, 0x5F, 0x60, 0x3E, 0x7E, 0x7C, 0x5B, 0x5D, 0x24]
        var u = Array(s.utf16)
        for i in u.indices {
            if marcas.contains(u[i]) {
                u[i] = 0x20
            } else if u[i] == 0x2D || u[i] == 0x2B, i + 1 < u.count, u[i + 1] == 0x20,
                      u[..<i].allSatisfy({ $0 == 0x20 }) {
                u[i] = 0x20
            }
        }
        let t = String(decoding: u, as: UTF16.self)
        return t.replacingOccurrences(of: "  x  ", with: "     ")
    }

    /// **Escucha la nota** desde el trozo de `desde` (una letra del texto).
    /// `false` si no hay nada que leer o el iPad no tiene voz.
    @discardableResult
    static func escuchar(texto: String, desde: Int, clave: String, titulo: String, vista: UITextView?) -> Bool {
        let voz = LeerEnVoz.compartida
        let rangos = rangos(de: texto)
        guard !rangos.isEmpty else { return false }
        let ns = texto as NSString
        let parrafos = rangos.map { sinMarcas(ns.substring(with: $0)) }
        let muestra = String(parrafos.prefix(40).joined(separator: " ").prefix(4000))
        let idioma = VozAlta.idiomaParaLeer(muestra: muestra, lang: "", delEquipo: VozAlta.idiomaDeLaApp)
        let nota = VozDeLaNotaIPad(vista: vista, rangos: rangos)
        guard voz.arrancar(LeerEnVoz.Documento(clave: clave, titulo: titulo, parrafos: parrafos, idioma: idioma),
                           lector: nota) else { return false }
        actual = nota
        let i = rangos.firstIndex { NSMaxRange($0) > desde } ?? 0
        voz.leer(desde: i)
        return true
    }

    // MARK: Lo que avisa la voz

    func vozSuena(_ i: Int, primero: Bool) {
        guard i >= 0, i < rangos.count else { return }
        let r = rangos[i]
        pintar([r], palabra: nil)
        vista?.scrollRangeToVisible(r)
    }

    func vozPalabra(_ r: NSRange?) {
        let i = LeerEnVoz.compartida.actual
        guard i >= 0, i < rangos.count else { return }
        let t = rangos[i]
        let p = r.flatMap { $0.length > 0 && NSMaxRange($0) <= t.length
            ? NSRange(location: t.location + $0.location, length: $0.length) : nil }
        pintar([t], palabra: p)
    }

    func vozAcabo() { pintar([], palabra: nil) }

    func vozFuera() {
        pintar([], palabra: nil)
        if VozDeLaNotaIPad.actual === self { VozDeLaNotaIPad.actual = nil }
    }

    /// El ambar con atributos de pintar (TextKit 2: no tocan el texto ni el
    /// deshacer).
    private func pintar(_ trozos: [NSRange], palabra: NSRange?) {
        guard let v = vista, let tlm = v.textLayoutManager else { return }
        let largo = (v.text as NSString).length
        func rango(_ r: NSRange) -> NSTextRange? {
            guard NSMaxRange(r) <= largo,
                  let a = tlm.location(tlm.documentRange.location, offsetBy: r.location),
                  let b = tlm.location(a, offsetBy: r.length) else { return nil }
            return NSTextRange(location: a, end: b)
        }
        for r in pintado {
            if let t = rango(r) { tlm.removeRenderingAttribute(.backgroundColor, for: t) }
        }
        pintado = []
        for r in trozos {
            if let t = rango(r) {
                tlm.addRenderingAttribute(.backgroundColor, value: VozDeLaNotaIPad.ambar, for: t)
                pintado.append(r)
            }
        }
        if let p = palabra, let t = rango(p) {
            tlm.addRenderingAttribute(.backgroundColor, value: VozDeLaNotaIPad.ambarPalabra, for: t)
            pintado.append(p)
        }
    }
}
