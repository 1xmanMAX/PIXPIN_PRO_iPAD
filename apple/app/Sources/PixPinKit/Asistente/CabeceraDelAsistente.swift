// La barra de arriba del chat «Asistente» (Windows, 17-oct): la pastilla
// del modelo (Local / Claude Haiku / DeepSeek), que cambia el MISMO ajuste
// que Ajustes → Asistente → Motor, al instante; y «Conversación nueva», una
// nota del asistente que corta el contexto que se le manda (lo de antes
// sigue a la vista y viaja como siempre).

import PixPinCore
import SwiftUI
import UIKit

struct CabeceraDelAsistente: View {
    let almacen: Almacen
    let chat: String
    @State private var motor = ""

    static let motores: [(id: String, nombre: String)] = [("", "En el iPad"), ("claude", "Claude Haiku"),
                                                           ("deepseek", "DeepSeek")]

    static func nombre(de motor: String) -> String {
        motores.first { $0.id == motor }?.nombre ?? "En el iPad"
    }

    var body: some View {
        HStack(spacing: 8) {
            Menu {
                ForEach(Self.motores, id: \.id) { m in
                    Button {
                        elegir(m.id)
                    } label: {
                        if m.id == motor { Label(m.nombre, systemImage: "checkmark") } else { Text(m.nombre) }
                    }
                }
            } label: {
                Label(Self.nombre(de: motor), systemImage: "sparkles")
                    .font(.caption)
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .background(Color.accentColor.opacity(0.15), in: Capsule())
            }
            .menuStyle(.button)
            .buttonStyle(.borderless)
            .fixedSize()
            .help("Quién contesta (el mismo ajuste que Ajustes → Asistente)")
            Spacer()
            Button {
                nueva()
            } label: {
                Label("Conversación nueva", systemImage: "square.and.pencil")
                    .font(.caption)
            }
            .buttonStyle(.borderless)
            .help("Lo de arriba ya no se le manda al asistente")
        }
        .padding(.horizontal, 12).padding(.vertical, 5)
        .task { motor = (try? almacen.nucleo.ajustes().asistenteMotor) ?? "" }
    }

    private func elegir(_ nuevo: String) {
        guard var a = try? almacen.nucleo.ajustes() else { return }
        a.asistenteMotor = nuevo
        do {
            try almacen.nucleo.guardarAjustes(nuevos: a)
            motor = nuevo
        } catch {
            CabeceraDelAsistente.pitar()
        }
    }

    private func nueva() {
        let (nucleo, chat) = (almacen.nucleo, chat)
        Task {
            guard let m = try? await Task.detached(operation: { try nucleo.iaConversacionNueva(chat: chat) }).value
            else { return CabeceraDelAsistente.pitar() }
            almacen.anadir([m], en: chat)
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        }
    }

    /// El «beep» del Mac: en el iPad, la vibracion de error.
    static func pitar() {
        UINotificationFeedbackGenerator().notificationOccurred(.error)
    }
}
