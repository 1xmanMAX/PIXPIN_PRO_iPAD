// Los cuadros de los pedidos en el iPad: el aviso flotante de Windows (una
// tarjeta que se va sola, `AvisoFlotante`), el «¿Hacerlo?» de los enlaces y la caja para escribir que
// hacer con unos ficheros. En el Mac son `NSAlert` que esperan; en el iPad
// nada puede esperar en el hilo principal: todos contestan llamando a
// `responder`. Se ponen encima de lo que se vea (tambien de un visor a
// pantalla completa), sin tocar la raiz de la app.

import Foundation
import UIKit

@MainActor
public enum CuadrosIPad {
    /// Otro que diga los avisos (las pruebas). Sin el, el globo.
    public static var alAvisar: ((String) -> Void)?
    /// Otro que conteste el «¿Hacerlo?» (las pruebas). Sin el, el cuadro.
    public static var alConfirmar: ((String) -> Bool)?
    /// Lo ultimo avisado (para las pruebas y para quien quiera repetirlo).
    public private(set) static var ultimoAviso: String?

    /// Lo que se ve encima de todo en la ventana activa.
    static func arriba() -> UIViewController? {
        let escenas = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        let escena = escenas.first { $0.activationState == .foregroundActive } ?? escenas.first
        let ventana = escena?.windows.first(where: \.isKeyWindow) ?? escena?.windows.first
        guard var vc = ventana?.rootViewController else { return nil }
        while let p = vc.presentedViewController, !p.isBeingDismissed { vc = p }
        return vc
    }

    /// **El aviso**: la tarjeta flotante de abajo a la derecha
    /// (`AvisoFlotante`), que se va sola a los pocos segundos. Con
    /// `botones` («Abrir», «Deshacer»...), dura lo de una respuesta.
    @discardableResult
    public static func avisar(_ texto: String, titulo: String = "PixPin",
                              botones: [BotonAviso] = []) -> TarjetaFlotante? {
        ultimoAviso = texto
        if let alAvisar {
            alAvisar(texto)
            return nil
        }
        return AvisoFlotante.compartido.mostrar(texto, titulo: titulo, botones: botones,
                                                dura: botones.isEmpty ? nil : AvisoFlotante.dura)
    }

    /// **¿Hacerlo?**: «No» es lo de por omision; «Sí» en rojo si `peligro`.
    public static func confirmar(_ cuerpo: String, titulo: String = "PixPin", si: String = "Sí",
                                 peligro: Bool = false, responder: @escaping @MainActor (Bool) -> Void) {
        if let alConfirmar { return responder(alConfirmar(cuerpo)) }
        guard let arriba = arriba(), !(arriba is UIAlertController) else { return responder(false) }
        let a = UIAlertController(title: titulo, message: cuerpo, preferredStyle: .alert)
        a.addAction(UIAlertAction(title: si, style: peligro ? .destructive : .default) { _ in
            MainActor.assumeIsolated { responder(true) }
        })
        let no = UIAlertAction(title: "No", style: .cancel) { _ in
            MainActor.assumeIsolated { responder(false) }
        }
        a.addAction(no)
        a.preferredAction = no
        arriba.present(a, animated: true)
    }

    /// Una caja para escribir (lo que hacer con unos ficheros). `nil`:
    /// cancelado o vacio.
    public static func pedirTexto(_ titulo: String, pista: String, boton: String = "Aceptar",
                                  responder: @escaping @MainActor (String?) -> Void) {
        guard let arriba = arriba(), !(arriba is UIAlertController) else { return responder(nil) }
        let a = UIAlertController(title: titulo, message: nil, preferredStyle: .alert)
        a.addTextField { $0.placeholder = pista }
        a.addAction(UIAlertAction(title: "Cancelar", style: .cancel) { _ in
            MainActor.assumeIsolated { responder(nil) }
        })
        a.addAction(UIAlertAction(title: boton, style: .default) { [weak a] _ in
            let t = a?.textFields?.first?.text?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            MainActor.assumeIsolated { responder(t.isEmpty ? nil : t) }
        })
        arriba.present(a, animated: true)
    }
}
