// **La foto anotada en su burbuja** (`foto_anotada.rs` de Windows y
// `VistaFotoAnotada` del Mac): una foto del chat a la que se le dibujo
// encima (en el movil, en Windows o aqui, en su lienzo propio) se ensena con
// lo dibujado, recortada a la foto (`soloLaFoto`).
//
// El lienzo de la foto lo dice el nucleo (`Nucleo.fotoAnotada`, `nil` si no
// ha llegado o solo lleva la foto). Leerlo y pintarlo va fuera del hilo
// principal y la imagen hecha se guarda en una cache con tope en bytes; se
// rehace al guardarse el lienzo (`pixpinLienzoGuardado`) o al traer cambios
// de otro aparato (`pixpinTrajoCambios`).

import CoreGraphics
import Foundation
import PixPinCore
import SwiftUI

/// La caja que se ensena de una foto anotada: la foto sola (`soloLaFoto`,
/// lo normal) o la foto con todo lo dibujado alrededor.
func cajaDeFotoAnotada(foto: CGRect?, todo: CGRect?, soloLaFoto: Bool) -> CGRect? {
    guard let foto else { return todo }
    guard !soloLaFoto, let todo else { return foto }
    return foto.union(todo)
}

/// El tamano en que se pinta una caja con su forma, con `lado` de lado mayor
/// a lo ancho (como una foto normal): ancha, a lo ancho; alta, hasta 1,4
/// veces y mas estrecha.
func tamanoDeFotoAnotada(_ caja: CGSize, lado: CGFloat) -> CGSize {
    let alto = caja.height / max(caja.width, 1)
    return alto > 1.4 ? CGSize(width: lado * 1.4 / alto, height: lado * 1.4) : CGSize(width: lado, height: lado * alto)
}

/// **Las fotos anotadas ya pintadas**, para toda la app.
@MainActor
final class FotosAnotadas {
    static let compartidas = FotosAnotadas()

    private final class Caja {
        let imagen: CGImage?
        init(_ imagen: CGImage?) { self.imagen = imagen }
    }

    private let cache: NSCache<NSString, Caja> = {
        let c = NSCache<NSString, Caja>()
        c.totalCostLimit = 24 << 20
        c.name = "PixPin.fotosAnotadas"
        return c
    }()
    private var enCurso: [String: Task<CGImage?, Never>] = [:]
    private var turno: [String: Int] = [:]
    /// Si la foto de cada lienzo (chat y lienzo) resulto anotada, a
    /// cualquier tamano (para arrastrar la burbuja con lo dibujado).
    private var anotadas: [String: Bool] = [:]

    /// Si la foto de `m` esta anotada; `nil` si aun no se sabe.
    func estaAnotada(_ m: MensajeChat, chat: String) -> Bool? {
        anotadas[chat + "\u{0}" + lienzoDeLaFoto(m)]
    }

    private static func clave(_ chat: String, _ lienzo: String, _ lado: Int, _ soloLaFoto: Bool) -> String {
        chat + "\u{0}" + lienzo + "\u{0}\(lado)" + (soloLaFoto ? "" : "+")
    }

    /// Olvida lo pintado del lienzo `lienzo` (se guardo, o llego de fuera).
    func olvidar(chat: String, lienzo: String) {
        for (k, _) in enCurso where k.hasPrefix(chat + "\u{0}" + lienzo + "\u{0}") { enCurso[k] = nil }
        for k in turno.keys where k.hasPrefix(chat + "\u{0}" + lienzo + "\u{0}") { turno[k, default: 0] += 1 }
        anotadas[chat + "\u{0}" + lienzo] = nil
        // La cache no se recorre: se olvida todo (son pocas y se rehacen).
        cache.removeAllObjects()
    }

    func olvidarTodo() {
        anotadas.removeAll()
        cache.removeAllObjects()
        enCurso.removeAll()
        for k in turno.keys { turno[k, default: 0] += 1 }
    }

    /// Lo hecho ya, si lo esta (`nil` por dentro: no tiene anotacion).
    func hecha(_ m: MensajeChat, chat: String, lado: Int) -> CGImage?? {
        guard let c = cache.object(forKey: Self.clave(chat, lienzoDeLaFoto(m), lado, m.soloLaFoto) as NSString)
        else { return nil }
        return .some(c.imagen)
    }

    /// **La foto con lo dibujado** con su lado mayor de `lado` pixeles, o
    /// `nil` si no tiene nada encima (entonces se ensena la foto de siempre).
    func imagen(_ m: MensajeChat, chat: String, almacen: Almacen, lado: Int) async -> CGImage? {
        let lienzo = lienzoDeLaFoto(m)
        let solo = m.soloLaFoto
        let k = Self.clave(chat, lienzo, lado, solo)
        if let c = cache.object(forKey: k as NSString) { return c.imagen }
        if let t = enCurso[k] { return await t.value }
        let mio = turno[k, default: 0]
        let t = Task<CGImage?, Never> { @MainActor in
            await FotosAnotadas.pintar(chat: chat, lienzo: lienzo, almacen: almacen, lado: lado, soloLaFoto: solo)
        }
        enCurso[k] = t
        let hecha = await t.value
        if turno[k, default: 0] == mio {
            enCurso[k] = nil
            anotadas[chat + "\u{0}" + lienzo] = hecha != nil
            cache.setObject(Caja(hecha), forKey: k as NSString, cost: hecha.map { $0.bytesPerRow * $0.height } ?? 64)
        }
        return hecha
    }

    /// Lee el lienzo de la foto y lo pinta recortado a la foto.
    private static func pintar(chat: String, lienzo id: String, almacen: Almacen, lado: Int,
                               soloLaFoto: Bool) async -> CGImage? {
        let nucleo = almacen.nucleo
        let anotada = await Task.detached(priority: .utility) {
            try? nucleo.fotoAnotada(chat: chat, lienzo: id)
        }.value ?? nil
        guard let anotada, let dibujo = await almacen.dibujo(anotada.lienzo, en: chat) else { return nil }
        var lienzo = dibujo.lienzo
        let foto = anotada.foto.map {
            CGRect(x: CGFloat($0.x0), y: CGFloat($0.y0), width: CGFloat($0.x1 - $0.x0), height: CGFloat($0.y1 - $0.y0))
        }
        lienzo.caja = cajaDeFotoAnotada(foto: foto, todo: lienzo.caja, soloLaFoto: soloLaFoto)
        guard let caja = lienzo.caja, caja.width > 0, caja.height > 0 else { return nil }
        let tam = tamanoDeFotoAnotada(caja.size, lado: CGFloat(lado))
        let w = max(1, Int(tam.width.rounded())), h = max(1, Int(tam.height.rounded()))
        // Las fotos de dentro, al tamano de la burbuja: no hace falta mas.
        var fotos: [UInt64: CGImage] = [:]
        for (i, url) in dibujo.fotos {
            if let img = await Miniaturas.compartidas.cargar(url, lado: max(w, h) * 2) { fotos[i] = img }
        }
        let listas = fotos
        let leido = lienzo
        return await Task.detached(priority: .utility) {
            MapaLienzo.imagen(leido, ancho: w, alto: h, margen: 0) { listas[$0] }
        }.value
    }
}

/// **La foto de la burbuja con lo dibujado encima**, si lo tiene; si no,
/// `sinAnotar` (la miniatura de siempre).
struct FotoConLoAnotado<SinAnotar: View>: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    let lado: CGFloat
    @ViewBuilder let sinAnotar: () -> SinAnotar
    @State private var anotada: CGImage?
    @State private var recargas = 0
    @Environment(\.displayScale) private var escalaPantalla

    private var pixeles: Int { Int((lado * max(1, escalaPantalla)).rounded()) }

    var body: some View {
        Group {
            if let anotada {
                let tam = tamanoDeFotoAnotada(CGSize(width: anotada.width, height: anotada.height), lado: lado)
                Image(decorative: anotada, scale: max(1, escalaPantalla))
                    .resizable()
                    .interpolation(.high)
                    .frame(width: tam.width, height: tam.height)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                    .accessibilityLabel("Foto con lo anotado")
            } else {
                sinAnotar()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinLienzoGuardado)) { aviso in
            guard let referencia = aviso.object as? String, referencia == lienzoDeLaFoto(mensaje) else { return }
            FotosAnotadas.compartidas.olvidar(chat: chat, lienzo: referencia)
            recargas += 1
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            FotosAnotadas.compartidas.olvidar(chat: chat, lienzo: lienzoDeLaFoto(mensaje))
            recargas += 1
        }
        .task(id: "\(mensaje.id)#\(recargas)#\(pixeles)") {
            if case let .some(hecha) = FotosAnotadas.compartidas.hecha(mensaje, chat: chat, lado: pixeles) {
                anotada = hecha
                return
            }
            anotada = await FotosAnotadas.compartidas.imagen(mensaje, chat: chat, almacen: almacen, lado: pixeles)
        }
    }
}
