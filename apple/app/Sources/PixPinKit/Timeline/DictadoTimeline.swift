// Dictar en el timeline (`timeline/dictar.rs` de Windows): un toque al
// microfono (o ⌘⇧M) empieza a grabar, otro termina; lo dicho se pasa a
// texto en el aparato y se apunta con su audio. Si habia algo escrito (o se
// esta corrigiendo), lo dicho se suma a la caja y el audio espera a ↩.
//
// No usa `GrabadoraVoz`: esa guarda la nota en un chat. Aqui el `.m4a` se
// queda en un temporal y el nucleo lo mueve a `timeline/medios/`.

import AVFoundation
import Foundation
import PixPinCore

@MainActor
public final class DictadoTimeline: ObservableObject {
    @Published public private(set) var grabando = false
    /// Lo que dice la caja mientras tanto («Te escucho…»).
    @Published public private(set) var rotulo: String?

    private var motor: AVAudioEngine?
    private var escritor: EscritorVoz?
    private var desde: Date?
    /// Lo dictado con algo escrito: su audio espera a ↩.
    private var pendiente: NotaGrabada?

    public init() {}

    /// Empieza o termina.
    public func pulsar(modelo: ModeloTimeline) async {
        if grabando {
            await terminar(modelo: modelo)
        } else {
            await empezar()
        }
    }

    private func empezar() async {
        guard await DictadoTimeline.permiso() else {
            rotulo = "PixPin no tiene permiso para el micrófono (Ajustes del Sistema > Privacidad y seguridad)."
            return
        }
        ReproductorVoz.compartido.parar()
        let motor = AVAudioEngine()
        let entrada = motor.inputNode
        let formato = entrada.outputFormat(forBus: 0)
        guard formato.sampleRate > 0, formato.channelCount > 0 else {
            rotulo = "No hay micrófono"
            return
        }
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-timeline-\(UUID().uuidString).\(FormatoVoz.extensionFichero)")
        do {
            let escritor = try EscritorVoz(url: url, muestreoDeEntrada: formato.sampleRate)
            DictadoTimeline.grifo(entrada, formato: formato, escritor: escritor)
            motor.prepare()
            try motor.start()
            self.motor = motor
            self.escritor = escritor
            desde = Date()
            grabando = true
            rotulo = "🎙 Te escucho… pulsa el micrófono (o ⌘⇧M) para terminar"
        } catch {
            entrada.removeTap(onBus: 0)
            motor.stop()
            try? FileManager.default.removeItem(at: url)
            rotulo = "No se pudo abrir el micrófono"
        }
    }

    private func terminar(modelo: ModeloTimeline) async {
        guard let motor, let escritor else { return }
        motor.inputNode.removeTap(onBus: 0)
        motor.stop()
        self.motor = nil
        self.escritor = nil
        grabando = false
        rotulo = "Guardando lo dicho…"
        let nota = await Task.detached(priority: .userInitiated) { escritor.terminar() }.value
        guard nota.duracionMs >= FormatoVoz.minimoMs else {
            try? FileManager.default.removeItem(at: nota.url)
            rotulo = "Demasiado corto: habla un poco más"
            return
        }
        rotulo = "Pasando a texto…"
        let texto: String
        do {
            texto = try await TranscriptorVoz.transcribir(nota.url).map(\.texto).joined(separator: " ")
        } catch {
            texto = ""
        }
        rotulo = texto.isEmpty ? "No se entendió: se guarda solo el audio" : nil
        let cuando = desde.map { Int64($0.timeIntervalSince1970 * 1000) }
        // Con algo escrito o corrigiendo, lo dicho se suma a la caja.
        if !modelo.texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || modelo.corrigiendo != nil {
            if !texto.isEmpty {
                let junta = modelo.texto.isEmpty || modelo.texto.hasSuffix(" ") || modelo.texto.hasSuffix("\n") ? "" : " "
                modelo.texto += junta + texto
            }
            if modelo.corrigiendo == nil {
                if let viejo = pendiente { try? FileManager.default.removeItem(at: viejo.url) }
                pendiente = nota
                rotulo = "🎙 La nota de voz va con lo escrito: pulsa ↩"
            } else {
                try? FileManager.default.removeItem(at: nota.url)
            }
            return
        }
        if await modelo.apuntar(texto: texto, fotos: [], audio: nota, dicho: texto.isEmpty ? nil : texto,
                                cuando: cuando) {
            modelo.avisar("Apuntado")
        }
    }

    /// La nota que esperaba a ↩ (o `nil`): la recoge quien apunta.
    public func tomarPendiente() -> NotaGrabada? {
        defer { pendiente = nil }
        if pendiente != nil { rotulo = nil }
        return pendiente
    }

    /// Para sin guardar nada (al cerrar la ventana).
    public func cancelar() {
        if let motor, let escritor {
            motor.inputNode.removeTap(onBus: 0)
            motor.stop()
            try? FileManager.default.removeItem(at: escritor.terminar().url)
        }
        motor = nil
        escritor = nil
        grabando = false
        if let p = pendiente { try? FileManager.default.removeItem(at: p.url) }
        pendiente = nil
    }

    private nonisolated static func grifo(_ nodo: AVAudioInputNode, formato: AVAudioFormat, escritor: EscritorVoz) {
        nodo.installTap(onBus: 0, bufferSize: 4096, format: formato) { bloque, _ in
            try? escritor.escribir(bloque)
        }
    }

    private nonisolated static func permiso() async -> Bool {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: return true
        case .notDetermined: return await AVCaptureDevice.requestAccess(for: .audio)
        default: return false
        }
    }
}
