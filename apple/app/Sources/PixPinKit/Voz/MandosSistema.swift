// **Los mandos de reproductor del Mac**: lo que suena sale en el Centro de
// control y en la barra de menus («Está sonando»), y las teclas multimedia
// del teclado (⏯ ⏭ ⏮) lo manejan. Es `pixpin_shell::reproductor_sistema` de
// Windows (los mandos del recuadro del volumen) con `MPRemoteCommandCenter`.
//
// Hay **un dueno a la vez**, como en Windows hay un solo sonido: la voz que
// lee un documento (`LeerEnVoz`: ⏭ ⏮ son parrafos) o el reproductor de las
// notas de voz y la musica (`ReproductorVoz`: ±10 s, ir a un punto y la
// velocidad). Quien empieza a sonar los toma; al soltarlos se apagan.

import Foundation
import MediaPlayer

/// Quien responde a los mandos del Mac.
@MainActor
protocol DuenoDeLosMandos: AnyObject {
    func mandoTocar()
    func mandoPausar()
    func mandoAlternar()
    func mandoParar()
    /// ⏭ (+1) y ⏮ (−1).
    func mandoPaso(_ sentido: Int)
    /// ±segundos (los saltos de 10 s del reproductor).
    func mandoSaltar(segundos: Double)
    func mandoIrA(segundos: Double)
}

/// Lo que se ensena de lo que suena.
struct LoQueSuena: Equatable {
    var titulo: String
    var subtitulo: String = ""
    var duracion: Double = 0
    var posicion: Double = 0
    var velocidad: Float = 1
    var sonando: Bool
}

@MainActor
final class MandosDelSistema {
    static let compartidos = MandosDelSistema()

    private weak var dueno: DuenoDeLosMandos?
    private var enganchados = false
    /// Las pruebas no ponen los mandos en el Mac de quien las pasa (como
    /// Windows con `cfg!(test)`).
    var apagados = ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
        || ProcessInfo.processInfo.processName.hasSuffix("Tests")

    /// De quien son ahora (para las pruebas y para soltar solo lo propio).
    var deQuien: DuenoDeLosMandos? { dueno }

    /// **Los toma `d`**. `porPasos`: ⏭ ⏮ van de parrafo en parrafo (la voz
    /// que lee); si no, son saltos de 10 s y se puede ir a un punto.
    func tomar(_ d: DuenoDeLosMandos, porPasos: Bool) {
        dueno = d
        guard !apagados else { return }
        enganchar()
        let c = MPRemoteCommandCenter.shared()
        c.nextTrackCommand.isEnabled = porPasos
        c.previousTrackCommand.isEnabled = porPasos
        c.skipForwardCommand.isEnabled = !porPasos
        c.skipBackwardCommand.isEnabled = !porPasos
        c.changePlaybackPositionCommand.isEnabled = !porPasos
        for m in [c.playCommand, c.pauseCommand, c.togglePlayPauseCommand, c.stopCommand] { m.isEnabled = true }
    }

    /// Los suelta `d` (si eran suyos): fuera de la barra de menus.
    func soltar(_ d: DuenoDeLosMandos) {
        guard dueno === d else { return }
        dueno = nil
        guard !apagados else { return }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        MPNowPlayingInfoCenter.default().playbackState = .stopped
    }

    /// Lo que suena, al dia (solo si es del dueno).
    func poner(_ s: LoQueSuena, de d: DuenoDeLosMandos) {
        guard dueno === d, !apagados else { return }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: s.titulo,
            MPNowPlayingInfoPropertyPlaybackRate: s.sonando ? Double(s.velocidad) : 0.0,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: 1.0,
        ]
        if !s.subtitulo.isEmpty { info[MPMediaItemPropertyArtist] = s.subtitulo }
        if s.duracion > 0 {
            info[MPMediaItemPropertyPlaybackDuration] = s.duracion
            info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = s.posicion
        }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
        MPNowPlayingInfoCenter.default().playbackState = s.sonando ? .playing : .paused
    }

    /// Los manejadores, una vez: mandan al dueno de ese momento.
    private func enganchar() {
        guard !enganchados else { return }
        enganchados = true
        let c = MPRemoteCommandCenter.shared()
        func con(_ m: MPRemoteCommand, _ hacer: @escaping @MainActor (DuenoDeLosMandos, MPRemoteCommandEvent) -> Void) {
            // El sistema no promete en que hilo llama: se pasa al principal.
            nonisolated(unsafe) let hacer = hacer
            m.addTarget { [weak self] e in
                nonisolated(unsafe) let e = e
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        if let d = self?.dueno { hacer(d, e) }
                    }
                }
                return .success
            }
        }
        con(c.playCommand) { d, _ in d.mandoTocar() }
        con(c.pauseCommand) { d, _ in d.mandoPausar() }
        con(c.togglePlayPauseCommand) { d, _ in d.mandoAlternar() }
        con(c.stopCommand) { d, _ in d.mandoParar() }
        con(c.nextTrackCommand) { d, _ in d.mandoPaso(1) }
        con(c.previousTrackCommand) { d, _ in d.mandoPaso(-1) }
        c.skipForwardCommand.preferredIntervals = [10]
        c.skipBackwardCommand.preferredIntervals = [10]
        con(c.skipForwardCommand) { d, e in d.mandoSaltar(segundos: (e as? MPSkipIntervalCommandEvent)?.interval ?? 10) }
        con(c.skipBackwardCommand) { d, e in d.mandoSaltar(segundos: -((e as? MPSkipIntervalCommandEvent)?.interval ?? 10)) }
        con(c.changePlaybackPositionCommand) { d, e in
            if let e = e as? MPChangePlaybackPositionCommandEvent { d.mandoIrA(segundos: e.positionTime) }
        }
    }
}
