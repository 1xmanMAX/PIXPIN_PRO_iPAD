// **El boton de sincronizar gira mientras se sincroniza** (Windows 0470294,
// 25-oct-2026, el usuario: «mientras se sincroniza, que el boton gire»).
// Portado del Mac (`Sincro/GiroDeSincronizar.swift`).
//
// - `EstadoDeLaVuelta`: cuantas vueltas hay en marcha y como acabo la
//   ultima, venga de donde venga (`EnCurso`, `ultima_fallo` y
//   `olvidar_fallo` de `sincronizar.rs` de Windows): la pulsada en
//   Sincronizar, la automatica, las tareas al instante, los chats
//   compartidos con otras personas y la que pide el otro aparato al llamar a
//   este (`AvisoDeLaPuerta`). Las sondas no cuentan.
// - `BotonSincronizar`: el disco dorado de la cabecera de la lista. Gira una
//   vuelta por segundo, en el sentido de las agujas del reloj, mientras haya
//   alguna vuelta (`pixpin_ui::giro`: el angulo sale del tiempo desde que
//   empezo; quieto, no hay temporizador); al acabar vuelve a su sitio. Si la
//   ultima fallo, lleva la marca roja con «!» arriba a la derecha (`#E53935`
//   con aro del color de la lista) hasta que se pulsa o empieza otra.

import PixPinCore
import SwiftUI

@MainActor
public final class EstadoDeLaVuelta: ObservableObject {
    public static let compartido = EstadoDeLaVuelta()

    /// Cuantas vueltas hay en marcha ahora.
    @Published public private(set) var enMarcha = 0
    /// Desde cuando gira (para el angulo); `nil`, quieto.
    @Published public private(set) var desde: Date?
    /// La ultima vuelta que dijo como acababa fallo, y nadie lo ha visto.
    @Published public private(set) var fallo = false

    public init() {}

    /// Empieza una vuelta. La primera tapa el fallo de la anterior: si
    /// vuelve a fallar, se dira otra vez al acabar.
    public func empieza() {
        if enMarcha == 0 {
            fallo = false
            desde = Date()
        }
        enMarcha += 1
    }

    /// Acaba una vuelta: `bien` si lo sabe quien la lleva (`nil`, cancelada
    /// o sin saberlo: no cambia la marca).
    public func acaba(bien: Bool?) {
        if let bien { fallo = !bien }
        enMarcha = max(0, enMarcha - 1)
        if enMarcha == 0 { desde = nil }
    }

    /// Ya se vio el fallo (se pulso el boton): vuelve a ser el de siempre.
    public func olvidarFallo() {
        fallo = false
    }
}

/// El `AvisoDeVuelta` del nucleo para la puerta: lo que llama de fuera
/// (que no sea una sonda) cuenta como vuelta en marcha.
final class AvisoDeLaPuerta: AvisoDeVuelta, @unchecked Sendable {
    func empieza() {
        DispatchQueue.main.async {
            MainActor.assumeIsolated { EstadoDeLaVuelta.compartido.empieza() }
        }
    }

    func acaba(bien: Bool) {
        DispatchQueue.main.async {
            MainActor.assumeIsolated { EstadoDeLaVuelta.compartido.acaba(bien: bien) }
        }
    }
}

/// **El boton redondo de sincronizar**, que gira mientras hay una vuelta
/// (el disco dorado de `BotonRedondoLista`).
struct BotonSincronizar: View {
    let accion: () -> Void
    @ObservedObject private var estado = EstadoDeLaVuelta.compartido

    var body: some View {
        Button {
            // La pantalla de Sincronizar dice que paso: la marca ya se vio.
            estado.olvidarFallo()
            accion()
        } label: {
            // Solo se repinta 30 veces por segundo mientras gira.
            TimelineView(.animation(minimumInterval: 1.0 / 30, paused: estado.desde == nil)) { tl in
                Simbolo(nombre: "arrow.triangle.2.circlepath", lado: 18)
                    .rotationEffect(.degrees(angulo(tl.date)))
            }
            .foregroundStyle(Paleta.chat.circuloIcono)
            .frame(width: 38, height: 38)
            .background(Paleta.chat.circulo, in: Circle())
            .overlay(alignment: .topTrailing) {
                if estado.fallo { MarcaDeFallo().offset(x: 3, y: -3) }
            }
            .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.lift)
        .accessibilityLabel("Sincronizar")
        .accessibilityValue(estado.desde != nil ? "Sincronizando" : estado.fallo ? "Falló la última vez" : "")
    }

    /// Una vuelta por segundo desde que empezo (`Giro::angulo`).
    private func angulo(_ ahora: Date) -> Double {
        guard let desde = estado.desde else { return 0 }
        let t = max(0, ahora.timeIntervalSince(desde)).truncatingRemainder(dividingBy: 1)
        return t * 360
    }
}

/// La marca roja con «!» de la ultima vuelta que fallo.
private struct MarcaDeFallo: View {
    var body: some View {
        ZStack {
            Circle().fill(Paleta.proyectos.lista)
            Circle().fill(Color(hex: 0xE53935)).padding(1.5)
            Text("!")
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(.white)
        }
        .frame(width: 14, height: 14)
    }
}
