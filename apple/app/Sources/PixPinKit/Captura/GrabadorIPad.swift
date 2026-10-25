// Grabar un video de PixPin (lo que en Windows es «Grabar GIF / video de la
// zona», `grabador.rs`). iOS no deja grabar otras apps desde una app: para
// eso esta la grabacion de pantalla del Centro de control (y luego al chat
// con Compartir). Lo de la propia app se graba con ReplayKit
// (`RPScreenRecorder`, pide permiso la primera vez) y el video va a la
// galeria de capturas como `captura-AAAAMMDD-HHMMSS.mp4` (la galeria de
// Windows ya ensena los `.mp4`).
//
// Mientras graba, la capa de pines ensena arriba un boton rojo para parar.
//
// **El GIF** (`gif.rs` de Windows): se graba igual y, al parar, el video se
// pasa a GIF (10 fotogramas por segundo, 720 px de lado como mucho, en
// bucle) con AVFoundation e ImageIO; va a la galeria como
// `captura-AAAAMMDD-HHMMSS.gif` y el video de paso se borra.

import AVFoundation
import ImageIO
import ReplayKit
import UniformTypeIdentifiers
import SwiftUI
import UIKit

@MainActor
public final class GrabadorIPad: ObservableObject {
    public static let compartido = GrabadorIPad()

    @Published public private(set) var grabando = false
    /// Desde cuando (para el reloj del boton de parar).
    @Published private(set) var desde: Date?
    /// Lo que se graba acaba en GIF.
    @Published public private(set) var paraGif = false

    private var grabadora: RPScreenRecorder { RPScreenRecorder.shared() }

    /// Si este iPad deja grabar (no siempre: perfiles, AirPlay...).
    public var disponible: Bool { grabadora.isAvailable }

    /// Graba o para.
    public func alternar() {
        if grabando { parar() } else { empezar() }
    }

    /// Graba un GIF de PixPin (o lo para).
    public func alternarGif() {
        if grabando { parar() } else { empezar(gif: true) }
    }

    public func empezar(gif: Bool = false) {
        guard !grabando, grabadora.isAvailable else { return avisarFalloPin() }
        paraGif = gif
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
        let nombre = NombresCaptura.rutaNueva(en: carpeta, fecha: Date()).deletingPathExtension()
        let gif = paraGif
        grabando = false
        desde = nil
        paraGif = false
        if gif {
            let video = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-gif-\(UUID().uuidString).mp4")
            let destino = nombre.appendingPathExtension("gif")
            grabadora.stopRecording(withOutput: video) { error in
                Task { @MainActor in
                    guard error == nil else { return avisarFalloPin() }
                    let hecho = await Task.detached(priority: .userInitiated) { () -> Bool in
                        defer { try? FileManager.default.removeItem(at: video) }
                        return await GrabadorIPad.gif(de: video, a: destino)
                    }.value
                    if hecho {
                        UINotificationFeedbackGenerator().notificationOccurred(.success)
                        CapturadorIPad.compartido.alGuardar?(destino)
                    } else {
                        avisarFalloPin()
                    }
                }
            }
            return
        }
        let destino = nombre.appendingPathExtension("mp4")
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

extension GrabadorIPad {
    /// Fotogramas por segundo del GIF y su lado mas largo.
    nonisolated static let fpsGif: Double = 10
    nonisolated static let ladoGif: CGFloat = 720
    /// Lo mas largo que se pasa a GIF (lo de despues se deja).
    nonisolated static let segundosGif: Double = 60

    /// Los instantes de los fotogramas de un video de `duracion` segundos
    /// (puro: se prueba solo).
    nonisolated static func instantesGif(duracion: Double, fps: Double = fpsGif, tope: Double = segundosGif) -> [Double] {
        guard duracion.isFinite, duracion > 0, fps > 0 else { return [] }
        let n = max(1, Int((min(duracion, tope) * fps).rounded(.down)))
        return (0 ..< n).map { Double($0) / fps }
    }

    /// **Pasa un video a GIF** en bucle. `false` si no se pudo.
    nonisolated static func gif(de video: URL, a destino: URL) async -> Bool {
        let asset = AVURLAsset(url: video)
        guard let duracion = try? await asset.load(.duration).seconds else { return false }
        let instantes = instantesGif(duracion: duracion)
        guard !instantes.isEmpty,
              let salida = CGImageDestinationCreateWithURL(destino as CFURL, UTType.gif.identifier as CFString,
                                                           instantes.count, nil) else { return false }
        CGImageDestinationSetProperties(salida, [kCGImagePropertyGIFDictionary as String:
                                                    [kCGImagePropertyGIFLoopCount as String: 0]] as CFDictionary)
        let cada = [kCGImagePropertyGIFDictionary as String:
                        [kCGImagePropertyGIFDelayTime as String: 1 / fpsGif]] as CFDictionary
        let generador = AVAssetImageGenerator(asset: asset)
        generador.appliesPreferredTrackTransform = true
        generador.maximumSize = CGSize(width: ladoGif, height: ladoGif)
        let margen = CMTime(seconds: 0.5 / fpsGif, preferredTimescale: 600)
        generador.requestedTimeToleranceBefore = margen
        generador.requestedTimeToleranceAfter = margen
        var anterior: CGImage?
        for t in instantes {
            let foto = try? await generador.image(at: CMTime(seconds: t, preferredTimescale: 600)).image
            // Un fotograma que no sale repite el anterior (la cuenta del
            // GIF ya esta dicha).
            guard let img = foto ?? anterior else { continue }
            CGImageDestinationAddImage(salida, img, cada)
            anterior = img
        }
        guard anterior != nil else { return false }
        return CGImageDestinationFinalize(salida)
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
                    Text(grabador.paraGif ? "Parar el GIF" : "Parar")
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
