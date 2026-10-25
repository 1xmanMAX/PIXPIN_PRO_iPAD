// **Dos idiomas en una misma nota** (Windows `pixpin_voz::dos_idiomas`, el
// `MotorWhisper.reconocer` del movil), con el dictado del iPad.
//
// El primer idioma es el de la app (`es-ES`); el segundo, el de Ajustes
// (`vozSegundoIdioma` del TOML, el mismo campo que Windows y el movil).
// Whisper decide trozo a trozo; el reconocedor del iPad solo entiende un
// idioma cada vez, asi que con «Cada trozo como se dijo» la nota se pasa a
// texto **en los dos** y se queda la version de la que el reconocedor esta
// mas seguro. Con «Todo en el primero» solo el primero (traducir no sabe).
// Si el iPad no tiene el dictado sin conexion del segundo idioma, como si
// no hubiera segundo.

import Foundation
import PixPinCore

enum DosIdiomasVoz {
    /// La etiqueta del reconocedor de cada codigo corto de Ajustes.
    static let etiquetas: [String: String] = [
        "en": "en-US", "es": "es-ES", "pt": "pt-BR", "fr": "fr-FR", "de": "de-DE", "it": "it-IT", "ca": "ca-ES",
    ]

    /// Cuanto mas seguro tiene que estar el segundo para ganar al primero.
    static let ventaja = 0.05

    /// **El segundo idioma que se prueba**, o `nil`: sin segundo, uno que no
    /// se conoce, el mismo que el primero o el modo «todo en el primero».
    static func segundo(_ codigo: String, modo: String, principal: String) -> String? {
        let c = codigo.split(separator: "-").first.map { $0.trimmingCharacters(in: .whitespaces).lowercased() } ?? ""
        guard !c.isEmpty, modo.trimmingCharacters(in: .whitespaces) != "todo_en_uno",
              let etiqueta = etiquetas[c], VozAlta.partes(principal).lengua != c else { return nil }
        return etiqueta
    }

    /// **Con cual se queda**: el segundo solo si dice algo y el reconocedor
    /// esta claramente mas seguro de el; si el primero no oyo nada, el otro.
    static func elegir(principal: TranscriptorVoz.Reconocido,
                       otro: TranscriptorVoz.Reconocido?) -> [SegmentoVoz] {
        guard let otro, !otro.segmentos.isEmpty else { return principal.segmentos }
        if principal.segmentos.isEmpty { return otro.segmentos }
        return otro.confianza > principal.confianza + ventaja ? otro.segmentos : principal.segmentos
    }

    /// Pasa a texto `url` con el primer idioma y, si hay, el segundo de los
    /// ajustes. Lanza lo que lance el primero (sin permiso, sin dictado).
    static func transcribir(_ url: URL, nucleo: Nucleo) async throws -> [SegmentoVoz] {
        let ajustes = await Task.detached(priority: .utility) { try? nucleo.ajustes() }.value
        let primero = try await TranscriptorVoz.transcribirConConfianza(url)
        guard let a = ajustes,
              let etiqueta = segundo(a.vozSegundoIdioma, modo: a.vozModoDeIdiomas,
                                     principal: TranscriptorVoz.idiomaPreferido.identifier),
              TranscriptorVoz.reconocedorLocal(idioma: Locale(identifier: etiqueta)) != nil
        else { return primero.segmentos }
        let otro = try? await TranscriptorVoz.transcribirConConfianza(url, idioma: Locale(identifier: etiqueta))
        return elegir(principal: primero, otro: otro)
    }
}
