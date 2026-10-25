// **El microfono del asistente**: lo que se dice va a la caja de escribir
// mientras se habla (`SFSpeechRecognizer` en el aparato, sin red: lo mismo
// que las notas de voz, `TranscriptorVoz`). Al soltarlo, con «Ver lo dicho
// antes de enviarlo» (`[asistente] revisar_lo_dicho`) se queda en la caja
// para corregirlo; sin el, se envia solo (Windows).

import AVFoundation
import Foundation
import Speech

@MainActor
final class DictadoAsistente: ObservableObject {
    @Published private(set) var escuchando = false
    @Published private(set) var aviso: String?

    private var motor: AVAudioEngine?
    private var pedido: SFSpeechAudioBufferRecognitionRequest?
    private var tarea: SFSpeechRecognitionTask?
    /// Lo que habia en la caja al empezar (lo dicho va detras).
    private var antes = ""
    private var alAcabar: ((String) -> Void)?

    /// Empieza a escuchar; `alTexto` recibe la caja entera cada vez que se
    /// entiende algo mas, `alAcabar` el texto final.
    func empezar(caja: String, alTexto: @escaping @MainActor (String) -> Void,
                 alAcabar: @escaping @MainActor (String) -> Void) async {
        guard !escuchando else { return }
        aviso = nil
        guard await GrabadoraVoz.permisoMicrofono() else {
            aviso = "Falta el permiso del micrófono (Ajustes del Sistema → Privacidad)."
            return
        }
        guard await TranscriptorVoz.autorizar(), let reconocedor = TranscriptorVoz.reconocedorLocal() else {
            aviso = "Este Mac no puede pasar la voz a texto sin internet (o falta el permiso)."
            return
        }
        antes = caja.isEmpty ? "" : caja + " "
        self.alAcabar = alAcabar
        let pedido = SFSpeechAudioBufferRecognitionRequest()
        pedido.requiresOnDeviceRecognition = true
        pedido.shouldReportPartialResults = true
        pedido.addsPunctuation = true
        pedido.taskHint = .dictation
        let motor = AVAudioEngine()
        let nodo = motor.inputNode
        DictadoAsistente.instalarGrifo(en: nodo, pedido: pedido)
        do {
            motor.prepare()
            try motor.start()
        } catch {
            nodo.removeTap(onBus: 0)
            aviso = "No se pudo abrir el micrófono."
            return
        }
        self.motor = motor
        self.pedido = pedido
        escuchando = true
        let antes = self.antes
        tarea = reconocedor.recognitionTask(with: pedido) { [weak self] r, error in
            let texto = r?.bestTranscription.formattedString
            let final = (r?.isFinal ?? false) || error != nil
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self else { return }
                    if let texto { alTexto(antes + texto) }
                    if final { self.acabar(antes + (texto ?? "")) }
                }
            }
        }
    }

    /// Suelta el microfono: lo entendido hasta aqui es lo final.
    func parar() {
        guard escuchando else { return }
        motor?.inputNode.removeTap(onBus: 0)
        motor?.stop()
        pedido?.endAudio()
    }

    private func acabar(_ texto: String) {
        guard escuchando else { return }
        escuchando = false
        motor?.inputNode.removeTap(onBus: 0)
        motor?.stop()
        motor = nil
        pedido = nil
        tarea = nil
        let f = alAcabar
        alAcabar = nil
        f?(texto.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    private nonisolated static func instalarGrifo(en nodo: AVAudioInputNode,
                                                   pedido: SFSpeechAudioBufferRecognitionRequest) {
        nodo.installTap(onBus: 0, bufferSize: 2048, format: nodo.outputFormat(forBus: 0)) { bloque, _ in
            pedido.append(bloque)
        }
    }
}
