// **La zona vinculada** (`salto_por_enlace.rs` de Windows,
// `DrawEditorActivity.abrirSublienzo` del movil): la marca azul que deja
// «Zona al chat» lleva en su esquina un icono de enlace; tocarlo abre el
// sublienzo de la zona (resuelto por el nucleo, `dibujoDelEnlace`, que
// entiende tambien los enlaces de un `.pixpin` importado antes de que el
// movil los renombrara). Desde el sublienzo, «Volver» abre el de origen.
//
// Un enlace que no lleva a ningun lienzo del proyecto avisa y el lienzo no
// se cierra.

import PixPinCore
import UIKit

/// **Los saltos por enlace**, para volver: de que lienzo se salto a cual.
@MainActor
public enum SaltosDeLienzo {
    public struct Salto: Equatable {
        public let chat: String
        /// El mensaje del lienzo de origen (para abrirlo otra vez).
        public let origen: MensajeChat
        public let titulo: String
        /// El dibujo al que se salto.
        public let destino: String
    }

    public private(set) static var pila: [Salto] = []

    static func apuntar(_ s: Salto) {
        pila.removeAll { $0.destino == s.destino }
        pila.append(s)
        if pila.count > 20 { pila.removeFirst(pila.count - 20) }
    }

    /// El salto que trajo al dibujo `destino`, si lo hubo.
    public static func vuelta(a destino: String?) -> Salto? {
        guard let destino else { return nil }
        return pila.last { $0.destino == destino }
    }

    static func quitar(_ s: Salto) {
        pila.removeAll { $0 == s }
    }

    /// Para las pruebas.
    static func olvidar() { pila = [] }
}

extension ModeloEditorLienzo {
    /// **Sigue el enlace de una zona**: busca su sublienzo en el proyecto y
    /// lo abre (lo de ahora se guarda antes); si no lleva a nada, lo dice.
    func seguirEnlace(_ enlace: String) {
        guard let chat, let nucleo, let alSeguirEnlace else { return }
        let referencia = self.referencia
        let titulo = self.titulo
        guardarAhora(terminarTexto: true)
        Task { [weak self] in
            let (destino, origen) = await Task.detached(priority: .userInitiated) { () -> (MensajeChat?, MensajeChat?) in
                guard let d = nucleo.dibujoDelEnlace(chat: chat, enlace: enlace) else { return (nil, nil) }
                let origen = referencia.flatMap { nucleo.mensajeDelDibujo(chat: chat, dibujo: $0) }
                return (nucleo.mensajeDelDibujo(chat: chat, dibujo: d), origen)
            }.value
            guard let self else { return }
            guard let destino, let dibujo = destino.referencia else {
                UINotificationFeedbackGenerator().notificationOccurred(.error)
                if let vista = self.vista {
                    CajetinesLienzo.avisar("Zona vinculada", "El enlace no lleva a ninguna hoja de este proyecto.",
                                           desde: vista)
                }
                return
            }
            if let origen {
                SaltosDeLienzo.apuntar(.init(chat: chat, origen: origen, titulo: titulo, destino: dibujo))
            }
            alSeguirEnlace(destino)
        }
    }
}
