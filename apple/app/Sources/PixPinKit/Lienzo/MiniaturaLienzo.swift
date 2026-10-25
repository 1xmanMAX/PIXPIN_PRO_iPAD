// La miniatura de un lienzo del chat (la burbuja `DIBUJO`, `referencia` =
// su fichero), como `BurbujaDibujo` del Mac: el dibujo entero encajado en la
// burbuja, con sus fotos. Un toque lo abre en el editor.
//
// Rendimiento (§4bis del plan):
// - leer el `.excalidraw` y pintar van FUERA del hilo principal: leer con
//   `Almacen.dibujo` (en tres cruces del puente) y pintar en un mapa de bits
//   con `MapaLienzo` (`Task.detached`); al hilo principal solo llega la
//   imagen hecha;
// - las imagenes hechas van a una `NSCache` con tope en BYTES; dos burbujas
//   del mismo dibujo esperan la misma lectura;
// - se rehace al guardarse el dibujo (`pixpinLienzoGuardado`) o al traer
//   cambios de otro aparato (`pixpinTrajoCambios`).

import CoreGraphics
import Foundation
import PixPinCore
import SwiftUI

/// La miniatura de un lienzo del chat (burbuja `DIBUJO`, `referencia` = su fichero).
public struct MiniaturaLienzo: View {
    let almacen: Almacen
    let chat: String
    let mensaje: MensajeChat

    @State private var imagen: CGImage?
    @State private var falta = false
    /// Sube cuando el dibujo cambia, para volver a pintarlo.
    @State private var recargas = 0
    @Environment(\.displayScale) private var escalaPantalla

    /// El tamano de la burbuja, en puntos (el del Mac: 220 x 154).
    static let tamano = CGSize(width: 220, height: 154)

    public init(almacen: Almacen, chat: String, mensaje: MensajeChat) {
        self.almacen = almacen; self.chat = chat; self.mensaje = mensaje
    }

    public var body: some View {
        Group {
            if let imagen {
                Image(decorative: imagen, scale: max(1, escalaPantalla))
                    .resizable()
                    .interpolation(.high)
            } else if falta {
                RoundedRectangle(cornerRadius: 8).fill(.quaternary)
                    .overlay {
                        Label(mensaje.nombre.isEmpty ? "Lienzo" : mensaje.nombre, systemImage: "scribble.variable")
                            .font(.callout).foregroundStyle(.secondary).padding(8)
                    }
            } else {
                RoundedRectangle(cornerRadius: 8).fill(.quaternary).overlay { ProgressView() }
            }
        }
        .frame(width: MiniaturaLienzo.tamano.width, height: MiniaturaLienzo.tamano.height)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(.separator.opacity(0.4)) }
        .contentShape(Rectangle())
        .onTapGesture(perform: editar)
        .accessibilityLabel(mensaje.nombre.isEmpty ? "Lienzo" : mensaje.nombre)
        .accessibilityAddTraits(.isButton)
        .onReceive(NotificationCenter.default.publisher(for: .pixpinLienzoGuardado)) { aviso in
            guard let referencia = aviso.object as? String, referencia == mensaje.referencia else { return }
            MiniaturasLienzo.compartidas.olvidar(chat: chat, referencia: referencia)
            recargas += 1
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            guard let referencia = mensaje.referencia else { return }
            MiniaturasLienzo.compartidas.olvidar(chat: chat, referencia: referencia)
            recargas += 1
        }
        .task(id: "\(mensaje.id)#\(recargas)#\(escalaPantalla)") {
            LienzoDelIPad.activar()
            guard let referencia = mensaje.referencia, !referencia.isEmpty else {
                falta = true
                return
            }
            let px = CGSize(width: MiniaturaLienzo.tamano.width * max(1, escalaPantalla),
                            height: MiniaturaLienzo.tamano.height * max(1, escalaPantalla))
            if let hecha = await MiniaturasLienzo.compartidas.imagen(chat: chat, referencia: referencia,
                                                                    pixeles: px, almacen: almacen) {
                imagen = hecha
                falta = false
            } else if imagen == nil {
                falta = true
            }
        }
    }

    private func editar() {
        guard let referencia = mensaje.referencia, !referencia.isEmpty else { return }
        Navegacion.compartida.abrir(.lienzo(chat: chat, mensaje: mensaje, foto: nil))
    }
}

/// **Las miniaturas de los lienzos ya pintadas**, para toda la app.
@MainActor
final class MiniaturasLienzo {
    static let compartidas = MiniaturasLienzo()

    private final class Caja {
        let imagen: CGImage
        init(_ imagen: CGImage) { self.imagen = imagen }
    }

    private let cache: NSCache<NSString, Caja> = {
        let c = NSCache<NSString, Caja>()
        c.totalCostLimit = 32 << 20
        c.name = "PixPin.miniaturasLienzo"
        return c
    }()
    private var enCurso: [String: Task<CGImage?, Never>] = [:]
    /// Sube al olvidar: lo que estaba a medias de antes no se guarda.
    private var turno: [String: Int] = [:]

    private static func clave(_ chat: String, _ referencia: String) -> String { chat + "\u{0}" + referencia }

    /// Se dejara de ensenar lo de antes de `referencia` (se guardo o llego
    /// de otro aparato).
    func olvidar(chat: String, referencia: String) {
        let k = MiniaturasLienzo.clave(chat, referencia)
        cache.removeObject(forKey: k as NSString)
        enCurso[k] = nil
        turno[k, default: 0] += 1
    }

    func vaciar() {
        cache.removeAllObjects()
    }

    /// La miniatura del dibujo `referencia` de `chat` a `pixeles`, hecha o
    /// por hacer (leer y pintar, fuera del hilo principal). `nil` si el
    /// dibujo no se puede leer aqui.
    func imagen(chat: String, referencia: String, pixeles: CGSize, almacen: Almacen) async -> CGImage? {
        let k = MiniaturasLienzo.clave(chat, referencia)
        let w = max(1, Int(pixeles.width.rounded())), h = max(1, Int(pixeles.height.rounded()))
        if let c = cache.object(forKey: k as NSString), c.imagen.width == w, c.imagen.height == h {
            return c.imagen
        }
        if let t = enCurso[k] { return await t.value }
        let mio = turno[k, default: 0]
        let t = Task<CGImage?, Never> { @MainActor in
            guard let dibujo = await almacen.dibujo(referencia, en: chat) else { return nil }
            // Las fotos de dentro, al tamano de la burbuja (a la escala de
            // la pantalla): no hace falta mas para verlas ahi.
            let lado = max(w, h)
            var fotos: [UInt64: CGImage] = [:]
            for (id, url) in dibujo.fotos {
                if let img = await Miniaturas.compartidas.cargar(url, lado: lado) { fotos[id] = img }
            }
            let lienzo = dibujo.lienzo
            let listas = fotos
            let margen = CGFloat(w) / 220 * 8
            return await Task.detached(priority: .utility) {
                MapaLienzo.imagen(lienzo, ancho: w, alto: h, margen: margen) { listas[$0] }
            }.value
        }
        enCurso[k] = t
        let hecha = await t.value
        if turno[k, default: 0] == mio {
            enCurso[k] = nil
            if let hecha { cache.setObject(Caja(hecha), forKey: k as NSString, cost: hecha.bytesPerRow * hecha.height) }
        }
        return hecha
    }
}
