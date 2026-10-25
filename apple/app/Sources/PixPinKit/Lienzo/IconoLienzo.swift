// Los iconos de la barra del lienzo: los MISMOS de Windows, que son los de
// Excalidraw (`pixpin-render/src/icono.rs` y `iconos_excalidraw.rs`). Cada
// uno es una lista de trazados SVG (`d`) con su relleno, su trazo y su
// grosor en las unidades de su `viewBox`; aqui se leen a `CGPath` y se
// pintan del color que pida la barra (el `currentColor` de SVG).
//
// Los datos estan en `IconosLienzo.swift` (generado); aqui, lo que los lee.

import CoreGraphics
import SwiftUI

/// De donde sale el color de un relleno o un trazo.
enum PinturaIcono {
    case nada
    /// `currentColor`: el de la barra (normal, elegida, oscura).
    case actual
    /// Blanco fijo: huecos que el icono quiere claros en cualquier tema.
    case blanco
}

/// Un `<path>` del icono.
struct TrazoIconoLienzo {
    let d: String
    let relleno: PinturaIcono
    let trazo: PinturaIcono
    let grosor: CGFloat
    let redondo: Bool
    let opacidad: Double
    let parImpar: Bool
    let matriz: CGAffineTransform?
    /// El trazado que recorta a este, si lo hay (el rayado del relleno).
    let mascara: String?

    init(d: String, relleno: PinturaIcono, trazo: PinturaIcono, grosor: CGFloat, redondo: Bool,
         opacidad: Double, parImpar: Bool, matriz: CGAffineTransform?, mascara: String?) {
        self.d = d
        self.relleno = relleno
        self.trazo = trazo
        self.grosor = grosor
        self.redondo = redondo
        self.opacidad = opacidad
        self.parImpar = parImpar
        self.matriz = matriz
        self.mascara = mascara
    }
}

/// Un icono entero: su `viewBox` y sus trazados.
struct IconoLienzo {
    let vista: CGRect
    let trazos: [TrazoIconoLienzo]

    /// Lo pinta llenando `caja` (centrado, sin deformar) con `color` como
    /// `currentColor`.
    func pintar(en ctx: inout GraphicsContext, caja: CGRect, color: Color) {
        let escala = min(caja.width / vista.width, caja.height / vista.height)
        let dx = caja.midX - vista.midX * escala, dy = caja.midY - vista.midY * escala
        let base = CGAffineTransform(a: escala, b: 0, c: 0, d: escala, tx: dx, ty: dy)
        for t in trazos {
            guard let cg = TrayectoSVG.camino(t.d) else { continue }
            let total = (t.matriz ?? .identity).concatenating(base)
            var capa = ctx
            capa.opacity = t.opacidad
            if let m = t.mascara, let recorte = TrayectoSVG.camino(m) {
                capa.clip(to: Path(recorte).applying(base))
            }
            let camino = Path(cg).applying(total)
            if let pintura = Self.color(t.relleno, color) {
                capa.fill(camino, with: .color(pintura), style: FillStyle(eoFill: t.parImpar))
            }
            if let pintura = Self.color(t.trazo, color), t.grosor > 0 {
                capa.stroke(camino, with: .color(pintura),
                            style: StrokeStyle(lineWidth: t.grosor * escala,
                                               lineCap: t.redondo ? .round : .butt,
                                               lineJoin: t.redondo ? .round : .miter))
            }
        }
    }

    private static func color(_ p: PinturaIcono, _ actual: Color) -> Color? {
        switch p {
        case .nada: return nil
        case .actual: return actual
        case .blanco: return .white
        }
    }
}

/// El icono en SwiftUI, del lado y el color que se le diga.
struct IconoLienzoVista: View {
    let icono: IconoLienzo
    var lado: CGFloat = 16
    var color: Color

    var body: some View {
        Canvas { ctx, tam in
            icono.pintar(en: &ctx, caja: CGRect(origin: .zero, size: tam), color: color)
        }
        .frame(width: lado, height: lado)
        .accessibilityHidden(true)
    }
}

/// **El lector de trazados SVG** (`<path d="...">`): todas las ordenes de
/// SVG 1.1 (M L H V C S Q T A Z, en mayuscula absolutas y en minuscula
/// relativas), con las repeticiones implicitas y los numeros pegados
/// (`.365.207`, `1-3.334`). Los arcos se pasan a curvas de Bezier.
enum TrayectoSVG {
    /// Cache: los iconos se pintan en cada fotograma de la barra.
    @MainActor private static var hechos: [String: CGPath] = [:]

    /// El trazado, o `nil` si `d` no se entiende.
    static func camino(_ d: String) -> CGPath? {
        if Thread.isMainThread {
            return MainActor.assumeIsolated {
                if let c = hechos[d] { return c }
                let c = leer(d)
                if let c { hechos[d] = c }
                return c
            }
        }
        return leer(d)
    }

    /// Lee sin cache. Publico para las pruebas.
    static func leer(_ d: String) -> CGPath? {
        var lector = Lector(Array(d.utf8))
        let p = CGMutablePath()
        var actual = CGPoint.zero, inicio = CGPoint.zero
        var control: CGPoint? // el ultimo control de C/S o Q/T, para reflejarlo
        var anterior: UInt8 = 0
        var orden: UInt8 = 0
        while true {
            lector.saltarBlancos()
            guard let c = lector.mirar() else { break }
            if Lector.esOrden(c) {
                orden = c
                lector.avanzar()
            } else if orden == 0 {
                return nil
            } else if orden == UInt8(ascii: "M") {
                orden = UInt8(ascii: "L") // pares tras M: rectas
            } else if orden == UInt8(ascii: "m") {
                orden = UInt8(ascii: "l")
            }
            let rel = orden >= UInt8(ascii: "a")
            let base = rel ? actual : .zero
            switch orden | 0x20 { // en minuscula
            case UInt8(ascii: "z"):
                p.closeSubpath()
                actual = inicio
                control = nil
                anterior = orden
                continue
            case UInt8(ascii: "m"):
                guard let x = lector.numero(), let y = lector.numero() else { return nil }
                actual = CGPoint(x: base.x + x, y: base.y + y)
                inicio = actual
                p.move(to: actual)
                control = nil
            case UInt8(ascii: "l"):
                guard let x = lector.numero(), let y = lector.numero() else { return nil }
                actual = CGPoint(x: base.x + x, y: base.y + y)
                p.addLine(to: actual)
                control = nil
            case UInt8(ascii: "h"):
                guard let x = lector.numero() else { return nil }
                actual = CGPoint(x: (rel ? actual.x : 0) + x, y: actual.y)
                p.addLine(to: actual)
                control = nil
            case UInt8(ascii: "v"):
                guard let y = lector.numero() else { return nil }
                actual = CGPoint(x: actual.x, y: (rel ? actual.y : 0) + y)
                p.addLine(to: actual)
                control = nil
            case UInt8(ascii: "c"):
                guard let x1 = lector.numero(), let y1 = lector.numero(), let x2 = lector.numero(),
                      let y2 = lector.numero(), let x = lector.numero(), let y = lector.numero() else { return nil }
                let c2 = CGPoint(x: base.x + x2, y: base.y + y2)
                actual = CGPoint(x: base.x + x, y: base.y + y)
                p.addCurve(to: actual, control1: CGPoint(x: base.x + x1, y: base.y + y1), control2: c2)
                control = c2
            case UInt8(ascii: "s"):
                guard let x2 = lector.numero(), let y2 = lector.numero(), let x = lector.numero(),
                      let y = lector.numero() else { return nil }
                let c1 = Self.reflejo(control, actual, si: anterior, de: "cs")
                let c2 = CGPoint(x: base.x + x2, y: base.y + y2)
                actual = CGPoint(x: base.x + x, y: base.y + y)
                p.addCurve(to: actual, control1: c1, control2: c2)
                control = c2
            case UInt8(ascii: "q"):
                guard let x1 = lector.numero(), let y1 = lector.numero(), let x = lector.numero(),
                      let y = lector.numero() else { return nil }
                let c1 = CGPoint(x: base.x + x1, y: base.y + y1)
                actual = CGPoint(x: base.x + x, y: base.y + y)
                p.addQuadCurve(to: actual, control: c1)
                control = c1
            case UInt8(ascii: "t"):
                guard let x = lector.numero(), let y = lector.numero() else { return nil }
                let c1 = Self.reflejo(control, actual, si: anterior, de: "qt")
                actual = CGPoint(x: base.x + x, y: base.y + y)
                p.addQuadCurve(to: actual, control: c1)
                control = c1
            case UInt8(ascii: "a"):
                guard let rx = lector.numero(), let ry = lector.numero(), let giro = lector.numero(),
                      let grande = lector.bandera(), let horario = lector.bandera(),
                      let x = lector.numero(), let y = lector.numero() else { return nil }
                let fin = CGPoint(x: base.x + x, y: base.y + y)
                Self.arco(p, desde: actual, hasta: fin, rx: rx, ry: ry, giroGrados: giro,
                          grande: grande, horario: horario)
                actual = fin
                control = nil
            default:
                return nil
            }
            anterior = orden
        }
        return p.isEmpty ? nil : p
    }

    /// El control reflejado de S/T: el anterior girado sobre el punto
    /// actual si la orden de antes era de su familia; si no, el punto.
    private static func reflejo(_ control: CGPoint?, _ actual: CGPoint, si anterior: UInt8,
                                de familia: String) -> CGPoint {
        guard let control, familia.utf8.contains(anterior | 0x20) else { return actual }
        return CGPoint(x: 2 * actual.x - control.x, y: 2 * actual.y - control.y)
    }

    /// El arco de SVG (de extremo a extremo) en curvas de Bezier: se pasa a
    /// centro y angulos (SVG 1.1, F.6.5) y se parte en tramos de 90° o menos.
    static func arco(_ p: CGMutablePath, desde a: CGPoint, hasta b: CGPoint, rx rx0: CGFloat, ry ry0: CGFloat,
                     giroGrados: CGFloat, grande: Bool, horario: Bool) {
        var rx = abs(rx0), ry = abs(ry0)
        if a == b { return }
        if rx == 0 || ry == 0 {
            p.addLine(to: b)
            return
        }
        let phi = giroGrados * .pi / 180
        let (cosf, senf) = (cos(phi), sin(phi))
        let dx = (a.x - b.x) / 2, dy = (a.y - b.y) / 2
        let x1 = cosf * dx + senf * dy, y1 = -senf * dx + cosf * dy
        // Radios que no llegan: se agrandan lo justo.
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry)
        if lambda > 1 {
            rx *= sqrt(lambda)
            ry *= sqrt(lambda)
        }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1
        var k = den == 0 ? 0 : sqrt(max(0, num / den))
        if grande == horario { k = -k }
        let cxp = k * rx * y1 / ry, cyp = -k * ry * x1 / rx
        let cx = cosf * cxp - senf * cyp + (a.x + b.x) / 2
        let cy = senf * cxp + cosf * cyp + (a.y + b.y) / 2
        func angulo(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat {
            atan2(ux * vy - uy * vx, ux * vx + uy * vy)
        }
        let t1 = angulo(1, 0, (x1 - cxp) / rx, (y1 - cyp) / ry)
        var dt = angulo((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry)
        if !horario, dt > 0 { dt -= 2 * .pi }
        if horario, dt < 0 { dt += 2 * .pi }
        let tramos = max(1, Int(ceil(abs(dt) / (.pi / 2) - 1e-9)))
        let paso = dt / CGFloat(tramos)
        let alfa = 4.0 / 3.0 * tan(paso / 4)
        func punto(_ t: CGFloat) -> CGPoint {
            CGPoint(x: cx + rx * cos(t) * cosf - ry * sin(t) * senf,
                    y: cy + rx * cos(t) * senf + ry * sin(t) * cosf)
        }
        func derivada(_ t: CGFloat) -> CGPoint {
            CGPoint(x: -rx * sin(t) * cosf - ry * cos(t) * senf,
                    y: -rx * sin(t) * senf + ry * cos(t) * cosf)
        }
        var t = t1
        for i in 0..<tramos {
            let t2 = t + paso
            let p1 = punto(t), p2 = i == tramos - 1 ? b : punto(t2)
            let d1 = derivada(t), d2 = derivada(t2)
            p.addCurve(to: p2,
                       control1: CGPoint(x: p1.x + alfa * d1.x, y: p1.y + alfa * d1.y),
                       control2: CGPoint(x: p2.x - alfa * d2.x, y: p2.y - alfa * d2.y))
            t = t2
        }
    }

    /// Lee numeros y banderas de un trazado, byte a byte.
    private struct Lector {
        let b: [UInt8]
        var i = 0

        init(_ b: [UInt8]) { self.b = b }

        static func esOrden(_ c: UInt8) -> Bool {
            "MmLlHhVvCcSsQqTtAaZz".utf8.contains(c)
        }

        func mirar() -> UInt8? { i < b.count ? b[i] : nil }
        mutating func avanzar() { i += 1 }

        mutating func saltarBlancos() {
            while i < b.count, b[i] == 0x20 || b[i] == 0x2c || b[i] == 0x0a || b[i] == 0x09 || b[i] == 0x0d {
                i += 1
            }
        }

        /// Un `0` o `1` de los arcos: pueden ir pegados (`a2 2 0 105 5`).
        mutating func bandera() -> Bool? {
            saltarBlancos()
            guard let c = mirar(), c == 0x30 || c == 0x31 else { return nil }
            i += 1
            return c == 0x31
        }

        mutating func numero() -> CGFloat? {
            saltarBlancos()
            let ini = i
            if i < b.count, b[i] == 0x2b || b[i] == 0x2d { i += 1 }
            var punto = false, cifras = false
            while i < b.count {
                let c = b[i]
                if c >= 0x30, c <= 0x39 {
                    cifras = true
                } else if c == 0x2e, !punto {
                    punto = true
                } else {
                    break
                }
                i += 1
            }
            if cifras, i < b.count, b[i] == 0x65 || b[i] == 0x45 { // exponente
                var j = i + 1
                if j < b.count, b[j] == 0x2b || b[j] == 0x2d { j += 1 }
                if j < b.count, b[j] >= 0x30, b[j] <= 0x39 {
                    i = j
                    while i < b.count, b[i] >= 0x30, b[i] <= 0x39 { i += 1 }
                }
            }
            guard cifras, let s = String(bytes: b[ini..<i], encoding: .ascii), let v = Double(s) else {
                i = ini
                return nil
            }
            return CGFloat(v)
        }
    }
}
