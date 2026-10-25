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

/// El reproductor del pin con sus botones (reproducir, PiP). Un audio
/// lleva su barra propia (la del reproductor flotante de Windows,
/// `reproductor.rs`): por donde va, saltar 15 s y la velocidad.
struct VistaVideoPin: View {
    let reproductor: AVPlayer
    let soloAudio: Bool
    @ObservedObject var pin: PinIPad
    @State private var enMarcha = false
    @State private var tiempo: Double = 0
    @State private var duracion: Double = 0
    @State private var velocidad: Float = 1
    /// Mientras se arrastra la barra no la mueve el reloj.
    @State private var buscando = false

    private let reloj = Timer.publish(every: 0.5, on: .main, in: .common).autoconnect()

    var body: some View {
        if soloAudio { barraAudio } else { video }
    }

    private var barraAudio: some View {
        VStack(spacing: 4) {
            Spacer(minLength: 26)
            if pin.pistas.count > 1 {
                // La fila del reproductor flotante: cual suena y de cuantas.
                Text(ReglasAudioPin.fila(pin.pistaActual, de: pin.pistas.count,
                                         nombre: pin.pistas[min(pin.pistaActual, pin.pistas.count - 1)].lastPathComponent))
                    .font(.caption2)
                    .foregroundStyle(Paleta.v2.gris)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            Slider(value: $tiempo, in: 0 ... max(duracion, 0.1), onEditingChanged: { editando in
                buscando = editando
                if !editando { reproductor.seek(to: CMTime(seconds: tiempo, preferredTimescale: 600)) }
            })
            .tint(Paleta.v2.acento)
            HStack(spacing: 12) {
                Text(ReglasAudioPin.reloj(tiempo)).monospacedDigit()
                Spacer(minLength: 0)
                Button { saltar(-15) } label: { Image(systemName: "gobackward.15") }
                    .help("Atrás 15 s")
                Button { alternar() } label: {
                    Image(systemName: enMarcha ? "pause.fill" : "play.fill").font(.system(size: 20, weight: .semibold))
                }
                .help(enMarcha ? "Pausar" : "Reproducir")
                Button { saltar(15) } label: { Image(systemName: "goforward.15") }
                    .help("Adelante 15 s")
                Menu {
                    ForEach(ReglasAudioPin.velocidades, id: \.self) { v in
                        Button { ponerVelocidad(v) } label: {
                            if v == velocidad { Label(ReglasAudioPin.rotulo(v), systemImage: "checkmark") }
                            else { Text(ReglasAudioPin.rotulo(v)) }
                        }
                    }
                } label: {
                    Text(ReglasAudioPin.rotulo(velocidad)).font(.caption.weight(.semibold))
                }
                Spacer(minLength: 0)
                Text(ReglasAudioPin.reloj(duracion)).monospacedDigit()
            }
            .font(.caption)
            .buttonStyle(.plain)
        }
        .foregroundStyle(Paleta.v2.texto)
        .padding(.horizontal, 12)
        .padding(.bottom, 8)
        .background(Paleta.v2.tarjeta)
        .onReceive(reloj) { _ in leerTiempo() }
        .onReceive(NotificationCenter.default.publisher(for: AVPlayerItem.didPlayToEndTimeNotification)) { aviso in
            guard (aviso.object as? AVPlayerItem) === reproductor.currentItem else { return }
            if let cola = reproductor as? AVQueuePlayer {
                // La fila: sigue sola con la siguiente; tras la ultima, el
                // reproductor flotante se va (con la barra llena un momento).
                if cola.items().count > 1 {
                    pin.pistaActual += 1
                    tiempo = 0
                    return
                }
                enMarcha = false
                if pin.cierraAlAcabar {
                    _ = Task<Void, Never> { @MainActor in
                        try? await Task.sleep(for: .milliseconds(700))
                        GestorPinesIPad.compartido.cerrar(pin)
                    }
                }
                return
            }
            enMarcha = false
            reproductor.seek(to: .zero)
        }
        .onAppear {
            if pin.sonarAlAbrir {
                pin.sonarAlAbrir = false
                if !enMarcha { alternar() }
            }
        }
        .onChange(of: pin.pistas) { _, _ in
            // Una fila nueva (otro pedido) ya suena.
            enMarcha = reproductor.rate != 0 || reproductor.timeControlStatus != .paused
            tiempo = 0
        }
    }

    private func leerTiempo() {
        guard !buscando else { return }
        let t = reproductor.currentTime().seconds
        if t.isFinite { tiempo = t }
        if let d = reproductor.currentItem?.duration.seconds, d.isFinite { duracion = d }
    }

    private func saltar(_ segundos: Double) {
        let destino = ReglasAudioPin.saltar(tiempo, segundos, duracion: duracion)
        tiempo = destino
        reproductor.seek(to: CMTime(seconds: destino, preferredTimescale: 600))
    }

    private func ponerVelocidad(_ v: Float) {
        velocidad = v
        if enMarcha { reproductor.rate = v }
    }

    private var video: some View {
        ZStack(alignment: .bottomLeading) {
            if soloAudio {
                Paleta.v2.tarjeta
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
            if velocidad != 1 { reproductor.rate = velocidad }
        }
        enMarcha.toggle()
    }
}

/// Las cuentas de la barra del pin de audio (puras, para las pruebas).
enum ReglasAudioPin {
    static let velocidades: [Float] = [0.75, 1, 1.25, 1.5, 2]

    /// `m:ss`, o `h:mm:ss` desde una hora.
    static func reloj(_ segundos: Double) -> String {
        guard segundos.isFinite, segundos > 0 else { return "0:00" }
        let t = Int(segundos.rounded(.down))
        let h = t / 3600, m = (t % 3600) / 60, s = t % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }

    /// Saltar `paso` segundos sin salirse (sin duracion conocida, sin tope).
    static func saltar(_ tiempo: Double, _ paso: Double, duracion: Double) -> Double {
        let nuevo = max(0, tiempo + paso)
        return duracion > 0 ? min(nuevo, duracion) : nuevo
    }

    /// «2 de 3 · clase.m4a»: por donde va la fila del reproductor.
    static func fila(_ actual: Int, de total: Int, nombre: String) -> String {
        let n = min(max(actual, 0), max(total - 1, 0)) + 1
        return nombre.isEmpty ? "\(n) de \(total)" : "\(n) de \(total) · \(nombre)"
    }

    /// «1×», «1,5×».
    static func rotulo(_ v: Float) -> String {
        let texto = v == v.rounded() ? String(Int(v)) : String(format: "%g", Double(v)).replacingOccurrences(of: ".", with: ",")
        return texto + "×"
    }
}
