// Quien escucha la respuesta del asistente mientras llega: el `OyenteIa`
// del nucleo de Rust (un interfaz que implementa Swift). Rust lo llama
// desde el hilo del turno, el texto en tandas (cada ~60 ms como mucho);
// aqui solo se pasa cada cosa a quien la pinta, que la lleva al hilo
// principal.

import Foundation
import PixPinCore

final class OyenteDelAsistente: OyenteIa, @unchecked Sendable {
    let alTexto: @Sendable (String) -> Void
    let alHerramienta: @Sendable (HerramientaUsada) -> Void
    let alPensar: @Sendable (String) -> Void
    let alLlamar: @Sendable (String) -> Void

    init(alTexto: @escaping @Sendable (String) -> Void,
         alHerramienta: @escaping @Sendable (HerramientaUsada) -> Void = { _ in },
         alPensar: @escaping @Sendable (String) -> Void = { _ in },
         alLlamar: @escaping @Sendable (String) -> Void = { _ in }) {
        self.alTexto = alTexto
        self.alHerramienta = alHerramienta
        self.alPensar = alPensar
        self.alLlamar = alLlamar
    }

    func texto(trozo: String) {
        alTexto(trozo)
    }

    func herramienta(usada: HerramientaUsada) {
        alHerramienta(usada)
    }

    func pensando(trozo: String) {
        alPensar(trozo)
    }

    func llamando(nombre: String) {
        alLlamar(nombre)
    }
}
