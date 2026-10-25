// **Lo que un pedido manda buscar en una seccion** (`lecciones {consulta}`
// de Windows: abre la lista de lecciones ya buscando). El pedido lo deja
// aqui y la seccion lo recoge al aparecer (o en el acto si ya esta
// delante) y lo borra.

import Combine
import Foundation

@MainActor
public final class ConsultaPedida: ObservableObject {
    public static let lecciones = ConsultaPedida()

    /// Lo que hay que poner en el buscador (nil: nada pendiente).
    @Published public private(set) var texto: String?

    public init() {}

    /// Lo pide un pedido (vacio o solo blancos: nada).
    public func poner(_ t: String?) {
        let limpio = t?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        texto = limpio.isEmpty ? nil : limpio
    }

    /// Lo recoge la seccion: lo devuelve una vez y lo olvida.
    public func tomar() -> String? {
        defer { texto = nil }
        return texto
    }
}
