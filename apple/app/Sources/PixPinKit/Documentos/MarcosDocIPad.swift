// **Los marcos numerados de los documentos** (Windows, 19-oct,
// `marcos_doc.rs`; el Mac, `MarcosDoc.swift`): el boton del marco del riel
// (o la I del teclado) entra y sale del modo; arrastrar con el lapiz o el
// dedo deja un marco (su numero es su sitio); «Quitar el último» (Supr)
// quita el ultimo; imprimir (⌘P) los imprime todos en grupo, una hoja por
// marco. Las fichas 1, 2, 3… abajo a la izquierda llevan a cada uno.
//
// Se guardan donde Windows: en el `anot-<uid>.marcos.json` que viaja si el
// documento es un adjunto (`rutaAnotadaDelAdjunto`) o, si no, junto al
// fichero en `<fichero>.pixpin-marcos`. El JSON es el de Windows: `caja`
// `[x0, y0, x1, y1]` en las unidades del lector (`HojasDelLector`) y
// `parte` = la hoja (en un libro reflujado).

import PDFKit
import PixPinCore
import SwiftUI
import UIKit

struct MarcoDoc: Codable, Equatable {
    var caja: [Double]
    var parte: UInt32 = 0
    var vista: String?

    var rect: CGRect { CGRect(x: caja[0], y: caja[1], width: caja[2] - caja[0], height: caja[3] - caja[1]) }

    init(caja: [Double], parte: UInt32 = 0, vista: String? = nil) {
        self.caja = caja
        self.parte = parte
        self.vista = vista
    }

    /// Lo que falte (un marco de antes sin `parte`) va con su valor de siempre.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        caja = (try? c.decode([Double].self, forKey: .caja)) ?? []
        parte = (try? c.decodeIfPresent(UInt32.self, forKey: .parte)) ?? 0
        vista = try? c.decodeIfPresent(String.self, forKey: .vista)
    }
}

@MainActor
final class MarcosDoc: ObservableObject {
    @Published private(set) var lista: [MarcoDoc] = []
    @Published var activo = false
    private var fichero: URL?
    /// Si viaja (es `anot-<uid>.marcos.json` de un adjunto): vaciar escribe
    /// `[]`, nunca borra (no hay lapidas de ficheros; volveria).
    private var viaja = false

    init(documento: URL?, nucleo: Nucleo? = nil) {
        (fichero, viaja) = MarcosDoc.donde(documento, nucleo)
        if let f = fichero, let d = try? Data(contentsOf: f) {
            lista = MarcosDoc.leer(d)
        }
    }

    private static func donde(_ documento: URL?, _ nucleo: Nucleo?) -> (URL?, Bool) {
        if let d = documento, let n = nucleo,
           let r = n.rutaAnotadaDelAdjunto(documento: d.path, terminacion: ".marcos.json") {
            return (URL(fileURLWithPath: r), true)
        }
        return (documento.map { $0.appendingPathExtension("pixpin-marcos") }, false)
    }

    /// El documento cambio de nombre (o de sitio): los marcos se van con el.
    func mudar(a documento: URL, nucleo: Nucleo?) {
        let antes = fichero
        (fichero, viaja) = MarcosDoc.donde(documento, nucleo)
        guard fichero != antes else { return }
        if let f = fichero, let d = try? Data(contentsOf: f), !MarcosDoc.leer(d).isEmpty {
            lista = MarcosDoc.leer(d)
            return
        }
        if !lista.isEmpty {
            guardar()
            if let antes, !viaja { try? FileManager.default.removeItem(at: antes) }
        }
    }

    /// Lo puro de leer el fichero: los marcos con sus cuatro numeros.
    nonisolated static func leer(_ d: Data) -> [MarcoDoc] {
        ((try? JSONDecoder().decode([MarcoDoc].self, from: d)) ?? []).filter { $0.caja.count == 4 }
    }

    func poner(_ r: CGRect, parte: Int) {
        lista.append(MarcoDoc(caja: [r.minX, r.minY, r.maxX, r.maxY], parte: UInt32(max(parte, 0))))
        guardar()
    }

    @discardableResult
    func quitarUltimo() -> Bool {
        guard lista.popLast() != nil else { return false }
        guardar()
        return true
    }

    func borrar() {
        lista = []
        guardar()
    }

    private func guardar() {
        guard let f = fichero else { return }
        if lista.isEmpty, !viaja {
            try? FileManager.default.removeItem(at: f)
            return
        }
        let e = JSONEncoder()
        e.outputFormatting = [.prettyPrinted]
        try? e.encode(lista).write(to: f, options: .atomic)
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }
}

/// **Las unidades del lector de Windows** (`pixpin_docs::vista::Hojas`):
/// cada hoja 1400 de ancho, una debajo de otra con 18 de hueco, Y hacia
/// abajo. En ellas van los marcos de un PDF, para que viajen con Windows.
struct HojasDelLector: Equatable {
    static let ancho: CGFloat = 1400
    static let hueco: CGFloat = 18
    let arriba: [CGFloat]
    let altos: [CGFloat]

    /// `tamanos`: el ancho y el alto de cada hoja (en puntos del PDF).
    init(tamanos: [CGSize]) {
        var y: CGFloat = 0, a: [CGFloat] = [], h: [CGFloat] = []
        for b in tamanos {
            let p = b.width > 0 && b.height > 0 ? min(max(b.height / b.width, 0.05), 20) : 1.414
            a.append(y)
            h.append(HojasDelLector.ancho * p)
            y += HojasDelLector.ancho * p + HojasDelLector.hueco
        }
        arriba = a
        altos = h
    }

    init(_ doc: PDFDocument, caja: PDFDisplayBox) {
        self.init(tamanos: (0 ..< doc.pageCount).map { doc.page(at: $0)?.bounds(for: caja).size ?? .zero })
    }

    /// De un punto de la hoja `i` (PDF, Y arriba) a unidades.
    func unidades(_ p: CGPoint, hoja i: Int, bounds b: CGRect) -> CGPoint {
        guard i >= 0, i < arriba.count, b.width > 0, b.height > 0 else { return .zero }
        return CGPoint(x: (p.x - b.minX) / b.width * HojasDelLector.ancho,
                       y: arriba[i] + (b.maxY - p.y) / b.height * altos[i])
    }

    /// La hoja de una Y en unidades (la mas cercana).
    func hoja(de y: CGFloat) -> Int {
        var i = 0
        while i + 1 < arriba.count, y >= arriba[i + 1] - HojasDelLector.hueco / 2 { i += 1 }
        return i
    }

    /// De unidades a un punto de su hoja (PDF, Y arriba).
    func punto(_ u: CGPoint, bounds b: CGRect, hoja i: Int) -> CGPoint {
        guard i >= 0, i < arriba.count, altos[i] > 0 else { return .zero }
        return CGPoint(x: b.minX + u.x / HojasDelLector.ancho * b.width,
                       y: b.maxY - (u.y - arriba[i]) / altos[i] * b.height)
    }

    /// Un marco dentro del rectangulo de su hoja `i` tal como se ve (`hoja`,
    /// Y hacia abajo). `nil` si el marco es de otra hoja.
    func rect(_ m: MarcoDoc, enHoja i: Int, vista hoja: CGRect) -> CGRect? {
        guard m.caja.count == 4, i >= 0, i < arriba.count, self.hoja(de: CGFloat(m.caja[1])) == i else { return nil }
        let kx = hoja.width / HojasDelLector.ancho
        let ky = hoja.height / max(altos[i], 1)
        let x0 = CGFloat(min(m.caja[0], m.caja[2])), x1 = CGFloat(max(m.caja[0], m.caja[2]))
        let y0 = CGFloat(min(m.caja[1], m.caja[3])) - arriba[i], y1 = CGFloat(max(m.caja[1], m.caja[3])) - arriba[i]
        return CGRect(x: hoja.minX + x0 * kx, y: hoja.minY + y0 * ky, width: (x1 - x0) * kx, height: (y1 - y0) * ky)
    }
}

enum PintarMarcos {
    static let azul = UIColor(red: 0, green: 0x78 / 255, blue: 0xD4 / 255, alpha: 1)

    /// Un marco con su numero en la esquina (rayas y numero del mismo
    /// tamano en pantalla: `k` puntos de pantalla por punto de `ctx`).
    static func pintar(_ r: CGRect, numero: Int, en ctx: CGContext, k: CGFloat = 1) {
        let k = max(k, 0.0001)
        ctx.saveGState()
        ctx.setStrokeColor(azul.cgColor)
        ctx.setLineWidth(1.4 / k)
        ctx.stroke(r)
        let t = "\(numero)" as NSString
        let atr: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: 10 / k, weight: .semibold),
                                                  .foregroundColor: UIColor.white]
        let s = t.size(withAttributes: atr)
        let caja = CGRect(x: r.minX, y: r.minY, width: max(16 / k, s.width + 6 / k), height: 16 / k)
        ctx.setFillColor(azul.cgColor)
        ctx.fill(caja)
        UIGraphicsPushContext(ctx)
        t.draw(at: CGPoint(x: caja.midX - s.width / 2, y: caja.midY - s.height / 2), withAttributes: atr)
        UIGraphicsPopContext()
        ctx.restoreGState()
    }

    /// **Los marcos en grupo, para imprimir**: cada marco, recortado de su
    /// hoja de `documento` (con la tinta si la lleva quemada), en su propia
    /// hoja de un PDF nuevo.
    static func pdf(_ marcos: [MarcoDoc], documento: PDFDocument) -> Data? {
        let h = HojasDelLector(documento, caja: .mediaBox)
        let datos = NSMutableData()
        guard !marcos.isEmpty, let consumidor = CGDataConsumer(data: datos as CFMutableData),
              let ctx = CGContext(consumer: consumidor, mediaBox: nil, nil) else { return nil }
        var hojas = 0
        for m in marcos where m.caja.count == 4 {
            let i = h.hoja(de: CGFloat(m.caja[1]))
            guard let p = documento.page(at: i) else { continue }
            let b = p.bounds(for: .mediaBox)
            let a = h.punto(CGPoint(x: m.caja[0], y: m.caja[1]), bounds: b, hoja: i)
            let z = h.punto(CGPoint(x: m.caja[2], y: m.caja[3]), bounds: b, hoja: i)
            let r = CGRect(x: min(a.x, z.x), y: min(a.y, z.y), width: abs(a.x - z.x), height: abs(a.y - z.y))
            guard r.width > 1, r.height > 1 else { continue }
            var caja = CGRect(origin: .zero, size: r.size)
            ctx.beginPage(mediaBox: &caja)
            ctx.translateBy(x: -r.minX, y: -r.minY)
            ctx.clip(to: r)
            p.draw(with: .mediaBox, to: ctx)
            ctx.endPage()
            hojas += 1
        }
        ctx.closePDF()
        return hojas > 0 ? datos as Data : nil
    }
}

/// El recuadro que se va dibujando al arrastrar un marco nuevo.
final class GomaDeMarcoIPad: UIView {
    var rect: CGRect? { didSet { setNeedsDisplay() } }

    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        isOpaque = false
        backgroundColor = .clear
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override func draw(_ r: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext(), let rect else { return }
        ctx.setStrokeColor(PintarMarcos.azul.cgColor)
        ctx.setLineWidth(1.2)
        ctx.setLineDash(phase: 0, lengths: [5, 3])
        ctx.stroke(rect)
        ctx.setFillColor(PintarMarcos.azul.withAlphaComponent(0.08).cgColor)
        ctx.fill(rect)
    }
}

/// **Las fichas de los marcos** (Windows, 20-oct): abajo a la izquierda, una
/// por marco (1, 2, 3…); tocar una va a ese marco. Poniendo marcos, a su
/// derecha la pista y los botones de quitar el ultimo e imprimir.
struct FichasDeMarcos: View {
    @ObservedObject var marcos: MarcosDoc
    /// Solo las de esta parte (la hoja de un libro); `nil`, todas.
    var parte: Int?
    let ir: (Int) -> Void
    let imprimir: () -> Void

    var body: some View {
        let visibles = marcos.lista.enumerated().filter { parte == nil || Int($0.element.parte) == parte }
        if !visibles.isEmpty || marcos.activo {
            HStack(spacing: 6) {
                ForEach(visibles, id: \.offset) { i, _ in
                    Button { ir(i) } label: {
                        Text("\(i + 1)").font(.system(size: 13, weight: .semibold)).foregroundStyle(.white)
                            .frame(width: 32, height: 32)
                            .background(RoundedRectangle(cornerRadius: 6).fill(Color(uiColor: PintarMarcos.azul)))
                    }
                    .buttonStyle(.plain)
                    .help("Ir al marco \(i + 1)")
                }
                if marcos.activo {
                    HStack(spacing: 10) {
                        Text(visibles.isEmpty ? "Arrastra para poner un marco"
                             : "Arrastra: otro marco")
                            .font(.system(size: 12))
                        if !visibles.isEmpty {
                            Button { marcos.quitarUltimo() } label: { Image(systemName: "delete.left") }
                                .keyboardShortcut(.delete, modifiers: [])
                                .help("Quitar el último (Supr)")
                            Button(action: imprimir) { Image(systemName: "printer") }
                                .help("Imprimir los marcos (⌘P)")
                        }
                        Button { marcos.activo = false } label: { Image(systemName: "xmark") }
                            .help("Dejar de poner marcos (I)")
                    }
                    .foregroundStyle(.white)
                    .buttonStyle(.plain)
                    .padding(.horizontal, 12).frame(height: 32)
                    .background(Capsule().fill(Color(white: 0.08, opacity: 0.9)))
                }
            }
            .fixedSize()
        }
    }
}
