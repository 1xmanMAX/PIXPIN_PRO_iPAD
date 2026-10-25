import SwiftUI

/// **Los iconos de Windows y del movil** (Material Icons, relleno): los mismos
/// trazados (`TrazadosMaterial`), pintados con el color de quien los usa.
struct IconoMaterial: View {
    let nombre: String
    var lado: CGFloat = 18

    var body: some View {
        let trazos = TrazadosMaterial.todos[nombre] ?? []
        ZStack {
            ForEach(Array(trazos.enumerated()), id: \.offset) { _, t in
                FormaMaterial(d: t.0).fill(style: FillStyle(eoFill: t.1))
            }
        }
        .frame(width: lado, height: lado)
    }

    /// El de Windows para un simbolo del Mac (los botones se escribieron con
    /// SF Symbols); `nil` si no hay equivalente y se queda el del Mac.
    static func deSimbolo(_ s: String) -> String? {
        let tabla: [String: String] = [
            "pencil": "edit", "square.and.pencil": "edit_note", "eye": "visibility", "xmark": "close",
            "pin": "push_pin", "pin.fill": "push_pin",
            "moon": "dark_mode", "sun.max": "light_mode",
            "arrow.up.left.and.arrow.down.right": "fullscreen",
            "arrow.down.right.and.arrow.up.left": "fullscreen_exit",
            "speaker.wave.2": "volume_up", "chevron.down.2": "keyboard_arrow_down",
            "bookmark": "bookmark_add", "square.and.arrow.up": "ios_share",
            "gearshape": "settings", "printer": "crop_free", "play.fill": "play_arrow",
            "list.bullet": "list", "magnifyingglass": "search", "text.magnifyingglass": "search",
            "arrow.clockwise": "replay", "arrow.uturn.backward": "undo", "arrow.uturn.forward": "redo",
            "hand.raised": "drag_indicator", "ruler": "straighten", "square.3.layers.3d": "layers",
            "sidebar.right": "layers", "cube": "view_in_ar", "textformat": "font_download",
            "trash": "delete", "photo.on.rectangle": "photo_library", "clock.arrow.circlepath": "timeline",
            "checklist": "checklist", "arrow.triangle.2.circlepath": "wifi", "globe": "public",
            "rectangle.dashed": "crop_free", "viewfinder": "crop_free", "bold": "format_bold",
            "italic": "format_italic", "underline": "format_underlined", "strikethrough": "format_strikethrough",
            "text.alignleft": "format_align_left", "text.aligncenter": "format_align_center",
            "text.alignright": "format_align_right", "text.word.spacing": "wrap_text", "eraser": "format_clear",
            "circle.grid.2x3.fill": "drag_indicator", "ellipsis": "more_vert", "doc.on.doc": "content_copy",
            "folder": "folder", "mic": "mic", "paperclip": "attach_file", "plus": "add",
            "arrow.up.right.square": "open_in_new", "chevron.left": "chevron_left",
        ]
        guard let n = tabla[s], TrazadosMaterial.todos[n] != nil else { return nil }
        return n
    }
}

/// Un simbolo: el de Windows si lo hay, si no el del Mac.
struct Simbolo: View {
    let nombre: String
    var lado: CGFloat = 16

    var body: some View {
        if let m = IconoMaterial.deSimbolo(nombre) {
            IconoMaterial(nombre: m, lado: lado + 2)
        } else {
            Image(systemName: nombre).font(.system(size: lado * 0.85))
        }
    }
}

/// Un trazado SVG (`d`) de 24 x 24 escalado a la caja.
struct FormaMaterial: Shape {
    let d: String

    func path(in r: CGRect) -> Path {
        let p = FormaMaterial.leer(d)
        let k = min(r.width, r.height) / 24
        return p.applying(CGAffineTransform(a: k, b: 0, c: 0, d: k, tx: r.minX, ty: r.minY))
    }

    nonisolated(unsafe) private static var cache: [String: Path] = [:]

    static func leer(_ d: String) -> Path {
        if let p = cache[d] { return p }
        var p = Path()
        var fichas: [String] = []
        var num = ""
        func cerrarNum() { if !num.isEmpty { fichas.append(num); num = "" } }
        for c in d {
            if c.isLetter && c != "e" && c != "E" { cerrarNum(); fichas.append(String(c)) }
            else if c == "," || c == " " || c == "\n" || c == "\t" { cerrarNum() }
            else if c == "-" && !(num.last == "e" || num.last == "E") { cerrarNum(); num = "-" }
            else if c == "." && num.contains(".") && !num.contains("e") { cerrarNum(); num = "." }
            else { num.append(c) }
        }
        cerrarNum()
        var i = 0, cmd = "M"
        var cur = CGPoint.zero, ini = CGPoint.zero, ctrl: CGPoint? = nil, qctrl: CGPoint? = nil
        func n() -> CGFloat { defer { i += 1 }; return CGFloat(Double(fichas[i]) ?? 0) }
        func hayNum() -> Bool { i < fichas.count && Double(fichas[i]) != nil }
        while i < fichas.count {
            if !hayNum() { cmd = fichas[i]; i += 1 }
            let rel = cmd == cmd.lowercased() && cmd.lowercased() != "z"
            let o = rel ? cur : .zero
            var sigue = "", ultimoC: CGPoint? = nil, ultimoQ: CGPoint? = nil
            switch cmd.lowercased() {
            case "m":
                cur = CGPoint(x: o.x + n(), y: o.y + n()); ini = cur; p.move(to: cur)
                sigue = rel ? "l" : "L"
            case "l": cur = CGPoint(x: o.x + n(), y: o.y + n()); p.addLine(to: cur)
            case "h": cur.x = (rel ? cur.x : 0) + n(); p.addLine(to: cur)
            case "v": cur.y = (rel ? cur.y : 0) + n(); p.addLine(to: cur)
            case "c":
                let c1 = CGPoint(x: o.x + n(), y: o.y + n()), c2 = CGPoint(x: o.x + n(), y: o.y + n())
                cur = CGPoint(x: o.x + n(), y: o.y + n()); p.addCurve(to: cur, control1: c1, control2: c2); ultimoC = c2
            case "s":
                let c1 = ctrl.map { CGPoint(x: 2 * cur.x - $0.x, y: 2 * cur.y - $0.y) } ?? cur
                let c2 = CGPoint(x: o.x + n(), y: o.y + n())
                cur = CGPoint(x: o.x + n(), y: o.y + n()); p.addCurve(to: cur, control1: c1, control2: c2); ultimoC = c2
            case "q":
                let c1 = CGPoint(x: o.x + n(), y: o.y + n())
                cur = CGPoint(x: o.x + n(), y: o.y + n()); p.addQuadCurve(to: cur, control: c1); ultimoQ = c1
            case "t":
                let c1 = qctrl.map { CGPoint(x: 2 * cur.x - $0.x, y: 2 * cur.y - $0.y) } ?? cur
                cur = CGPoint(x: o.x + n(), y: o.y + n()); p.addQuadCurve(to: cur, control: c1); ultimoQ = c1
            case "a":
                let rx = n(), ry = n(), rot = n(), grande = n() != 0, barrido = n() != 0
                let fin = CGPoint(x: o.x + n(), y: o.y + n())
                arco(&p, cur, fin, rx, ry, rot, grande, barrido); cur = fin
            case "z": p.closeSubpath(); cur = ini
            default: i += 1
            }
            ctrl = ultimoC; qctrl = ultimoQ
            if !sigue.isEmpty { cmd = sigue }
        }
        cache[d] = p
        return p
    }

    /// Un arco eliptico del SVG en curvas de Bezier.
    private static func arco(_ p: inout Path, _ a: CGPoint, _ b: CGPoint, _ rx0: CGFloat, _ ry0: CGFloat,
                             _ rotGrados: CGFloat, _ grande: Bool, _ barrido: Bool) {
        var rx = abs(rx0), ry = abs(ry0)
        guard rx > 0, ry > 0, a != b else { p.addLine(to: b); return }
        let fi = rotGrados * .pi / 180, cf = cos(fi), sf = sin(fi)
        let dx = (a.x - b.x) / 2, dy = (a.y - b.y) / 2
        let x1 = cf * dx + sf * dy, y1 = -sf * dx + cf * dy
        let l = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry)
        if l > 1 { rx *= sqrt(l); ry *= sqrt(l) }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1
        var co = sqrt(max(0, num / den)); if grande == barrido { co = -co }
        let cx1 = co * rx * y1 / ry, cy1 = -co * ry * x1 / rx
        let cx = cf * cx1 - sf * cy1 + (a.x + b.x) / 2, cy = sf * cx1 + cf * cy1 + (a.y + b.y) / 2
        func ang(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat {
            let s: CGFloat = ux * vy - uy * vx < 0 ? -1 : 1
            let d = (ux * vx + uy * vy) / (sqrt(ux * ux + uy * uy) * sqrt(vx * vx + vy * vy))
            return s * acos(max(-1, min(1, d)))
        }
        let t1 = ang(1, 0, (x1 - cx1) / rx, (y1 - cy1) / ry)
        var dt = ang((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry)
        if !barrido && dt > 0 { dt -= 2 * .pi } else if barrido && dt < 0 { dt += 2 * .pi }
        let partes = Int(ceil(abs(dt) / (.pi / 2)))
        let paso = dt / CGFloat(partes), k = 4 / 3 * tan(paso / 4)
        var t = t1
        func punto(_ t: CGFloat) -> CGPoint {
            CGPoint(x: cx + rx * cos(t) * cf - ry * sin(t) * sf, y: cy + rx * cos(t) * sf + ry * sin(t) * cf)
        }
        func deriv(_ t: CGFloat) -> CGPoint {
            CGPoint(x: -rx * sin(t) * cf - ry * cos(t) * sf, y: -rx * sin(t) * sf + ry * cos(t) * cf)
        }
        for _ in 0..<partes {
            let p0 = punto(t), p3 = punto(t + paso), d0 = deriv(t), d3 = deriv(t + paso)
            p.addCurve(to: p3, control1: CGPoint(x: p0.x + k * d0.x, y: p0.y + k * d0.y),
                       control2: CGPoint(x: p3.x - k * d3.x, y: p3.y - k * d3.y))
            t += paso
        }
    }
}
