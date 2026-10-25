// El pin de video del iPad: un `AVPlayerLayer` dentro de la capa de pines y
// **Picture in Picture** (`AVPictureInPictureController`) para que el video
// siga flotando encima de otras apps, que es lo que en Windows hace un pin de
// video encima de todo. Al irse PixPin a segundo plano con el video en
// marcha, entra solo en PiP (`canStartPictureInPictureAutomaticallyFromInline`).
//
// Hace falta el modo de fondo `audio` del Info.plist (ya esta) y la sesion
// de audio en `.playback`.

import AVFoundation
import AVKit
import Combine
import SwiftUI
import UIKit

/// Una vista cuya capa es un `AVPlayerLayer`.
final class VistaCapaVideo: UIView {
    override class var layerClass: AnyClass { AVPlayerLayer.self }
    var capaVideo: AVPlayerLayer { layer as! AVPlayerLayer }
}

struct VideoPinIPad: UIViewRepresentable {
    let reproductor: AVPlayer
    let pin: PinIPad

    func makeUIView(context: Context) -> VistaCapaVideo {
        let v = VistaCapaVideo()
        v.backgroundColor = .black
        v.capaVideo.player = reproductor
        v.capaVideo.videoGravity = .resizeAspect
        if AVPictureInPictureController.isPictureInPictureSupported() {
            let pip = AVPictureInPictureController(playerLayer: v.capaVideo)
            pip?.canStartPictureInPictureAutomaticallyFromInline = true
            context.coordinator.pip = pip
            pin.pip = pip
        }
        return v
    }

    func updateUIView(_ v: VistaCapaVideo, context: Context) {
        if v.capaVideo.player !== reproductor { v.capaVideo.player = reproductor }
    }

    func makeCoordinator() -> Coordinador { Coordinador() }

    /// Guarda el controlador de PiP mientras viva la vista (el pin solo lo
    /// apunta debil).
    final class Coordinador {
        var pip: AVPictureInPictureController?
    }
}

/// El reproductor del pin con sus botones (reproducir, PiP).
struct VistaVideoPin: View {
    let reproductor: AVPlayer
    let soloAudio: Bool
    @ObservedObject var pin: PinIPad
    @State private var enMarcha = false

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            if soloAudio {
                Paleta.v2.tarjeta
                Image(systemName: "waveform").font(.largeTitle).foregroundStyle(Paleta.v2.gris)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                VideoPinIPad(reproductor: reproductor, pin: pin)
            }
            HStack(spacing: 14) {
                Button { alternar() } label: {
                    Image(systemName: enMarcha ? "pause.fill" : "play.fill")
                }
                .help(enMarcha ? "Pausa" : "Reproducir")
                if !soloAudio {
                    Button { pin.pip?.startPictureInPicture() } label: {
                        Image(systemName: "pip.enter")
                    }
                    .help("Picture in Picture: flota encima de otras apps")
                }
            }
            .font(.system(size: 16, weight: .semibold))
            .foregroundStyle(.white)
            .padding(8)
            .background(.black.opacity(0.45), in: Capsule())
            .padding(8)
        }
        .onReceive(NotificationCenter.default.publisher(for: AVPlayerItem.didPlayToEndTimeNotification)) { aviso in
            guard (aviso.object as? AVPlayerItem) === reproductor.currentItem else { return }
            enMarcha = false
            reproductor.seek(to: .zero)
        }
    }

    private func alternar() {
        if enMarcha {
            reproductor.pause()
        } else {
            try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback)
            try? AVAudioSession.sharedInstance().setActive(true)
            reproductor.isMuted = false
            reproductor.play()
        }
        enMarcha.toggle()
    }
}
