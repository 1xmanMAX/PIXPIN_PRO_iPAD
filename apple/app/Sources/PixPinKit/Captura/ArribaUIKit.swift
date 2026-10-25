// Lo comun de UIKit que usan la captura y los pines: el controlador de mas
// arriba de la ventana activa (para ensenar hojas desde cualquier sitio, este
// abierto un visor o no), poner una vista de SwiftUI encima, y pasar fotos
// de UIKit (`UIImage`, con su orientacion) a `CGImage` derecho y a PNG.

import CoreGraphics
import ImageIO
import SwiftUI
import UIKit
import UniformTypeIdentifiers

@MainActor
enum ArribaUIKit {
    /// La escena de delante (con Stage Manager, la que tiene el foco).
    static func escena() -> UIWindowScene? {
        let escenas = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        return escenas.first { $0.activationState == .foregroundActive } ?? escenas.first
    }

    /// La ventana principal de la escena de delante.
    static func ventana() -> UIWindow? {
        guard let escena = escena() else { return nil }
        return escena.windows.first { $0.isKeyWindow } ?? escena.windows.first
    }

    /// Los pixeles por punto de la pantalla del iPad (2 en todos los de hoy).
    static var escalaPantalla: CGFloat {
        ventana()?.screen.scale ?? 2
    }

    /// El controlador de mas arriba: el visor, una hoja abierta... o la raiz.
    static func controladorDeArriba() -> UIViewController? {
        var vc = ventana()?.rootViewController
        while let p = vc?.presentedViewController, !p.isBeingDismissed {
            vc = p
        }
        return vc
    }

    /// Pone `vista` encima de todo. `contenido` recibe con que cerrarla.
    /// `.formSheet` es una hoja en medio; `.fullScreen`, la pantalla entera.
    @discardableResult
    static func presentar<V: View>(estilo: UIModalPresentationStyle = .formSheet,
                                   tamano: CGSize? = nil,
                                   @ViewBuilder _ contenido: (@escaping () -> Void) -> V) -> UIViewController? {
        guard let arriba = controladorDeArriba() else { return nil }
        let caja = CajaCerrar()
        let host = UIHostingController(rootView: contenido { caja.cerrar() })
        caja.controlador = host
        host.modalPresentationStyle = estilo
        if let tamano { host.preferredContentSize = tamano }
        if estilo == .fullScreen {
            host.view.backgroundColor = .black
        }
        arriba.present(host, animated: true)
        return host
    }

    /// Pone un controlador de UIKit (camara, escaner, Fotos) encima de todo.
    static func presentar(_ controlador: UIViewController) {
        guard let arriba = controladorDeArriba() else { return }
        arriba.present(controlador, animated: true)
    }

    /// Lo que cierra la hoja: el controlador se apunta despues de crearlo.
    private final class CajaCerrar {
        weak var controlador: UIViewController?
        /// Se le pide a quien la presento: asi se cierra tambien lo que ella
        /// tuviera encima (una hoja abierta desde el editor).
        @MainActor func cerrar() {
            guard let controlador else { return }
            if let quien = controlador.presentingViewController {
                quien.dismiss(animated: true)
            } else {
                controlador.dismiss(animated: true)
            }
        }
    }
}

/// Fotos de UIKit a lo que usan los pines y el nucleo.
enum ImagenIPad {
    /// La foto derecha (la orientacion de la camara ya aplicada) y a sus
    /// pixeles de verdad. `nil` si no hay de donde sacarla.
    static func derecha(_ foto: UIImage) -> CGImage? {
        if foto.imageOrientation == .up, let cg = foto.cgImage { return cg }
        let formato = UIGraphicsImageRendererFormat()
        formato.scale = 1
        formato.opaque = false
        let tam = CGSize(width: (foto.size.width * foto.scale).rounded(), height: (foto.size.height * foto.scale).rounded())
        guard tam.width >= 1, tam.height >= 1 else { return nil }
        let dibujada = UIGraphicsImageRenderer(size: tam, format: formato).image { _ in
            foto.draw(in: CGRect(origin: .zero, size: tam))
        }
        return dibujada.cgImage
    }

    /// PNG con ImageIO (`nil` si no se pudo).
    static func png(_ imagen: CGImage) -> Data? {
        let datos = NSMutableData()
        guard let destino = CGImageDestinationCreateWithData(datos as CFMutableData, UTType.png.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(destino, imagen, nil)
        guard CGImageDestinationFinalize(destino) else { return nil }
        return datos as Data
    }

    /// Hace `trabajo` en una cola de fondo y devuelve su resultado (con una
    /// cola y no `Task.detached` porque `CGImage` no es `Sendable`).
    static func fuera<T>(_ trabajo: @escaping () -> T) async -> T {
        await withCheckedContinuation { (c: CheckedContinuation<T, Never>) in
            DispatchQueue.global(qos: .userInitiated).async {
                c.resume(returning: trabajo())
            }
        }
    }
}
