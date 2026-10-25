// **Presentar un PowerPoint como Windows** (`diapositivas.rs`; el Mac,
// `Diapositivas.swift`), con el tacto del iPad:
//
// - La presentacion va a pantalla completa, la diapositiva entera centrada
//   sobre negro, y abajo la pastilla del movil «← n / total → ✕». Tocar el
//   tercio derecho pasa a la siguiente; el izquierdo, a la anterior; el
//   medio esconde o ensena la pastilla. Deslizar a los lados tambien pasa.
// - El teclado de PowerPoint (lo que mandan los mandos de presentar): → ↓
//   AvPag ↩ Espacio siguiente; ← ↑ RePag ⌫ P anterior; Inicio/Fin; B o «.»
//   pantalla negra; N las notas en una franja abajo; T apaga o enciende las
//   transiciones; ⎋ sale.
// - **Transiciones** (fundido, empuje, barrido, cubrir, descubrir, corte) y
//   el pase solo (`advTm`): cada diapositiva se pinta una vez a la medida de
//   la pantalla y la transicion solo mezcla las dos imagenes con las capas
//   que calcula Rust (`capasDeTransicion`), las mismas que en Windows.
// - **Videos y audios**: su poster ya viene en la diapositiva; tocarlo suena
//   el audio en el reproductor de la app y abre el video con el del sistema.
// - Con AirPlay o una pantalla conectada, iPadOS duplica la pantalla: la
//   presentacion sale en el proyector tal cual.
//
// Las diapositivas las maqueta Rust (`DocumentoOffice`) y las pinta
// `PintorOffice`, fuera del hilo principal. Solo se guardan las de
// alrededor de la que se ve.

import AVKit
import PixPinCore
import QuartzCore
import SwiftUI
import UIKit

// MARK: - Las cuentas (sin ventanas: se prueban solas)

/// Lo que se le pide a la presentacion (una tecla, un toque).
public enum MandoDiapositiva: Equatable, Sendable {
    case siguiente, anterior, primera, ultima
    /// Pantalla negra (B o «.»).
    case negro
    /// Esconder o ensenar la pastilla.
    case pastilla
    /// Las notas del orador en una franja abajo (N).
    case notas
    /// Encender o apagar las transiciones (T).
    case transiciones
    case salir
}

/// Por donde va el pase. `total` es 0 mientras no se sabe.
public struct PaseDiapositivas: Equatable, Sendable {
    public var actual = 0
    public var total = 0
    public var negro = false
    public var pastilla = true

    public init(actual: Int = 0, total: Int = 0) {
        self.actual = actual
        self.total = total
    }

    /// Aplica un mando. `false` si hay que salir. No da la vuelta; con la
    /// pantalla en negro, cualquier mando de pasar primero la enciende.
    public mutating func aplicar(_ m: MandoDiapositiva) -> Bool {
        let ultima = max(total - 1, 0)
        if negro, ![.salir, .pastilla, .notas, .transiciones].contains(m) {
            negro = false
            if m == .negro { return true }
        }
        switch m {
        case .siguiente: actual = min(actual + 1, ultima)
        case .anterior: actual = max(actual - 1, 0)
        case .primera: actual = 0
        case .ultima: actual = ultima
        case .negro: negro = true
        case .pastilla: pastilla.toggle()
        case .notas, .transiciones: break
        case .salir: return false
        }
        return true
    }

    public var puedeAnterior: Bool { actual > 0 }
    public var puedeSiguiente: Bool { total == 0 || actual + 1 < total }
}

public enum CuentasDiapositivas {
    public static let segundosPastilla: CFTimeInterval = 3
    public static let segundosAviso: CFTimeInterval = 1.6

    /// Los formatos que maqueta PixPin (`pixpin-office`).
    public static let extensiones: Set<String> = ["pptx", "pptm", "ppsx", "ppsm", "potx", "potm"]

    public static func sePresenta(_ url: URL) -> Bool {
        extensiones.contains(url.pathExtension.lowercased())
    }

    /// Las teclas de PowerPoint (`UIKeyboardHIDUsage` y lo escrito).
    public static func mando(tecla: Int, caracteres: String, mayusculas: Bool) -> MandoDiapositiva? {
        switch tecla {
        case 0x4F, 0x51, 0x4E, 0x28, 0x58: return .siguiente // → ↓ AvPag ↩ Intro
        case 0x2C: return mayusculas ? .anterior : .siguiente // Espacio
        case 0x50, 0x52, 0x4B, 0x2A: return .anterior // ← ↑ RePag ⌫
        case 0x4A: return .primera // Inicio
        case 0x4D: return .ultima // Fin
        case 0x29: return .salir // ⎋
        default: break
        }
        switch caracteres.lowercased() {
        case "p": return .anterior
        case "b", ".": return .negro
        case "n": return .notas
        case "t": return .transiciones
        default: return nil
        }
    }

    /// Un toque: por tercios, como `ZonaDePasar` del movil.
    public static func mando(toqueEn x: CGFloat, ancho: CGFloat) -> MandoDiapositiva {
        let tercio = ancho / 3
        if x < tercio { return .anterior }
        if x > tercio * 2 { return .siguiente }
        return .pastilla
    }

    /// La caja donde cabe entera una diapositiva de `pagina` en `pantalla`.
    public static func encajar(_ pagina: CGSize, en pantalla: CGSize) -> CGRect {
        let pw = max(pagina.width, 1), ph = max(pagina.height, 1)
        let s = min(pantalla.width / pw, pantalla.height / ph)
        let w = pw * s, h = ph * s
        return CGRect(x: (pantalla.width - w) / 2, y: (pantalla.height - h) / 2, width: w, height: h)
    }

    public static func encajarEntera(_ pagina: CGSize, en pantalla: CGSize) -> CGRect {
        let c = encajar(pagina, en: pantalla)
        return CGRect(x: c.minX.rounded(), y: c.minY.rounded(),
                      width: max(c.width.rounded(), 1), height: max(c.height.rounded(), 1))
    }

    /// Solo hacia delante y de una en una, encendidas y si dura algo.
    public static func tocaTransicion(antes: Int, ahora: Int, encendidas: Bool, ms: UInt32?) -> Bool {
        encendidas && ahora == antes + 1 && (ms ?? 0) > 0
    }

    public static func progreso(transcurrido: CFTimeInterval, ms: UInt32) -> Float {
        guard ms > 0 else { return 1 }
        return Float(min(max(transcurrido * 1000 / Double(ms), 0), 1))
    }

    /// Un rectangulo en fracciones (izquierda, arriba, derecha, abajo).
    public static func trozo(_ f: [Float], de caja: CGRect) -> CGRect {
        guard f.count == 4 else { return caja }
        return CGRect(x: caja.minX + CGFloat(f[0]) * caja.width, y: caja.minY + CGFloat(f[1]) * caja.height,
                      width: CGFloat(f[2] - f[0]) * caja.width, height: CGFloat(f[3] - f[1]) * caja.height)
    }
}

// MARK: - La presentacion abierta (Rust)

/// **El PowerPoint abierto para presentarlo**: cada cosa se pide a Rust la
/// primera vez y se guarda. Lo que cruza el puente va fuera del hilo
/// principal.
final class ArchivoDiapositivas: @unchecked Sendable {
    let office: DocumentoOffice
    let total: Int
    let tamano: CGSize
    private let cerrojo = NSLock()
    private var notas: [Int: String?] = [:]
    private var transiciones: [Int: TransicionDiapositiva?] = [:]
    private var medios: [Int: [MedioDiapositiva]] = [:]

    private init(office: DocumentoOffice, total: Int, tamano: CGSize) {
        self.office = office
        self.total = total
        self.tamano = tamano
    }

    static func abrir(_ url: URL) -> ArchivoDiapositivas? {
        guard let office = try? DocumentoOffice.abrir(ruta: url.path, medidor: MedidorCoreText()),
              office.esPresentacion() else { return nil }
        let total = Int(office.total())
        guard total > 0, let p = office.pagina(n: 0) else { return nil }
        return ArchivoDiapositivas(office: office, total: total,
                                   tamano: CGSize(width: CGFloat(p.ancho), height: CGFloat(p.alto)))
    }

    func notasSiHay(_ i: Int) -> String?? {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return notas[i]
    }

    func notas(_ i: Int) -> String? {
        if let n = notasSiHay(i) { return n }
        let n = office.notas(n: UInt32(i))
        cerrojo.lock()
        notas[i] = .some(n)
        cerrojo.unlock()
        return n
    }

    func transicion(_ i: Int) -> TransicionDiapositiva? {
        cerrojo.lock()
        if let t = transiciones[i] {
            cerrojo.unlock()
            return t
        }
        cerrojo.unlock()
        let t = office.transicion(n: UInt32(i))
        cerrojo.lock()
        transiciones[i] = .some(t)
        cerrojo.unlock()
        return t
    }

    func medios(_ i: Int) -> [MedioDiapositiva] {
        cerrojo.lock()
        if let m = medios[i] {
            cerrojo.unlock()
            return m
        }
        cerrojo.unlock()
        let m = office.medios(n: UInt32(i))
        cerrojo.lock()
        medios[i] = m
        cerrojo.unlock()
        return m
    }

    /// **La diapositiva `i` pintada** a `pixeles`, sobre blanco. Bloquea.
    func foto(_ i: Int, pixeles: CGSize) -> CGImage? {
        let w = Int(pixeles.width.rounded()), h = Int(pixeles.height.rounded())
        guard w > 0, h > 0, let p = office.pagina(n: UInt32(i)), p.ancho > 0, p.alto > 0,
              let pintor = try? PintorOffice(pagina: p, imagen: { [office] in office.imagen(id: $0) }),
              let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: espacio, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        ctx.setFillColor(CGColor(gray: 1, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        ctx.interpolationQuality = .high
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: CGFloat(w) / CGFloat(p.ancho), y: -CGFloat(h) / CGFloat(p.alto))
        pintor.pintar(en: ctx)
        return ctx.makeImage()
    }

    /// Saca un video o un audio a un fichero temporal para reproducirlo.
    func fichero(de m: MedioDiapositiva, origen: URL) -> URL? {
        guard let parte = m.parte else { return nil }
        let nombre = String((parte.split(separator: "/").last ?? "medio").map {
            $0.isLetter || $0.isNumber || ".-_".contains($0) ? $0 : "_"
        })
        let fecha = (try? origen.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
        var h = Hasher()
        h.combine(origen.path)
        h.combine(fecha)
        h.combine(parte)
        let carpeta = FileManager.default.temporaryDirectory.appendingPathComponent("pixpin-medios", isDirectory: true)
        let ruta = carpeta.appendingPathComponent(String(format: "%016llx-", UInt64(bitPattern: Int64(h.finalize()))) + nombre)
        if FileManager.default.fileExists(atPath: ruta.path) { return ruta }
        guard let datos = office.bytesDeMedio(parte: parte) else { return nil }
        do {
            try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
            try datos.write(to: ruta, options: .atomic)
            return ruta
        } catch {
            return nil
        }
    }
}

enum TextosPase {
    static let preparando = "Preparando la presentación…"
    static let noSePudo = "No se pudo preparar la presentación"
    static let sinNotas = "Esta diapositiva no tiene notas"
    static let transicionesSi = "Transiciones activadas (T)"
    static let transicionesNo = "Transiciones desactivadas (T)"
}

// MARK: - La vista de la presentacion

/// **La presentacion**: pinta la diapositiva (y la transicion), las notas,
/// el aviso y la pastilla; recoge toques, deslizar y el teclado.
final class VistaPaseIPad: UIView {
    weak var pase: PresentacionIPad?
    /// Los botones de la pastilla pintada.
    private var botones: [(CGRect, MandoDiapositiva)] = []

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        isOpaque = true
        contentMode = .redraw
        isMultipleTouchEnabled = false
        let toque = UITapGestureRecognizer(target: self, action: #selector(tocado(_:)))
        addGestureRecognizer(toque)
        for d in [UISwipeGestureRecognizer.Direction.left, .right, .up, .down] {
            let s = UISwipeGestureRecognizer(target: self, action: #selector(deslizado(_:)))
            s.direction = d
            addGestureRecognizer(s)
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override var canBecomeFirstResponder: Bool { true }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil { becomeFirstResponder() }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        pase?.alCambiarElTamano()
    }

    @objc private func tocado(_ g: UITapGestureRecognizer) {
        guard let p = pase else { return }
        let punto = g.location(in: self)
        if p.pastillaVisible, let b = botones.first(where: { $0.0.insetBy(dx: -6, dy: -6).contains(punto) }) {
            p.mandar(b.1)
            return
        }
        if p.reproducirMedio(en: punto, caja: bounds.size) { return }
        p.mandar(CuentasDiapositivas.mando(toqueEn: punto.x, ancho: bounds.width))
    }

    @objc private func deslizado(_ g: UISwipeGestureRecognizer) {
        switch g.direction {
        case .left, .up: pase?.mandar(.siguiente)
        case .right, .down: pase?.mandar(.anterior)
        default: break
        }
    }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var hecho = false
        for p in presses {
            guard let k = p.key else { continue }
            if let m = CuentasDiapositivas.mando(tecla: k.keyCode.rawValue, caracteres: k.charactersIgnoringModifiers,
                                                 mayusculas: k.modifierFlags.contains(.shift)) {
                pase?.mandar(m)
                hecho = true
            }
        }
        if !hecho { super.pressesBegan(presses, with: event) }
    }

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        ctx.setFillColor(UIColor.black.cgColor)
        ctx.fill(bounds)
        botones = []
        guard let p = pase else { return }
        let (ancho, alto) = (bounds.width, bounds.height)
        let centrado = { (t: String, c: UIColor) in
            let s = NSAttributedString(string: t, attributes: [.font: UIFont.systemFont(ofSize: 18), .foregroundColor: c])
            let w = s.size().width
            s.draw(at: CGPoint(x: (ancho - w) / 2, y: alto / 2))
        }
        if p.fallo {
            centrado(TextosPase.noSePudo, .white)
            return
        }
        guard let archivo = p.archivo else {
            centrado(TextosPase.preparando, UIColor(white: 0.65, alpha: 1))
            return
        }
        if p.estado.negro { return }
        let caja = CuentasDiapositivas.encajarEntera(archivo.tamano, en: bounds.size)
        let nueva = p.fotos[p.estado.actual]
        if let (vieja, capas) = p.fotograma() {
            ctx.saveGState()
            ctx.clip(to: caja)
            for c in capas {
                let destino = CuentasDiapositivas.trozo(c.destino, de: caja)
                let origen = CuentasDiapositivas.trozo(c.origen, de: CGRect(x: 0, y: 0, width: 1, height: 1))
                switch c.fuente {
                case .vieja: VistaPaseIPad.imagen(vieja, en: destino, recorte: origen, alfa: CGFloat(c.alfa), ctx: ctx)
                case .nueva:
                    if let nueva { VistaPaseIPad.imagen(nueva, en: destino, recorte: origen, alfa: CGFloat(c.alfa), ctx: ctx) }
                case .negro:
                    ctx.setFillColor(UIColor.black.withAlphaComponent(CGFloat(c.alfa)).cgColor)
                    ctx.fill(destino)
                }
            }
            ctx.restoreGState()
        } else if let nueva {
            VistaPaseIPad.imagen(nueva, en: caja, recorte: nil, alfa: 1, ctx: ctx)
        }
        var suelo = alto - safeAreaInsets.bottom
        if p.notasAbajo {
            let margen: CGFloat = 24
            let notas = archivo.notasSiHay(p.estado.actual) ?? nil
            let estilo = NSMutableParagraphStyle()
            estilo.lineBreakMode = .byWordWrapping
            let t = NSAttributedString(string: notas ?? TextosPase.sinNotas, attributes: [
                .font: UIFont.systemFont(ofSize: 20),
                .foregroundColor: notas == nil ? UIColor(white: 0.65, alpha: 1) : UIColor.white,
                .paragraphStyle: estilo,
            ])
            let h = ceil(t.boundingRect(with: CGSize(width: ancho - 2 * margen, height: .greatestFiniteMagnitude),
                                        options: [.usesLineFragmentOrigin, .usesFontLeading], context: nil).height)
            let fh = min(h + 2 * margen, alto * 0.33)
            let franja = CGRect(x: 0, y: alto - fh, width: ancho, height: fh)
            ctx.setFillColor(UIColor(red: 0.16, green: 0.16, blue: 0.18, alpha: 0.92).cgColor)
            ctx.fill(franja)
            ctx.saveGState()
            ctx.clip(to: franja)
            t.draw(with: CGRect(x: margen, y: franja.minY + margen, width: ancho - 2 * margen, height: fh),
                   options: [.usesLineFragmentOrigin, .usesFontLeading], context: nil)
            ctx.restoreGState()
            suelo = franja.minY
        }
        if let aviso = p.avisoVisible {
            let s = NSAttributedString(string: aviso, attributes: [.font: UIFont.systemFont(ofSize: 16),
                                                                   .foregroundColor: UIColor.white])
            let tam = s.size()
            let c = CGRect(x: (ancho - tam.width) / 2 - 16, y: 32 + safeAreaInsets.top, width: tam.width + 32,
                           height: tam.height + 16)
            ctx.setFillColor(UIColor(red: 0.16, green: 0.16, blue: 0.18, alpha: 0.9).cgColor)
            ctx.addPath(UIBezierPath(roundedRect: c, cornerRadius: c.height / 2).cgPath)
            ctx.fillPath()
            s.draw(at: CGPoint(x: c.minX + 16, y: c.minY + 8))
        }
        guard p.pastillaVisible else { return }
        let lado: CGFloat = 48
        let cuenta = NSAttributedString(string: "\(p.estado.actual + 1) / \(p.estado.total)",
                                        attributes: [.font: UIFont.monospacedDigitSystemFont(ofSize: 16, weight: .regular),
                                                     .foregroundColor: UIColor.white])
        let tc = cuenta.size()
        let total = lado * 3 + tc.width + 30
        let c = CGRect(x: (ancho - total) / 2, y: suelo - lado - 28, width: total, height: lado)
        ctx.setFillColor(UIColor(red: 0.16, green: 0.16, blue: 0.18, alpha: 0.86).cgColor)
        ctx.addPath(UIBezierPath(roundedRect: c, cornerRadius: lado / 2).cgPath)
        ctx.fillPath()
        var x = c.minX + 6
        let boton = { (icono: String, activo: Bool, mando: MandoDiapositiva, x: CGFloat) in
            let zona = CGRect(x: x, y: c.minY, width: lado, height: lado)
            let conf = UIImage.SymbolConfiguration(pointSize: 18, weight: .semibold)
            if let img = UIImage(systemName: icono, withConfiguration: conf)?
                .withTintColor(activo ? .white : UIColor.white.withAlphaComponent(0.35), renderingMode: .alwaysOriginal) {
                let s = img.size
                img.draw(in: CGRect(x: zona.midX - s.width / 2, y: zona.midY - s.height / 2, width: s.width, height: s.height))
            }
            self.botones.append((zona, mando))
        }
        boton("arrow.left", p.estado.puedeAnterior, .anterior, x)
        x += lado
        cuenta.draw(at: CGPoint(x: x + 4, y: c.minY + (lado - tc.height) / 2))
        x += tc.width + 8
        boton("arrow.right", p.estado.puedeSiguiente, .siguiente, x)
        x += lado + 10
        boton("xmark", true, .salir, x)
    }

    /// Una imagen (o un trozo, en fracciones) en `r`, con su alfa.
    private static func imagen(_ img: CGImage, en r: CGRect, recorte: CGRect?, alfa: CGFloat, ctx: CGContext) {
        var fuente = img
        if let f = recorte, f != CGRect(x: 0, y: 0, width: 1, height: 1) {
            let w = CGFloat(img.width), h = CGFloat(img.height)
            let px = CGRect(x: f.minX * w, y: f.minY * h, width: f.width * w, height: f.height * h).integral
            guard px.width >= 1, px.height >= 1, let t = img.cropping(to: px) else { return }
            fuente = t
        }
        ctx.saveGState()
        ctx.setAlpha(alfa)
        ctx.translateBy(x: r.minX, y: r.maxY)
        ctx.scaleBy(x: 1, y: -1)
        ctx.interpolationQuality = .high
        ctx.draw(fuente, in: CGRect(origin: .zero, size: r.size))
        ctx.restoreGState()
    }
}

// MARK: - La presentacion

@MainActor
final class PresentacionIPad: NSObject, ObservableObject {
    let url: URL
    let vista = VistaPaseIPad()
    var alSalir: ((Int) -> Void)?
    private(set) var archivo: ArchivoDiapositivas?
    private(set) var fallo = false
    private(set) var estado: PaseDiapositivas
    private(set) var fotos: [Int: CGImage] = [:]
    private(set) var notasAbajo = false
    private var transiciones = true
    private var pidiendo: Set<Int> = []
    private var vista_: Int
    private var visibleDesde: CFTimeInterval = 0
    private var enCurso: (vieja: CGImage, efecto: EfectoDiapositiva, ms: UInt32, empezo: CFTimeInterval)?
    private var enlace: CADisplayLink?
    private var avance: Timer?
    private var esconder: Timer?
    private var pastillaHasta: CFTimeInterval = .infinity
    private var aviso: (texto: String, hasta: CFTimeInterval)?
    private var tamanoPedido: CGSize = .zero
    private var cerrada = false
    private let cola = DispatchQueue(label: "pixpin.diapositivas", qos: .userInitiated)

    init(url: URL, desde: Int) {
        self.url = url
        estado = PaseDiapositivas(actual: max(desde, 0))
        vista_ = max(desde, 0)
        super.init()
        vista.pase = self
    }

    var pastillaVisible: Bool { estado.pastilla && CACurrentMediaTime() < pastillaHasta }
    var avisoVisible: String? {
        guard let a = aviso, CACurrentMediaTime() < a.hasta else { return nil }
        return a.texto
    }

    func empezar() {
        UIApplication.shared.isIdleTimerDisabled = true
        ensenarPastilla()
        let url = self.url
        cola.async { [weak self] in
            let a = ArchivoDiapositivas.abrir(url)
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.abierto(a) } }
        }
    }

    private func abierto(_ a: ArchivoDiapositivas?) {
        guard !cerrada else { return }
        guard let a else {
            fallo = true
            vista.setNeedsDisplay()
            return
        }
        archivo = a
        estado.total = a.total
        estado.actual = min(estado.actual, a.total - 1)
        vista_ = estado.actual
        visibleDesde = CACurrentMediaTime()
        pedirFotos()
        programarAvance()
        vista.setNeedsDisplay()
    }

    func alCambiarElTamano() {
        guard vista.bounds.size != tamanoPedido else { return }
        tamanoPedido = vista.bounds.size
        fotos = [:]
        pedirFotos()
    }

    // MARK: Mandos

    func mandar(_ m: MandoDiapositiva) {
        switch m {
        case .notas:
            notasAbajo.toggle()
            if let a = archivo {
                let i = estado.actual
                cola.async { _ = a.notas(i); DispatchQueue.main.async { MainActor.assumeIsolated { self.vista.setNeedsDisplay() } } }
            }
        case .transiciones:
            transiciones.toggle()
            avisar(transiciones ? TextosPase.transicionesSi : TextosPase.transicionesNo)
        case .pastilla:
            if pastillaVisible {
                pastillaHasta = 0
            } else {
                estado.pastilla = true
                ensenarPastilla()
            }
        default:
            guard estado.aplicar(m) else {
                terminar()
                return
            }
            alCambiar()
        }
        vista.setNeedsDisplay()
    }

    private func ensenarPastilla() {
        pastillaHasta = CACurrentMediaTime() + CuentasDiapositivas.segundosPastilla
        programarEsconder()
        vista.setNeedsDisplay()
    }

    private func avisar(_ t: String) {
        aviso = (t, CACurrentMediaTime() + CuentasDiapositivas.segundosAviso)
        programarEsconder()
    }

    /// Un toque en un video o un audio lo reproduce. `true` si habia uno.
    func reproducirMedio(en p: CGPoint, caja tam: CGSize) -> Bool {
        guard let a = archivo, !estado.negro, estado.total > 0 else { return false }
        let caja = CuentasDiapositivas.encajarEntera(a.tamano, en: tam)
        let k = caja.width / max(a.tamano.width, 1)
        let (px, py) = (Float((p.x - caja.minX) / k), Float((p.y - caja.minY) / k))
        guard let m = a.medios(estado.actual).first(where: {
            px >= $0.x && px <= $0.x + $0.ancho && py >= $0.y && py <= $0.y + $0.alto
        }) else { return false }
        let url = self.url
        cola.async {
            let f = a.fichero(de: m, origen: url)
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let f else { return }
                    if m.audio {
                        ReproductorVoz.compartido.alternar(f.path, url: f, titulo: f.lastPathComponent)
                    } else if let c = SalidaDocumentoIPad.presentador(desde: self.vista) {
                        let reproductor = AVPlayerViewController()
                        reproductor.player = AVPlayer(url: f)
                        c.present(reproductor, animated: true) { reproductor.player?.play() }
                    }
                }
            }
        }
        return true
    }

    // MARK: Pasar

    private func alCambiar() {
        guard estado.actual != vista_ else { return }
        let antes = vista_
        vista_ = estado.actual
        let ahora = CACurrentMediaTime()
        visibleDesde = ahora
        enCurso = nil
        if let a = archivo, !estado.negro {
            let t = a.transicion(estado.actual)
            if CuentasDiapositivas.tocaTransicion(antes: antes, ahora: estado.actual, encendidas: transiciones, ms: t?.ms),
               let t, let vieja = fotos[antes] {
                enCurso = (vieja, t.efecto, t.ms, ahora)
                visibleDesde = ahora + Double(t.ms) / 1000
                arrancarEnlace()
            }
        }
        pedirFotos()
        programarAvance()
    }

    func fotograma() -> (CGImage, [CapaDiapositiva])? {
        guard let c = enCurso else { return nil }
        let t = CuentasDiapositivas.progreso(transcurrido: CACurrentMediaTime() - c.empezo, ms: c.ms)
        if t >= 1 { return nil }
        return (c.vieja, capasDeTransicion(efecto: c.efecto, t: t))
    }

    private func arrancarEnlace() {
        guard enlace == nil else { return }
        let e = CADisplayLink(target: self, selector: #selector(fotogramaSiguiente))
        e.add(to: .main, forMode: .common)
        enlace = e
    }

    @objc private func fotogramaSiguiente() {
        vista.setNeedsDisplay()
        if let c = enCurso, CACurrentMediaTime() - c.empezo >= Double(c.ms) / 1000 { enCurso = nil }
        if enCurso == nil {
            enlace?.invalidate()
            enlace = nil
        }
    }

    private func programarAvance() {
        avance?.invalidate()
        avance = nil
        guard let a = archivo, estado.total > 0, estado.puedeSiguiente,
              let ms = a.transicion(estado.actual)?.avanceMs else { return }
        let espera = max(visibleDesde + Double(ms) / 1000 - CACurrentMediaTime(), 0)
        let i = estado.actual
        let r = Timer(timeInterval: espera, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.estado.actual == i, !self.estado.negro else { return }
                self.mandar(.siguiente)
            }
        }
        RunLoop.main.add(r, forMode: .common)
        avance = r
    }

    private func programarEsconder() {
        esconder?.invalidate()
        let ahora = CACurrentMediaTime()
        let cuando = [pastillaHasta, aviso?.hasta ?? 0].filter { $0 > ahora && $0.isFinite }.min()
        guard let cuando else { return }
        let r = Timer(timeInterval: cuando - ahora + 0.02, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.vista.setNeedsDisplay()
                self?.programarEsconder()
            }
        }
        RunLoop.main.add(r, forMode: .common)
        esconder = r
    }

    /// Pide la que se ve y las de al lado; suelta las lejanas.
    private func pedirFotos() {
        guard let a = archivo, estado.total > 0, vista.bounds.width > 0 else { return }
        let i = estado.actual
        fotos = fotos.filter { abs($0.key - i) <= 1 }
        let escala = vista.traitCollection.displayScale
        let caja = CuentasDiapositivas.encajarEntera(a.tamano, en: vista.bounds.size)
        let pixeles = CGSize(width: caja.width * escala, height: caja.height * escala)
        for j in [i, i + 1, i - 1] where j >= 0 && j < estado.total && fotos[j] == nil && !pidiendo.contains(j) {
            pidiendo.insert(j)
            cola.async { [weak self] in
                let f = a.foto(j, pixeles: pixeles)
                _ = a.notas(j)
                _ = a.transicion(j)
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        guard let self, !self.cerrada else { return }
                        self.pidiendo.remove(j)
                        if let f, abs(j - self.estado.actual) <= 1 { self.fotos[j] = f }
                        self.vista.setNeedsDisplay()
                    }
                }
            }
        }
    }

    // MARK: Salir

    func terminar() {
        guard !cerrada else { return }
        cerrada = true
        UIApplication.shared.isIdleTimerDisabled = false
        enlace?.invalidate()
        enlace = nil
        avance?.invalidate()
        esconder?.invalidate()
        alSalir?(estado.actual)
    }
}

// MARK: - SwiftUI

private struct VistaPaseRepresentada: UIViewRepresentable {
    let pase: PresentacionIPad

    func makeUIView(context: Context) -> VistaPaseIPad { pase.vista }
    func updateUIView(_ v: VistaPaseIPad, context: Context) {}
}

/// **La presentacion a pantalla completa** (para `.fullScreenCover`).
/// `alSalir` recibe la diapositiva en la que se quedo.
struct PresentarDiapositivasIPad: View {
    @StateObject private var pase: PresentacionIPad
    let alSalir: (Int) -> Void

    init(url: URL, desde: Int = 0, alSalir: @escaping (Int) -> Void) {
        _pase = StateObject(wrappedValue: PresentacionIPad(url: url, desde: desde))
        self.alSalir = alSalir
    }

    var body: some View {
        VistaPaseRepresentada(pase: pase)
            .ignoresSafeArea()
            .statusBarHidden()
            .persistentSystemOverlays(.hidden)
            .onAppear {
                pase.alSalir = alSalir
                pase.empezar()
            }
            .onDisappear { pase.terminar() }
    }
}
