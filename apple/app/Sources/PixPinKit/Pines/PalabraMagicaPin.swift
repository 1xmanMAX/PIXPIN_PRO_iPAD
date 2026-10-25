// **La palabra magica** (`crates/pixpin-pin/src/magia.rs` de Windows,
// `MagicWord.kt` del movil): copiar `timer`, `todo`, `gastos`, `pizarra`,
// `hoja`... **sola** y pinear lo copiado saca la herramienta en vez de una
// nota. Solo cuando lo UNICO copiado es esa palabra (sin espacios ni mas
// largo que la palabra mas larga): copiar «time» dentro de un texto sigue
// dando una nota. Las palabras son las de fabrica de Windows (las propias
// de Ajustes › Pinear llegaran cuando el nucleo las de).

import Foundation

enum PalabraMagicaPin: Equatable {
    /// Una mini-app de los pines (`temporizador`, `cronometro`, `tareas`,
    /// `contador`, `gastos`, `ruleta`).
    case miniapp(String)
    /// Un lienzo nuevo (pizarra, lienzo u hoja).
    case lienzo

    /// Las de fabrica (`por_defecto` de Windows).
    static let deFabrica: [String: PalabraMagicaPin] = [
        "time": .miniapp("temporizador"), "timer": .miniapp("temporizador"), "pomodoro": .miniapp("temporizador"),
        "temporizador": .miniapp("temporizador"),
        "crono": .miniapp("cronometro"), "cronometro": .miniapp("cronometro"), "cronómetro": .miniapp("cronometro"),
        "stopwatch": .miniapp("cronometro"),
        "todo": .miniapp("tareas"), "checklist": .miniapp("tareas"), "compras": .miniapp("tareas"),
        "tareas": .miniapp("tareas"),
        "count": .miniapp("contador"), "contador": .miniapp("contador"),
        "gastos": .miniapp("gastos"), "cuentas": .miniapp("gastos"), "money": .miniapp("gastos"),
        "ruleta": .miniapp("ruleta"), "roulette": .miniapp("ruleta"), "choose": .miniapp("ruleta"),
        "sorteo": .miniapp("ruleta"), "sortear": .miniapp("ruleta"), "elegir": .miniapp("ruleta"),
        "board": .lienzo, "pizarra": .lienzo, "canvas": .lienzo, "lienzo": .lienzo, "excalidraw": .lienzo,
        "hoja": .lienzo, "sheet": .lienzo,
    ]

    /// Lo que abre `texto`, si es una palabra magica sola (`detectar`).
    static func detectar(_ texto: String, palabras: [String: PalabraMagicaPin] = deFabrica) -> PalabraMagicaPin? {
        let t = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty, let larga = palabras.keys.map(\.count).max(), t.count <= larga,
              !t.contains(where: \.isWhitespace) else { return nil }
        return palabras[t.lowercased()]
    }
}
