// **Las clases grabadas en el chat** (del Mac, `Grabar/VisorClase.swift`):
// que mensajes son de una clase y sus capturas, leidos del cuaderno por el
// nucleo y guardados aqui por mensaje (el menu y la burbuja preguntan a
// menudo y el cuaderno no se lee cada vez).
//
// - `capturas`: las de una nota de voz grabada como clase
//   (`Nucleo.capturasDeLaClase`, el `capturas` de la voz), para «Enviar a
//   Anki».
// - `deUnaClase`: si un mensaje es de una clase (la nota, una captura o el
//   video: `estudiar_clase::de_una_clase` de Windows), para «Estudiar la
//   clase» en su menu y la pastilla de la nota de voz. La pantalla es
//   `EstudiarClase` (la que sustituye a la de la letra con las capturas,
//   como en Windows ce6c058).

import PixPinCore

/// Lo de las clases en el chat, por mensaje.
@MainActor
enum ClaseEnElChat {
    private static var leidas: [String: [CapturaDeLaClase]] = [:]
    /// `nil` dentro: se miro y no es de ninguna clase.
    private static var clases: [String: EnUnaClase?] = [:]

    private static func clave(_ chat: String, _ id: String) -> String { chat + "\u{0}" + id }

    /// Las capturas de `m` (vacio si no es una nota de voz de una clase).
    /// La primera vez lee el cuaderno.
    static func capturas(_ m: MensajeChat, chat: String, nucleo: Nucleo) -> [CapturaDeLaClase] {
        guard m.clase == "VOZ" else { return [] }
        let k = clave(chat, m.id)
        if let ya = leidas[k] { return ya }
        let v = (try? nucleo.capturasDeLaClase(chat: chat, mensaje: m.id)) ?? []
        leidas[k] = v
        return v
    }

    /// **De que clase es `m`**: su nota de voz y el minuto donde abrirla.
    /// `nil` si no es de ninguna. Solo una nota de voz, una foto o un
    /// archivo (el video) pueden serlo. La primera vez lee el cuaderno.
    static func deUnaClase(_ m: MensajeChat, chat: String, nucleo: Nucleo) -> EnUnaClase? {
        guard m.clase == "VOZ" || m.clase == "IMAGEN" || m.clase == "ARCHIVO" else { return nil }
        let k = clave(chat, m.id)
        if let ya = clases[k] { return ya }
        let v: EnUnaClase? = try? nucleo.deUnaClase(chat: chat, mensaje: m.id)
        clases[k] = .some(v)
        return v
    }

    /// Una nota que acaba de cambiar (se guardo la clase): se vuelve a leer.
    static func olvidar(chat: String, mensaje: String) {
        leidas[clave(chat, mensaje)] = nil
        clases[clave(chat, mensaje)] = nil
    }

    /// La captura que toca en `posicionMs`: la ultima hecha antes (la
    /// primera si aun no llego ninguna). `nil` sin capturas.
    nonisolated static func enElMinuto(_ capturas: [CapturaDeLaClase], posicionMs: Int64) -> Int? {
        captura(deLaClaseEn: capturas.map(\.ms), posicionMs: posicionMs)
    }

    nonisolated static func captura(deLaClaseEn tiempos: [Int64], posicionMs: Int64) -> Int? {
        capturaDeLaClaseEn(tiempos: tiempos, posicionMs: posicionMs).map(Int.init)
    }
}
