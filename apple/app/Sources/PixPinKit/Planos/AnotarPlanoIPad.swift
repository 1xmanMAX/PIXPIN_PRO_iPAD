// **Anotar el plano a mano con el Apple Pencil** (`plano_cad::anotar` de
// Windows, `AnotarPlano.swift` del Mac), en sencillo: el editor del lienzo
// de Rust (`Plano.anotar`) recibe los trazos del lapiz en las coordenadas de
// la capa del plano y lo dibujado se pinta encima con Core Graphics.
//
// - La capa es la de siempre (`anot-<uid>` del mensaje o la de la cache):
//   lo anotado aqui se ve en Windows, el Mac y Android, y al reves.
// - Las coordenadas: `x = cero_x + x_capa * u`, `y = cero_y - y_capa * u`
//   (`EncajePlano`; la y de la capa baja, como en el lienzo).
// - Solo el lapiz pinta en cuanto se ha visto un Pencil (la palma no pinta);
//   sin Pencil (el simulador, un iPad sin lapiz) pinta un dedo.
// - Al terminar, lo escrito se guarda y la `.hoja` va detras de la capa.
//
// Pendiente: la barra entera del lienzo (formas, texto, colores...) cuando
// el lienzo del iPad tenga su vista con papel transparente y camara de fuera.

import CoreGraphics
import Foundation
import PixPinCore
import UIKit

/// Las herramientas del lapiz sobre el plano (los nombres del motor del lienzo).
enum HerramientaTintaPlano: String, CaseIterable, Identifiable {
    case lapiz, resaltador, borrador

    var id: String { rawValue }

    var titulo: String {
        switch self {
        case .lapiz: "Lápiz"
        case .resaltador: "Resaltador"
        case .borrador: "Goma"
        }
    }

    var simbolo: String {
        switch self {
        case .lapiz: "pencil.tip"
        case .resaltador: "highlighter"
        case .borrador: "eraser"
        }
    }
}

/// El editor del lienzo sobre la capa del plano mientras se anota.
@MainActor
final class AnotadorPlanoIPad {
    let editor: EditorLienzo
    /// Como cae la capa en el dibujo (el de cuando se empezo: con el se
    /// escribe la `.hoja`).
    let encaje: EncajePlano
    private let plano: Plano
    /// Lo dibujado, leido de nuevo solo cuando cambia la version o la vista.
    private(set) var leido: LienzoLeido?
    private var clave: [Double] = []

    init(plano: Plano) throws {
        self.plano = plano
        editor = try plano.anotar()
        encaje = plano.encaje()
        try editor.herramienta(nombre: HerramientaTintaPlano.lapiz.rawValue)
        // Trazo limpio: sobre un plano se senala, no se boceta.
        _ = editor.cambiarEstilo(cambio: .rugosidad(valor: 0))
        releer()
    }

    func herramienta(_ h: HerramientaTintaPlano) {
        try? editor.herramienta(nombre: h.rawValue)
    }

    /// Un punto del lapiz, ya en coordenadas de la capa; `zoom` es lo que
    /// mide en puntos de la pantalla una unidad de la capa.
    func puntero(_ fase: FasePuntero, x: Double, y: Double, presion: Float?, zoom: Double) {
        _ = editor.puntero(fase: fase, x: Float(x), y: Float(y), presion: presion, modificadores: 0, zoom: Float(zoom))
    }

    /// Lo nuevo, a la escala de lo que se ve (como Windows): un trazo fino
    /// se ve fino este el plano lejos o cerca.
    func escalaDeLoNuevo(zoom: Double) {
        guard zoom > 0, zoom.isFinite else { return }
        editor.escalaDeLoNuevo(k: Float(1 / zoom))
    }

    @discardableResult
    func deshacer() -> Bool {
        let hecho = editor.deshacer()
        if hecho { releer() }
        return hecho
    }

    var puedeDeshacer: Bool { editor.puedeDeshacer() }

    /// Vuelve a leer lo dibujado si cambio. Con `origen` (el punto de la
    /// capa arriba a la izquierda de la vista), `zoom` (puntos por unidad de
    /// la capa) y `tamano` (la vista en puntos), lo que se ve con lo de
    /// encima (lo elegido, sus tiradores), como la vista del lienzo; sin el,
    /// todo el dibujo.
    @discardableResult
    func releer(origen: CGPoint? = nil, zoom: Double = 1, tamano: CGSize = .zero) -> Bool {
        let v = Double(editor.version())
        let k: [Double] = if let origen {
            [v, Double(origen.x), Double(origen.y), zoom, Double(tamano.width), Double(tamano.height)]
        } else {
            [v]
        }
        guard k != clave else { return false }
        clave = k
        if let origen, zoom > 0, tamano.width > 0, tamano.height > 0 {
            leido = try? leerLienzo(editor.planoDeVista(x: Float(origen.x), y: Float(origen.y), zoom: Float(zoom),
                                                        anchoPx: Float(tamano.width), altoPx: Float(tamano.height)))
        } else {
            leido = try? leerLienzo(editor.plano())
        }
        return true
    }

    /// **Termina**: lo que se escribia se queda escrito y la capa se guarda
    /// (con su `.hoja`) fuera del hilo principal.
    func terminar() async {
        let editor = self.editor, plano = self.plano, encaje = self.encaje
        await Task.detached(priority: .userInitiated) {
            _ = editor.terminarTexto()
            if (try? editor.guardar()) == true {
                _ = try? plano.ponerHoja(encaje: encaje)
                await MainActor.run { NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil) }
            }
        }.value
    }
}

// MARK: - Pintar lo anotado

/// **Pinta la capa del plano** (un `LienzoLeido`) en un `CGContext` con la y
/// hacia abajo, ya puesto en las coordenadas de la capa. Solo lo que se
/// anota sobre un plano: rellenos, tinta, rayas, textos y rotulos de cotas;
/// las imagenes y el velo no (pendiente: el pintor comun del lienzo).
enum PintorTintaPlano {
    static func pintar(_ l: LienzoLeido, en ctx: CGContext) {
        for figura in l.figuras {
            switch figura {
            case let .relleno(path, color):
                ctx.addPath(path)
                ctx.setFillColor(color.cg)
                ctx.fillPath(using: .evenOdd)
            case let .tinta(path, color):
                ctx.addPath(path)
                ctx.setFillColor(color.cg)
                ctx.fillPath(using: .winding)
            case let .linea(path, color, grosor, estilo):
                ctx.saveGState()
                ctx.addPath(path)
                ctx.setStrokeColor(color.cg)
                ctx.setLineWidth(grosor)
                ctx.setLineCap(.round)
                ctx.setLineJoin(.round)
                switch estilo {
                case 1: ctx.setLineDash(phase: 0, lengths: [grosor * 4, grosor * 3])
                case 2: ctx.setLineDash(phase: 0, lengths: [0.01, grosor * 2.5])
                case 3: ctx.setLineDash(phase: 0, lengths: [grosor * 5, grosor * 2.5, 0.01, grosor * 2.5])
                default: break
                }
                ctx.strokePath()
                ctx.restoreGState()
            case let .texto(texto, origen, tam, anchoMax, _, negrita, cursiva, color):
                var f = UIFont.systemFont(ofSize: tam, weight: negrita ? .bold : .regular)
                if cursiva, let d = f.fontDescriptor.withSymbolicTraits(.traitItalic) { f = UIFont(descriptor: d, size: tam) }
                let atr: [NSAttributedString.Key: Any] = [.font: f, .foregroundColor: UIColor(cgColor: color.cg)]
                UIGraphicsPushContext(ctx)
                let ancho = anchoMax > 0 ? anchoMax : .greatestFiniteMagnitude
                (texto as NSString).draw(with: CGRect(x: origen.x, y: origen.y, width: ancho, height: .greatestFiniteMagnitude),
                                         options: [.usesLineFragmentOrigin], attributes: atr, context: nil)
                UIGraphicsPopContext()
            case let .rotulo(texto, origen, tam, _, color, halo, grosorHalo, centro, angulo):
                ctx.saveGState()
                ctx.translateBy(x: centro.x, y: centro.y)
                ctx.rotate(by: angulo)
                ctx.translateBy(x: -centro.x, y: -centro.y)
                let f = UIFont.monospacedDigitSystemFont(ofSize: tam, weight: .semibold)
                UIGraphicsPushContext(ctx)
                if grosorHalo > 0 {
                    let atrHalo: [NSAttributedString.Key: Any] = [.font: f, .foregroundColor: UIColor(cgColor: halo.cg),
                                                                  .strokeColor: UIColor(cgColor: halo.cg),
                                                                  .strokeWidth: -Double(grosorHalo / max(tam, 1) * 100)]
                    (texto as NSString).draw(at: origen, withAttributes: atrHalo)
                }
                (texto as NSString).draw(at: origen, withAttributes: [.font: f, .foregroundColor: UIColor(cgColor: color.cg)])
                UIGraphicsPopContext()
                ctx.restoreGState()
            case .velo, .imagen:
                continue
            }
        }
    }

    /// La transformacion de la capa a la vista (puntos, y hacia abajo) con
    /// la camara del plano: `x = cero_x + x_capa * u`, `y = cero_y - y_capa * u`.
    /// `s`: unidades del dibujo por punto de la pantalla.
    static func transformacion(camara c: CamaraPlano, s: Double, encaje e: EncajePlano, tamano: CGSize) -> CGAffineTransform {
        let a = e.u / s
        return CGAffineTransform(a: a, b: 0, c: 0, d: a,
                                 tx: (e.ceroX - c.centroX) / s + Double(tamano.width) / 2,
                                 ty: Double(tamano.height) / 2 - (e.ceroY - c.centroY) / s)
    }
}
