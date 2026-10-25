// **Los marcadores y los marcos con su vista** de los planos y los modelos
// 3D (`pixpin-cad/marcadores.rs` de Windows, `RielVisor.swift` y
// `Marcos3D.swift` del Mac), en los mismos ficheros que Windows, el Mac y
// Android, asi que viajan con el chat:
//
// - `anot-<uid>.vistas.json` (`{"version":1,"plano":[…],"modelo":[…]}`):
//   los marcadores de un plano (centro y lo que se ve) y los de un modelo
//   (su `Estado3d` entero: camara, caja de seccion, aislado y ocultos, con
//   claves estables por `GlobalId`);
// - `anot-<uid>.marcos.json`: los marcos numerados de un modelo 3D, cada uno
//   con su recuadro en la pantalla y su vista;
// - `anot-<uid>-m<id>`: la tinta de cada marcador 3D
//   (`Nucleo.rutaDeTintaDeMarcador3d`), en la pantalla desde el centro, en
//   milesimas de su alto, y solo con la vista de su marcador.
//
// Un fichero que no es de ningun chat guarda lo suyo a su lado
// (`.pixpin-vistas`, `.pixpin-marcos`).

import Foundation
import PixPinCore
import SwiftUI
import UIKit

/// **Un marcador de un plano con su vista** (`Marcador2d` de Windows): el
/// centro y lo que se ve, en coordenadas del dibujo (Y arriba). Van
/// numerados (1, 2, 3…).
struct MarcadorDeVista: Equatable, Identifiable {
    var id: String
    var nombre: String
    var x: Double, y: Double, ancho: Double, alto: Double
}

/// Un id nuevo como los de Windows («m» y los milisegundos en base 36).
func idDeMarcador(_ fecha: Date = Date()) -> String {
    "m" + String(UInt64(fecha.timeIntervalSince1970 * 1000), radix: 36)
}

/// Los marcadores de un plano o un modelo en el `.vistas.json` de Windows.
/// Lo que no se entiende se conserva tal cual.
@MainActor
final class MarcadoresDeVista: ObservableObject {
    @Published private(set) var lista: [MarcadorDeVista] = []
    let fichero: URL
    private var resto: [String: Any] = ["version": 1]

    init(documento: URL, nucleo: Nucleo?) {
        if let n = nucleo, let r = n.rutaAnotadaDelAdjunto(documento: documento.path, terminacion: ".vistas.json") {
            fichero = URL(fileURLWithPath: r)
        } else {
            fichero = documento.appendingPathExtension("pixpin-vistas")
        }
        guard let d = try? Data(contentsOf: fichero) else { return }
        leer(d)
    }

    /// Para las pruebas: en un fichero dado.
    init(fichero: URL) {
        self.fichero = fichero
        if let d = try? Data(contentsOf: fichero) { leer(d) }
    }

    private func leer(_ d: Data) {
        guard let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return }
        resto = o
        lista = (o["plano"] as? [[String: Any]] ?? []).compactMap { m in
            guard let id = m["id"] as? String else { return nil }
            func n(_ k: String) -> Double { (m[k] as? NSNumber)?.doubleValue ?? 0 }
            return MarcadorDeVista(id: id, nombre: m["nombre"] as? String ?? "", x: n("x"), y: n("y"),
                                   ancho: n("ancho"), alto: n("alto"))
        }
    }

    func poner(x: Double, y: Double, ancho: Double, alto: Double) {
        lista.append(MarcadorDeVista(id: idDeMarcador(), nombre: "", x: x, y: y, ancho: ancho, alto: alto))
        guardar()
    }

    func quitar(_ m: MarcadorDeVista) {
        lista.removeAll { $0.id == m.id }
        guardar()
    }

    // MARK: Modelos 3D (`Marcador3d` con su `Estado3d`)

    /// Los de un modelo: id y su `Estado3d` tal cual.
    var del3d: [(id: String, estado: [String: Any])] {
        (resto["modelo"] as? [[String: Any]] ?? []).compactMap { m in
            guard let id = m["id"] as? String, let e = m["estado"] as? [String: Any] else { return nil }
            return (id, e)
        }
    }

    @discardableResult
    func poner3d(estado: [String: Any]) -> String {
        let id = idDeMarcador()
        var l = resto["modelo"] as? [[String: Any]] ?? []
        l.append(["id": id, "nombre": "", "estado": estado])
        resto["modelo"] = l
        objectWillChange.send()
        guardar()
        return id
    }

    func quitar3d(_ id: String) {
        resto["modelo"] = (resto["modelo"] as? [[String: Any]] ?? []).filter { ($0["id"] as? String) != id }
        objectWillChange.send()
        guardar()
    }

    /// Vaciar escribe la lista vacia: un fichero borrado volveria al sincronizar.
    private func guardar() {
        var o = resto
        o["version"] = 1
        o["plano"] = lista.map { ["id": $0.id, "nombre": $0.nombre, "x": $0.x, "y": $0.y, "ancho": $0.ancho, "alto": $0.alto] }
        if o["modelo"] == nil { o["modelo"] = [Any]() }
        resto = o
        if let d = try? JSONSerialization.data(withJSONObject: o, options: [.prettyPrinted, .sortedKeys]) {
            try? d.write(to: fichero, options: .atomic)
        }
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }
}

// MARK: - El estado de la vista 3D (`Estado3d` de Windows)

/// La camara de un `Estado3d` (`objetivo`, `rumbo`, `altura`, `dist`).
func camaraDeEstado3d(_ e: [String: Any]) -> Camara3d? {
    guard let o = (e["objetivo"] as? [NSNumber])?.map(\.doubleValue), o.count == 3 else { return nil }
    func n(_ k: String) -> Double { (e[k] as? NSNumber)?.doubleValue ?? 0 }
    return Camara3d(objetivo: Punto3d(x: o[0], y: o[1], z: o[2]), rumbo: n("rumbo"), altura: n("altura"), dist: n("dist"))
}

/// Si la camara `a` es la `b` de un marcador (Windows: ~2e-3 relativo).
func mismaCamara3d(_ a: Camara3d, _ b: Camara3d) -> Bool {
    let d = max(abs(a.dist), 1e-9)
    let o = hypot(hypot(a.objetivo.x - b.objetivo.x, a.objetivo.y - b.objetivo.y), a.objetivo.z - b.objetivo.z)
    return o / d < 2e-3 && abs(a.dist - b.dist) / d < 2e-3 && abs(a.rumbo - b.rumbo) < 2e-3 && abs(a.altura - b.altura) < 2e-3
}

// MARK: - Los marcos numerados de un modelo 3D

/// **Los marcos numerados de un modelo 3D** (`Marco3d` de Windows): cada
/// uno, su recuadro en la pantalla (desde el centro de la vista, en
/// unidades de su alto, Y hacia abajo) y la vista con que se puso. Solo se
/// ve con su vista.
@MainActor
final class Marcos3D: ObservableObject {
    struct Marco: Identifiable {
        var id: String
        var caja: [Double]
        var camara: Camara3d
        /// El `Estado3d` entero (corte, ocultos…), tal cual.
        var estado: [String: Any]
    }

    @Published private(set) var lista: [Marco] = []
    /// Arrastrar pone un marco (el modo de los marcos).
    @Published var activo = false
    private let fichero: URL
    private var resto: [String: Any] = ["version": 1]

    init(documento: URL, nucleo: Nucleo?) {
        if let n = nucleo, let r = n.rutaAnotadaDelAdjunto(documento: documento.path, terminacion: ".marcos.json") {
            fichero = URL(fileURLWithPath: r)
        } else {
            fichero = documento.appendingPathExtension("pixpin-marcos")
        }
        guard let d = try? Data(contentsOf: fichero),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return }
        resto = o
        lista = (o["marcos"] as? [[String: Any]] ?? []).compactMap { m in
            guard let id = m["id"] as? String, let c = (m["caja"] as? [NSNumber])?.map(\.doubleValue), c.count == 4,
                  let e = m["estado"] as? [String: Any], let camara = camaraDeEstado3d(e) else { return nil }
            return Marco(id: id, caja: c, camara: camara, estado: e)
        }
    }

    func poner(caja: [Double], camara: Camara3d, estado: [String: Any]) {
        lista.append(Marco(id: idDeMarcador(), caja: caja, camara: camara, estado: estado))
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

    /// El recuadro de un marco en una vista de `tamano` (puntos o pixeles).
    static func rect(_ caja: [Double], en tamano: CGSize) -> CGRect {
        guard caja.count == 4 else { return .zero }
        let w = Double(tamano.width), h = Double(tamano.height)
        return CGRect(x: caja[0] * h + w / 2, y: caja[1] * h + h / 2,
                      width: (caja[2] - caja[0]) * h, height: (caja[3] - caja[1]) * h)
    }

    /// La caja de un recuadro de la vista (al reves que `rect`).
    static func caja(_ a: CGPoint, _ b: CGPoint, en tamano: CGSize) -> [Double] {
        let w = Double(tamano.width), h = max(Double(tamano.height), 1)
        return [(Double(min(a.x, b.x)) - w / 2) / h, (Double(min(a.y, b.y)) - h / 2) / h,
                (Double(max(a.x, b.x)) - w / 2) / h, (Double(max(a.y, b.y)) - h / 2) / h]
    }

    private func guardar() {
        var o = resto
        o["version"] = 1
        o["marcos"] = lista.map { m -> [String: Any] in ["id": m.id, "caja": m.caja, "estado": m.estado] }
        resto = o
        if let d = try? JSONSerialization.data(withJSONObject: o, options: [.prettyPrinted, .sortedKeys]) {
            try? d.write(to: fichero, options: .atomic)
        }
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }
}

// MARK: - El croquis del chat con sus fotos

/// **Los croquis 3D del movil abiertos desde un chat**: quien abre el visor
/// (el chat) apunta aqui de que chat y con que referencia es la ruta, y el
/// visor 3D lo lee entonces con `Nucleo.abrirCroquis` (con sus fotos) en vez
/// de como un modelo suelto.
@MainActor
public enum CroquisDelChatIPad {
    private static var apuntados: [String: (chat: String, referencia: String)] = [:]

    /// Apunta que `ruta` es el croquis `referencia` del chat `chat`.
    public static func apuntar(ruta: String, chat: String, referencia: String) {
        apuntados[ruta] = (chat, referencia)
    }

    /// De que chat es `ruta`, si se apunto.
    static func de(_ ruta: String) -> (chat: String, referencia: String)? { apuntados[ruta] }
}

// MARK: - La tinta de un marcador 3D

/// **La tinta de un marcador 3D, encima del modelo**: se ve solo con la
/// vista de su marcador y, anotando, recibe el lapiz (con todas las
/// herramientas del lienzo). Sus coordenadas son las de Windows: desde el
/// centro de la vista, en milesimas de su alto y la Y hacia abajo.
@MainActor
final class CapaTintaMarcador3DIPad: UIView, UIKeyInput, UIPencilInteractionDelegate {
    /// El marcador cuya tinta se ve.
    var marcador: String?
    /// Lo dibujado, para verlo (sin anotar).
    var leido: LienzoLeido? { didSet { setNeedsDisplay() } }
    /// Anotando: el modelo del lienzo.
    var modelo: ModeloEditorLienzo? {
        didSet {
            isUserInteractionEnabled = modelo != nil
            clave = []
            ajustarTeclado()
            setNeedsDisplay()
        }
    }

    private var editor: EditorLienzo? { modelo?.editor }
    private var lectura: LienzoLeido?
    private var clave: [Double] = []
    private weak var trazando: UITouch?
    private var vistoLapiz = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        isUserInteractionEnabled = false
        isMultipleTouchEnabled = true
        contentMode = .redraw
        let lapiz = UIPencilInteraction()
        lapiz.delegate = self
        addInteraction(lapiz)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    /// El punto del mundo arriba a la izquierda y los puntos por unidad.
    var origen: CGPoint { CGPoint(x: -500 * bounds.width / max(bounds.height, 1), y: -500) }
    var zoom: CGFloat { max(bounds.height, 1) / 1000 }

    override func layoutSubviews() {
        super.layoutSubviews()
        setNeedsDisplay()
    }

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        let o = origen, z = zoom
        let lienzo: LienzoLeido?
        if let editor {
            let k: [Double] = [Double(editor.version()), Double(bounds.width), Double(bounds.height)]
            if k != clave {
                clave = k
                lectura = try? leerLienzo(editor.planoDeVista(x: Float(o.x), y: Float(o.y), zoom: Float(z),
                                                              anchoPx: Float(bounds.width), altoPx: Float(bounds.height)))
            }
            lienzo = lectura
        } else {
            lienzo = leido
        }
        guard let lienzo else { return }
        ctx.saveGState()
        ctx.scaleBy(x: z, y: z)
        ctx.translateBy(x: -o.x, y: -o.y)
        let imagen: (UInt64) -> CGImage? = modelo?.imagenes(escala: z * contentScaleFactor) ?? { _ in nil }
        PintorLienzo(lienzo: lienzo, imagen: imagen)
            .pintar(en: ctx, limites: CGRect(x: o.x, y: o.y, width: bounds.width / z, height: bounds.height / z))
        ctx.restoreGState()
    }

    // MARK: El lapiz

    private func aMundo(_ p: CGPoint) -> CGPoint {
        CGPoint(x: origen.x + p.x / zoom, y: origen.y + p.y / zoom)
    }

    private func mandar(_ fase: FasePuntero, _ t: UITouch, _ evento: UIEvent?) {
        guard let editor, let modelo else { return }
        let toques = (fase == .mover ? evento?.coalescedTouches(for: t) : nil) ?? [t]
        for c in toques {
            let p = aMundo(c.type == .pencil ? c.preciseLocation(in: self) : c.location(in: self))
            let presion: Float? = c.type == .pencil ? Float(min(1, max(0.02, c.force / 2))) : nil
            _ = editor.puntero(fase: fase, x: Float(p.x), y: Float(p.y), presion: presion, modificadores: 0, zoom: Float(zoom))
        }
        if fase != .mover {
            modelo.refrescarBarra()
            modelo.revisarCambios()
            ajustarTeclado()
        }
        if fase == .soltar { _ = editor.tomarPeticion() }
        setNeedsDisplay()
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard modelo != nil else { return super.touchesBegan(touches, with: event) }
        for t in touches {
            if t.type == .pencil { vistoLapiz = true }
            let puede = t.type == .pencil || (!vistoLapiz && t.type == .direct && (event?.allTouches?.count ?? 1) == 1)
            if trazando == nil, puede {
                trazando = t
                if modelo?.desplegado != nil { modelo?.desplegado = nil }
                mandar(.pulsar, t, event)
            } else if trazando != nil, t.type != .pencil, trazando?.type != .pencil {
                cancelar()
            }
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = trazando, touches.contains(t) else { return super.touchesMoved(touches, with: event) }
        mandar(.mover, t, event)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = trazando, touches.contains(t) else { return super.touchesEnded(touches, with: event) }
        mandar(.soltar, t, event)
        trazando = nil
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = trazando, touches.contains(t) else { return super.touchesCancelled(touches, with: event) }
        cancelar()
    }

    private func cancelar() {
        trazando = nil
        _ = editor?.puntero(fase: .cancelar, x: 0, y: 0, presion: nil, modificadores: 0, zoom: Float(zoom))
        setNeedsDisplay()
    }

    func pencilInteraction(_ interaction: UIPencilInteraction, didReceiveTap tap: UIPencilInteraction.Tap) {
        guard let modelo else { return }
        modelo.alternarGoma()
        setNeedsDisplay()
    }

    // MARK: El teclado (el texto del lienzo)

    private var escribiendo: Bool { editor?.escribiendo() ?? false }

    override var canBecomeFirstResponder: Bool { escribiendo }

    func ajustarTeclado() {
        if escribiendo, !isFirstResponder {
            becomeFirstResponder()
        } else if !escribiendo, isFirstResponder {
            resignFirstResponder()
        }
    }

    var hasText: Bool { true }

    func insertText(_ text: String) {
        guard let editor, editor.escribiendo() else { return }
        if text == "\n" ? editor.tecla(nombre: "entrar") : editor.teclear(texto: text) {
            modelo?.revisarCambios()
            setNeedsDisplay()
        }
        ajustarTeclado()
    }

    func deleteBackward() {
        guard let editor, editor.tecla(nombre: "retroceso") else { return }
        modelo?.revisarCambios()
        setNeedsDisplay()
    }

    var autocorrectionType: UITextAutocorrectionType = .no
    var spellCheckingType: UITextSpellCheckingType = .no
    var smartQuotesType: UITextSmartQuotesType = .no
    var smartDashesType: UITextSmartDashesType = .no
}
