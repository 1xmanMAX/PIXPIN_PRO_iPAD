// Grabar un video de PixPin (lo que en Windows es «Grabar GIF / video de la
// zona», `grabador.rs`). iOS no deja grabar otras apps desde una app: para
// eso esta la grabacion de pantalla del Centro de control (y luego al chat
// con Compartir). Lo de la propia app se graba con ReplayKit
// (`RPScreenRecorder`, pide permiso la primera vez) y el video va a la
// galeria de capturas como `captura-AAAAMMDD-HHMMSS.mp4` (la galeria de
// Windows ya ensena los `.mp4`).
//
// Mientras graba, la capa de pines ensena arriba un boton rojo para parar.

import ReplayKit
import SwiftUI
import UIKit

@MainActor
public final class GrabadorIPad: ObservableObject {
    public static let compartido = GrabadorIPad()

    @Published public private(set) var grabando = false
    /// Desde cuando (para el reloj del boton de parar).
    @Published private(set) var desde: Date?

    private var grabadora: RPScreenRecorder { RPScreenRecorder.shared() }

    /// Si este iPad deja grabar (no siempre: perfiles, AirPlay...).
    public var disponible: Bool { grabadora.isAvailable }

    /// Graba o para.
    public func alternar() {
        if grabando { parar() } else { empezar() }
    }

    public func empezar() {
        guard !grabando, grabadora.isAvailable else { return avisarFalloPin() }
        grabadora.isMicrophoneEnabled = false
        grabadora.startRecording { error in
            Task { @MainActor in
                let yo = GrabadorIPad.compartido
                if error != nil {
                    yo.grabando = false
                    avisarFalloPin()
                } else {
                    yo.grabando = true
                    yo.desde = Date()
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                }
            }
        }
    }

    /// Para y deja el video en la galeria de capturas.
    public func parar() {
        guard grabando else { return }
        let carpeta = CapturadorIPad.compartido.carpetaDeCapturas
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        let destino = NombresCaptura.rutaNueva(en: carpeta, fecha: Date())
            .deletingPathExtension().appendingPathExtension("mp4")
        grabando = false
        desde = nil
        grabadora.stopRecording(withOutput: destino) { error in
            Task { @MainActor in
                if error != nil {
                    avisarFalloPin()
                } else {
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    CapturadorIPad.compartido.alGuardar?(destino)
                }
            }
        }
    }
}

/// El boton rojo de parar, arriba en medio de la capa de pines.
struct BotonPararGrabacion: View {
    @ObservedObject var grabador: GrabadorIPad

    var body: some View {
        if grabador.grabando {
            Button { grabador.parar() } label: {
                HStack(spacing: 8) {
                    Circle().fill(Color.white).frame(width: 10, height: 10)
                    if let desde = grabador.desde {
                        Text(desde, style: .timer).monospacedDigit()
                    }
                    Text("Parar")
                }
                .font(.callout.weight(.semibold))
                .foregroundStyle(.white)
                .padding(.horizontal, 14)
                .frame(height: 36)
                .background(Paleta.v2.rojo, in: Capsule())
                .shadow(color: .black.opacity(0.3), radius: 6, y: 2)
            }
            .buttonStyle(.plain)
            .help("Parar la grabación")
            .accessibilityLabel("Parar la grabación")
        }
    }
}
