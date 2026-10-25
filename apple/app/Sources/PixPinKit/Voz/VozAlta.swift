// **Escuchar un documento en voz alta**: las cuentas, sin sonido, para poder
// comprobarlas. Es `pixpin_docs::voz_alta` de Windows (el `VozAlta.kt` del
// movil) traido a Swift: en que trozos se lee, con que idioma, a que
// velocidad y como se escribe en el boton. Quien habla es `LeerEnVoz`
// (`AVSpeechSynthesizer`).
//
// Por que en Swift y no cruzando el puente: los trozos se cortan sobre el
// texto de cada hoja que da PDFKit (`PDFPage.string`) y se devuelven como
// `NSRange` en UTF-16, que es lo que pide `PDFPage.selection(for:)` para
// resaltar en la hoja lo que suena. Con el corte de Rust (en `char`) habria
// que traducir los rangos en cada trozo.

import AVFoundation
import Foundation

public enum VozAlta {
    /// El emoticono del marcador de la voz (`EMOJI_DE_VOZ`): donde se dejo
    /// de escuchar, uno solo por documento.
    public static let emojiDeVoz = "🟢"

    /// Las velocidades que se ofrecen; 1 es la normal (`VELOCIDADES`).
    public static let velocidades: [Float] = [0.75, 1.0, 1.25, 1.5, 1.75, 2.0]

    /// Lo mas largo que se le da a la voz de una vez (`TOPE`).
    public static let tope = 600

    /// El idioma de reserva: el de la app (`lector-voz-idioma` de Windows).
    public static let idiomaDeLaApp = "es-ES"

    /// La velocidad siguiente al pulsar el boton, dando la vuelta al final.
    /// Una rara vuelve a la normal.
    public static func siguienteVelocidad(_ ahora: Float) -> Float {
        guard let i = velocidades.firstIndex(where: { abs($0 - ahora) < 0.01 }) else { return velocidades[1] }
        return velocidades[(i + 1) % velocidades.count]
    }

    /// «1×», «1,25×»: como se lee en el boton (`rotulo`).
    public static func rotulo(_ velocidad: Float) -> String {
        let centesimas = Int((velocidad * 100).rounded())
        let entero = centesimas / 100, resto = centesimas % 100
        if resto == 0 { return "\(entero)×" }
        if resto % 10 == 0 { return "\(entero),\(resto / 10)×" }
        return String(format: "%d,%02d×", entero, resto)
    }

    /// **La velocidad como la entiende la voz del Mac** (`rate` de
    /// `AVSpeechUtterance`, de 0 a 1 con la normal en 0,5). No es lineal:
    /// 0,75 de `rate` ya suena al doble, asi que por encima de la normal se
    /// sube un cuarto por cada «×» y por debajo se baja un poco menos de lo
    /// que manda la cuenta (a 0,4 una voz ya se arrastra). Es el
    /// `tasa_de_windows` de Windows con la escala de Apple.
    public static func tasaDeApple(_ velocidad: Float) -> Float {
        let normal = AVSpeechUtteranceDefaultSpeechRate
        let minimo = AVSpeechUtteranceMinimumSpeechRate, maximo = AVSpeechUtteranceMaximumSpeechRate
        guard velocidad.isFinite, velocidad > 0 else { return normal }
        let tasa = velocidad >= 1 ? normal + (velocidad - 1) * 0.25 : normal - (1 - velocidad) * 0.4
        return min(max(tasa, minimo), maximo)
    }

    // MARK: Los trozos

    /// **Un texto, en trozos que la voz admita**, como `voz_alta::cortes`: se
    /// corta por el final de las frases; una frase mas larga que `tope`, por
    /// las comas; y lo que aun sobre, por los espacios. Devuelve los trozos
    /// como rangos **en UTF-16** del texto original, sin los blancos de los
    /// bordes: el lector los usa para marcar en la hoja lo que suena.
    public static func cortes(_ texto: String, tope: Int) -> [NSRange] {
        let l = Array(texto.utf16)
        let tope = max(tope, 1)
        var salida: [NSRange] = []
        var actual: Range<Int>?

        func soltar() {
            if let r = actual, let r = sinBlancos(l, r) {
                salida.append(NSRange(location: r.lowerBound, length: r.count))
            }
            actual = nil
        }
        func anadir(_ pieza: Range<Int>) {
            if (actual?.count ?? 0) + pieza.count > tope { soltar() }
            actual = actual.map { $0.lowerBound..<pieza.upperBound } ?? pieza
        }

        // Los finales de frase: tras `.!?…;:` (con comillas o parentesis que
        // cierran) y un blanco.
        let frases = partirDetras(l, 0..<l.count) { l, i in esBlanco(l[i]) && i > 0 && cierraFrase(l, i - 1) }
        for frase in frases {
            if frase.count <= tope { anadir(frase); continue }
            let partes = partirDetras(l, frase) { l, i in esBlanco(l[i]) && i > 0 && l[i - 1] == 0x2C }
            for parte in partes {
                if parte.count <= tope { anadir(parte); continue }
                // Por los espacios, y una «palabra» eterna, a tajos.
                for palabra in partirDetras(l, parte, { l, i in esBlanco(l[i]) }) {
                    var desde = palabra.lowerBound
                    while desde < palabra.upperBound {
                        var hasta = min(desde + tope, palabra.upperBound)
                        // Sin partir una letra de dos mitades (un emoticono).
                        if hasta < palabra.upperBound, UTF16.isLeadSurrogate(l[hasta - 1]), hasta - 1 > desde {
                            hasta -= 1
                        }
                        anadir(desde..<hasta)
                        desde = hasta
                    }
                }
            }
        }
        soltar()
        return salida
    }

    /// Los trozos ya como texto, con los blancos juntados (`trozos`).
    public static func trozos(_ texto: String, tope: Int) -> [String] {
        let ns = texto as NSString
        return cortes(texto, tope: tope).map { juntarBlancos(ns.substring(with: $0)) }.filter { !$0.isEmpty }
    }

    /// Los blancos seguidos, uno; y sin blancos en los bordes.
    public static func juntarBlancos(_ texto: String) -> String {
        texto.split(whereSeparator: { $0.isWhitespace }).joined(separator: " ")
    }

    /// **Lo que se le da a la voz**: cada blanco (un salto de renglon del PDF)
    /// pasa a ser un espacio, **sin cambiar el largo**. Asi el rango de la
    /// palabra que dice la voz (`willSpeakRangeOfSpeechString`) cae en el
    /// mismo sitio del texto de la hoja y se puede resaltar alli.
    public static func paraDecir(_ texto: String) -> String {
        String(decoding: texto.utf16.map { esBlanco($0) ? 0x20 : $0 }, as: UTF16.self)
    }

    static func esBlanco(_ u: UInt16) -> Bool {
        guard let s = Unicode.Scalar(u) else { return false }
        return s.properties.isWhitespace
    }

    /// Si en `i` acaba una frase: un signo de final, quiza seguido de
    /// comillas o parentesis que cierran.
    private static func cierraFrase(_ l: [UInt16], _ i: Int) -> Bool {
        var i = i
        // " ' » ” )
        while [0x22, 0x27, 0xBB, 0x201D, 0x29].contains(l[i]) {
            if i == 0 { return false }
            i -= 1
        }
        // . ! ? … ; :
        return [0x2E, 0x21, 0x3F, 0x2026, 0x3B, 0x3A].contains(l[i])
    }

    /// Parte `rango` justo **detras** de cada `i` en la que `corta` dice que
    /// si (la letra `i` va con lo de delante). Sin perder nada.
    private static func partirDetras(_ l: [UInt16], _ rango: Range<Int>,
                                     _ corta: ([UInt16], Int) -> Bool) -> [Range<Int>] {
        var salida: [Range<Int>] = []
        var desde = rango.lowerBound
        for i in rango where corta(l, i) {
            salida.append(desde..<(i + 1))
            desde = i + 1
        }
        if desde < rango.upperBound { salida.append(desde..<rango.upperBound) }
        return salida
    }

    private static func sinBlancos(_ l: [UInt16], _ r: Range<Int>) -> Range<Int>? {
        var a = r.lowerBound, b = r.upperBound
        while a < b, esBlanco(l[a]) { a += 1 }
        while b > a, esBlanco(l[b - 1]) { b -= 1 }
        return a < b ? a..<b : nil
    }

    // MARK: El idioma

    /// «es_ES», «es-es», «ES» → («es», «ES») (`partes`).
    public static func partes(_ idioma: String) -> (lengua: String, pais: String) {
        let p = idioma.trimmingCharacters(in: .whitespaces)
            .split(whereSeparator: { $0 == "-" || $0 == "_" }).map(String.init)
        let lengua = p.first?.lowercased() ?? ""
        let pais = p.count > 1 && (p[1].count == 2 || p[1].count == 3) ? p[1].uppercased() : ""
        return (lengua, pais)
    }

    /// Las palabras mas corrientes de cada lengua (`PALABRAS`).
    private static let palabras: [(String, Set<String>)] = [
        ("es", ["el", "la", "los", "las", "de", "del", "que", "y", "en", "un", "una", "por", "con",
                "para", "es", "se", "no", "lo", "al", "su", "como", "más", "pero", "sus", "le", "ya",
                "o", "este", "sí", "porque", "esta", "entre", "cuando", "muy", "sin", "sobre",
                "también", "hay", "donde", "está"]),
        ("en", ["the", "of", "and", "to", "in", "is", "that", "it", "was", "for", "on", "are", "as",
                "with", "his", "they", "at", "be", "this", "have", "from", "or", "by", "but", "not",
                "what", "all", "were", "when", "we", "there", "can", "which", "their", "if", "would",
                "been", "has", "an", "she"]),
        ("pt", ["o", "os", "as", "de", "do", "da", "dos", "das", "que", "e", "em", "um", "uma", "para",
                "com", "não", "no", "na", "por", "mais", "se", "ao", "como", "mas", "foi", "ele",
                "ela", "seu", "sua", "ou", "ser", "quando", "muito", "há", "nos", "já", "está",
                "também", "só", "pelo"]),
        ("fr", ["le", "la", "les", "de", "des", "du", "et", "un", "une", "est", "en", "que", "qui",
                "dans", "pour", "pas", "sur", "au", "aux", "ne", "il", "elle", "se", "ce", "avec",
                "plus", "par", "son", "sa", "ses", "mais", "nous", "vous", "ont", "été", "cette",
                "sont", "comme", "leur", "tout"]),
        ("it", ["il", "lo", "la", "gli", "le", "di", "del", "della", "che", "e", "è", "un", "una",
                "per", "con", "non", "in", "si", "da", "al", "sono", "come", "più", "ma", "anche",
                "questo", "nel", "nella", "dei", "delle", "suo", "sua", "ha", "molto", "quando", "già",
                "tra", "cosa", "loro", "essere"]),
        ("de", ["der", "die", "das", "und", "ist", "nicht", "ein", "eine", "zu", "den", "dem", "mit",
                "sich", "des", "auf", "für", "im", "von", "auch", "es", "an", "werden", "aus", "er",
                "hat", "dass", "sie", "nach", "wird", "bei", "einer", "um", "noch", "wie", "einem",
                "über", "so", "zum", "war", "haben"]),
    ]

    /// **De que idioma es el texto**, por sus palabras mas corrientes
    /// (`idioma_del_texto`). Si la muestra no deja claro nada, `porDefecto`.
    public static func idiomaDelTexto(_ muestra: String, porDefecto: String) -> String {
        let todas = muestra.lowercased()
            .split(whereSeparator: { !($0.isLetter || $0 == "'") })
            .prefix(600).map(String.init)
        guard todas.count >= 8 else { return porDefecto }
        let cuentas = palabras.map { lengua, lista in (lengua, todas.filter { lista.contains($0) }.count) }
        // El primero con mas (el primero en caso de empate).
        let mejor = cuentas.dropFirst().reduce(cuentas[0]) { $1.1 > $0.1 ? $1 : $0 }
        let segundo = cuentas.filter { $0.0 != mejor.0 }.map(\.1).max() ?? 0
        // Claro: al menos un 6 % de las palabras y bastante por delante.
        if Float(mejor.1) >= Float(todas.count) * 0.06, Float(mejor.1) > Float(segundo) * 1.3 {
            return mejor.0
        }
        return porDefecto
    }

    /// **Con que idioma se lee** (`idioma_para_leer`): el adivinado por el
    /// texto, afinado con el acento del documento o del equipo si es la
    /// misma lengua; sin nada que adivinar, el del documento o el del equipo.
    public static func idiomaParaLeer(muestra: String, lang: String, delEquipo: String) -> String {
        let adivinado = idiomaDelTexto(muestra, porDefecto: "")
        let (suyo, suPais) = partes(lang)
        if adivinado.isEmpty { return suyo.isEmpty ? delEquipo : lang }
        if suyo == adivinado, !suPais.isEmpty { return lang }
        if partes(delEquipo).lengua == adivinado { return delEquipo }
        return adivinado
    }

    // MARK: Las voces del Mac

    /// **La voz con la que se lee un idioma**: la elegida para esa lengua si
    /// sigue en el Mac; si no, la mejor del idioma con su pais (o del pais
    /// del Mac), luego cualquiera de la lengua. `nil` si el Mac no habla esa
    /// lengua. Las mejoradas y las premium ganan a las compactas.
    public static func voz(para idioma: String, preferida: String?,
                           voces: [AVSpeechSynthesisVoice] = AVSpeechSynthesisVoice.speechVoices())
        -> AVSpeechSynthesisVoice? {
        let (lengua, pais) = partes(idioma)
        let deLaLengua = voces.filter { partes($0.language).lengua == lengua }
        if let p = preferida, let v = deLaLengua.first(where: { $0.identifier == p }) { return v }
        let paisDelMac = Locale.current.region?.identifier ?? ""
        func puntos(_ v: AVSpeechSynthesisVoice) -> Int {
            let suPais = partes(v.language).pais
            var n = 0
            if !pais.isEmpty, suPais == pais { n += 100 } else if suPais == paisDelMac { n += 50 }
            switch v.quality {
            case .premium: n += 20
            case .enhanced: n += 10
            default: break
            }
            // Las voces de broma del Mac (`com.apple.speech.synthesis...`)
            // y las de novedad, al final.
            if v.identifier.contains("novelty") || v.identifier.contains("eloquence") { n -= 30 }
            return n
        }
        return deLaLengua.max { puntos($0) < puntos($1) }
    }

    /// «Mónica · mejorada»: la voz como se ensena debajo de la barra.
    public static func nombre(de v: AVSpeechSynthesisVoice) -> String {
        switch v.quality {
        case .premium: "\(v.name) · premium"
        case .enhanced: "\(v.name) · mejorada"
        default: v.name
        }
    }

    /// La voz elegida para una lengua (el `Preferencias::voz_de` de Windows),
    /// de este Mac.
    public static func preferida(lengua: String, en d: UserDefaults = .standard) -> String? {
        d.string(forKey: "PixPinVozParaLeer." + partes(lengua).lengua)
    }

    public static func ponerPreferida(_ identificador: String?, lengua: String, en d: UserDefaults = .standard) {
        let clave = "PixPinVozParaLeer." + partes(lengua).lengua
        if let identificador { d.set(identificador, forKey: clave) } else { d.removeObject(forKey: clave) }
    }

    // MARK: Por donde va

    /// **El trozo por el que empezar**: el primero de la hoja `pagina` que
    /// empiece a la altura `dentro` (0 arriba, 1 abajo) o mas abajo, o el
    /// primero de las hojas siguientes (`trozo_desde`). Sin ninguno, el 0.
    public static func trozoDesde(_ donde: [(pagina: Int, altura: Float)], pagina: Int, dentro: Float) -> Int {
        donde.firstIndex { $0.pagina > pagina || ($0.pagina == pagina && $0.altura >= dentro - 0.01) } ?? 0
    }

    /// **Si hay que mover la vista** para seguir a lo que suena: solo si lo
    /// de antes se veia (quien se fue a mirar otra cosa no es arrastrado) y
    /// lo nuevo cae fuera de la franja comoda (del 8 % al 85 % de lo que se
    /// ve). Devuelve la `y` de arriba de la vista nueva (lo nuevo al 25 %).
    /// Todo en la misma escala, con la Y hacia abajo.
    public static func seguir(y: CGFloat, yAntes: CGFloat?, arriba: CGFloat, visto: CGFloat,
                              primero: Bool) -> CGFloat? {
        let seVeia = primero || yAntes.map { $0 > arriba && $0 < arriba + visto } ?? true
        guard seVeia, y < arriba + visto * 0.08 || y > arriba + visto * 0.85 else { return nil }
        return y - visto * 0.25
    }
}
