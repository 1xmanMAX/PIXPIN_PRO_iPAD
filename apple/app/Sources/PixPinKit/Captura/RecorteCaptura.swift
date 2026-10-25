// Las cuentas de recortar una captura en el iPad (la barra de abajo de la
// captura de Windows, `captura2/`, y `MedidasCaptura` del Mac): elegir la
// zona arrastrando, atarla a una proporcion (16:9 · 4:3 · 1:1), teclear las
// medidas exactas, «Repetir la ultima zona» y el color de la lupa en HEX y
// RGB.
//
// En el iPad no se elige la zona encima de la pantalla (iOS no deja), sino
// despues, encima de la foto en el editor: todo va en PIXELES de la foto,
// con el origen arriba a la izquierda. Puras a proposito: las prueban
// `CapturaIPadMasTests`.

import CoreGraphics
import Foundation

/// Las proporciones de la barra (`Proporcion` de `captura2/disposicion.rs`).
enum ProporcionCaptura: Int, CaseIterable, Identifiable, Sendable {
    case libre, p16x9, p4x3, p1x1

    var id: Int { rawValue }

    /// `(ancho, alto)`, o `nil` si es libre.
    var par: (Int, Int)? {
        switch self {
        case .libre: nil
        case .p16x9: (16, 9)
        case .p4x3: (4, 3)
        case .p1x1: (1, 1)
        }
    }

    var rotulo: String {
        switch self {
        case .libre: "Libre"
        case .p16x9: "16:9"
        case .p4x3: "4:3"
        case .p1x1: "1:1"
        }
    }
}

enum RecorteCaptura {
    /// Cinco cifras como mucho por lado (`Cifras` de Windows).
    static let cifrasMaximas = 5
    /// Por debajo de esto (en pixeles de la foto) no hay zona.
    static let ladoMinimo: CGFloat = 2

    /// La zona entre dos puntos, se arrastre hacia donde se arrastre.
    static func rectEntre(_ a: CGPoint, _ b: CGPoint) -> CGRect {
        CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(a.x - b.x), height: abs(a.y - b.y))
    }

    /// `r` dentro de `limites`; `.zero` si queda fuera del todo.
    static func dentro(_ r: CGRect, de limites: CGRect) -> CGRect {
        let d = r.standardized.intersection(limites)
        return d.isNull ? .zero : d
    }

    /// En pixeles enteros (hacia fuera) y dentro de la foto.
    static func enPixeles(_ r: CGRect, ancho: Int, alto: Int) -> CGRect {
        let r = r.standardized
        let x0 = (r.minX + 1e-6).rounded(.down), y0 = (r.minY + 1e-6).rounded(.down)
        let x1 = (r.maxX - 1e-6).rounded(.up), y1 = (r.maxY - 1e-6).rounded(.up)
        return dentro(CGRect(x: x0, y: y0, width: x1 - x0, height: y1 - y0),
                      de: CGRect(x: 0, y: 0, width: ancho, height: alto))
    }

    /// Si la zona sirve para recortar.
    static func valida(_ r: CGRect?) -> Bool {
        guard let r = r?.standardized else { return false }
        return r.width >= ladoMinimo && r.height >= ladoMinimo
    }

    // MARK: Proporciones

    /// El rectangulo desde `ancla` hacia `p` atado a la proporcion: manda el
    /// lado que mas se arrastro (`atado` de `pixpin-ui/src/overlay.rs`).
    /// Libre: el rectangulo entre los dos puntos, sin mas.
    static func atado(desde ancla: CGPoint, hasta p: CGPoint, proporcion: ProporcionCaptura) -> CGRect {
        guard let par = proporcion.par else { return rectEntre(ancla, p) }
        let (pa, pb) = par
        let dx = p.x - ancla.x, dy = p.y - ancla.y
        let ax = abs(dx), ay = abs(dy)
        let ancho: CGFloat, alto: CGFloat
        if ax * CGFloat(pb) >= ay * CGFloat(pa) {
            ancho = ax
            alto = ax * CGFloat(pb) / CGFloat(pa)
        } else {
            alto = ay
            ancho = ay * CGFloat(pa) / CGFloat(pb)
        }
        let x = dx < 0 ? ancla.x - ancho : ancla.x
        let y = dy < 0 ? ancla.y - alto : ancla.y
        return CGRect(x: x, y: y, width: ancho, height: alto)
    }

    /// Ajusta una zona ya elegida a la proporcion, en el sitio: se conserva
    /// la esquina de arriba a la izquierda y el ancho, y se calcula el alto;
    /// si no cabe hacia abajo en `limites`, manda el alto que cabe y se
    /// calcula el ancho (`poner_proporcion` de Windows). Sin zona, la mayor
    /// de esa proporcion centrada. Libre: igual.
    static func ajustar(_ zona: CGRect?, a proporcion: ProporcionCaptura, limites: CGRect) -> CGRect? {
        guard let par = proporcion.par else { return zona }
        let (pa, pb) = par
        guard let r = zona?.standardized, valida(r) else {
            // La mayor que cabe, centrada.
            var ancho = limites.width
            var alto = ancho * CGFloat(pb) / CGFloat(pa)
            if alto > limites.height {
                alto = limites.height
                ancho = alto * CGFloat(pa) / CGFloat(pb)
            }
            return CGRect(x: limites.midX - ancho / 2, y: limites.midY - alto / 2, width: ancho, height: alto)
        }
        var ancho = max(r.width, 1)
        var alto = max(ancho * CGFloat(pb) / CGFloat(pa), 1)
        let cabeAlto = max(limites.maxY - r.minY, 1)
        if alto > cabeAlto {
            alto = cabeAlto
            ancho = max(alto * CGFloat(pa) / CGFloat(pb), 1)
        }
        let cabeAncho = max(limites.maxX - r.minX, 1)
        if ancho > cabeAncho {
            ancho = cabeAncho
            alto = max(ancho * CGFloat(pb) / CGFloat(pa), 1)
        }
        return dentro(CGRect(x: r.minX, y: r.minY, width: ancho, height: alto), de: limites)
    }

    // MARK: Medidas tecleadas

    /// Lee «ancho × alto» como lo teclea la gente: `800x600`, `800 × 600`,
    /// `800*600`, `800,600` o `800 600`. Solo cifras, de 1 a 5 por lado y
    /// mayores que cero; cualquier otra cosa es `nil`.
    static func leer(_ texto: String) -> (ancho: Int, alto: Int)? {
        let separadores = CharacterSet(charactersIn: "xX×*,;: ")
        let trozos = texto.trimmingCharacters(in: .whitespacesAndNewlines)
            .components(separatedBy: separadores)
            .filter { !$0.isEmpty }
        guard trozos.count == 2, let a = numero(trozos[0]), let b = numero(trozos[1]) else { return nil }
        return (a, b)
    }

    /// Un lado tecleado: solo cifras, de 1 a 5, mayor que cero.
    static func numero(_ s: String) -> Int? {
        let s = s.trimmingCharacters(in: .whitespaces)
        guard !s.isEmpty, s.count <= cifrasMaximas,
              s.unicodeScalars.allSatisfy({ $0.value >= 48 && $0.value <= 57 }),
              let v = Int(s), v > 0 else { return nil }
        return v
    }

    /// La zona que piden unas medidas (`zona_de_medidas` de Windows): con
    /// zona elegida, desde su esquina de arriba a la izquierda; sin ella,
    /// centrada. Siempre dentro de `limites` (si no cabe, se queda en lo que
    /// cabe, pegada al borde).
    static func zona(ancho: Int, alto: Int, seleccion: CGRect?, limites: CGRect) -> CGRect {
        let w = min(CGFloat(max(ancho, 1)), limites.width)
        let h = min(CGFloat(max(alto, 1)), limites.height)
        var x: CGFloat, y: CGFloat
        if let s = seleccion?.standardized, valida(s) {
            x = s.minX
            y = s.minY
        } else {
            x = limites.midX - w / 2
            y = limites.midY - h / 2
        }
        x = min(max(x, limites.minX), limites.maxX - w)
        y = min(max(y, limites.minY), limites.maxY - h)
        return CGRect(x: x, y: y, width: w, height: h)
    }

    /// Mueve una zona `dx`, `dy` sin salirse ni cambiar de tamano
    /// (`deslizar_dentro` de Windows).
    static func deslizar(_ r: CGRect, dx: CGFloat, dy: CGFloat, dentroDe limites: CGRect) -> CGRect {
        guard !r.isEmpty, !limites.isEmpty else { return r }
        let x = min(max(r.minX + dx, limites.minX), max(limites.maxX - r.width, limites.minX))
        let y = min(max(r.minY + dy, limites.minY), max(limites.maxY - r.height, limites.minY))
        return CGRect(x: x, y: y, width: r.width, height: r.height)
    }

    // MARK: Las marcas tras recortar

    /// Las marcas pasadas a la foto recortada (todo se mueve lo que se
    /// quito por arriba y por la izquierda).
    static func moverMarcas(_ marcas: [MarcaCaptura], recorte: CGRect) -> [MarcaCaptura] {
        marcas.map { m in
            var n = m
            n.puntos = m.puntos.map { CGPoint(x: $0.x - recorte.minX, y: $0.y - recorte.minY) }
            return n
        }
    }

    // MARK: Color de la lupa

    /// `#RRGGBB`, como `texto_color(Hex)` de `pixpin-ui/src/lupa.rs`.
    static func hex(_ r: UInt8, _ g: UInt8, _ b: UInt8) -> String {
        String(format: "#%02X%02X%02X", Int(r), Int(g), Int(b))
    }

    /// `RGB r g b`, la otra linea de la lupa de Windows.
    static func rgb(_ r: UInt8, _ g: UInt8, _ b: UInt8) -> String {
        "RGB \(r) \(g) \(b)"
    }

    /// El pixel (entero) de la foto bajo `p`; `nil` si cae fuera.
    static func pixel(en p: CGPoint, ancho: Int, alto: Int) -> (x: Int, y: Int)? {
        guard p.x.isFinite, p.y.isFinite else { return nil }
        let x = Int(p.x.rounded(.down)), y = Int(p.y.rounded(.down))
        guard x >= 0, y >= 0, x < ancho, y < alto else { return nil }
        return (x, y)
    }

    /// El cuadrado de `lado` pixeles alrededor de `p` que ensena la lupa,
    /// dentro de la foto.
    static func zonaDeLupa(en p: CGPoint, lado: Int, ancho: Int, alto: Int) -> CGRect {
        let l = CGFloat(max(1, min(lado, min(ancho, alto))))
        let x = min(max((p.x - l / 2).rounded(.down), 0), CGFloat(ancho) - l)
        let y = min(max((p.y - l / 2).rounded(.down), 0), CGFloat(alto) - l)
        return CGRect(x: x, y: y, width: l, height: l)
    }
}

/// La ultima zona recortada, en `UserDefaults`: «Repetir la ultima zona»
/// (`R`) la vuelve a poner aunque la app se haya cerrado. Va en pixeles de
/// la foto; en una foto mas pequena se queda en lo que cabe.
struct UltimaZonaCaptura {
    static let clave = "captura.ultimaZona.ipad"
    let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func guardar(_ r: CGRect) {
        let r = r.standardized
        guard r.width >= 1, r.height >= 1 else { return }
        defaults.set([Double(r.minX), Double(r.minY), Double(r.width), Double(r.height)], forKey: Self.clave)
    }

    func leer() -> CGRect? {
        guard let v = defaults.array(forKey: Self.clave) as? [Double], v.count == 4, v[2] >= 1, v[3] >= 1
        else { return nil }
        return CGRect(x: v[0], y: v[1], width: v[2], height: v[3])
    }

    /// La ultima, dentro de una foto de `ancho` x `alto` (nil si no hay o
    /// queda fuera del todo).
    func leer(ancho: Int, alto: Int) -> CGRect? {
        guard let r = leer() else { return nil }
        let d = RecorteCaptura.dentro(r, de: CGRect(x: 0, y: 0, width: ancho, height: alto))
        return RecorteCaptura.valida(d) ? d : nil
    }
}
