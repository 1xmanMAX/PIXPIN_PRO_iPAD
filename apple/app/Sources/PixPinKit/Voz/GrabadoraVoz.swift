// Grabar una nota de voz con el microfono (AVAudioEngine) y mandarla al
// chat abierto: un toque empieza, otro para (el `GrabadoraActivity` del
// movil y el microfono flotante de Windows).
//
// Al parar: se cierra el `.m4a` (en una carpeta temporal, nunca a medias
// dentro del proyecto), se guarda en el chat como `VOZ` y se pasa a texto en
// el aparato. Una a la vez en toda la app.
//
// Rendimiento (§4bis): el motor de sonido solo existe mientras se graba; el
// grifo escribe cada bloque al codificador AAC del sistema sin guardar el
// sonido en memoria; los picos son 256 enteros como mucho. Nada de
// temporizadores: lo que se ensena del tiempo lo pinta un `TimelineView`
// solo mientras se graba.

import AVFoundation
import Foundation
import PixPinCore

@MainActor
public final class GrabadoraVoz: ObservableObject {
    public static let compartida = GrabadoraVoz()

    public enum Estado: Equatable {
        case quieta
        case grabando(chat: String, desde: Date)
        case guardando
        case transcribiendo
    }

    @Published public private(set) var estado: Estado = .quieta
    /// Una vez: la nota recien guardada (`nil` si no se guardo), antes de
    /// pasarla a texto. El microfono flotante ofrece con ella la llamada.
    public var alGuardar: (@MainActor (MensajeChat?) -> Void)?

    private var motor: AVAudioEngine?
    private var escritor: EscritorVoz?

    public init() {}

    /// Si esta grabando en `chat`.
    public func grabando(en chat: String) -> Bool {
        if case let .grabando(c, _) = estado { return c == chat }
        return false
    }

    /// Empieza o para, segun como este.
    public func alternar(en chat: String, almacen: Almacen) {
        switch estado {
        case .quieta:
            Task { await empezar(en: chat, almacen: almacen) }
        case .grabando:
            Task { await parar(almacen: almacen) }
        case .guardando, .transcribiendo:
            break
        }
    }

    /// Pide el microfono (la primera vez sale el aviso del sistema) y
    /// empieza a grabar.
    public func empezar(en chat: String, almacen: Almacen) async {
        guard estado == .quieta else { return }
        guard await GrabadoraVoz.permisoMicrofono() else {
            almacen.ponerError("PixPin no tiene permiso para el micrófono (Ajustes > Privacidad y seguridad > Micrófono).")
            return
        }
        ReproductorVoz.compartido.parar()
        // iOS: la sesion de sonido para grabar (si no, el motor no tiene
        // entrada). Al acabar se vuelve a la de escuchar.
        do {
            let sesion = AVAudioSession.sharedInstance()
            try sesion.setCategory(.playAndRecord, mode: .default, options: [.defaultToSpeaker, .allowBluetooth])
            try sesion.setActive(true)
        } catch {
            almacen.ponerError("No se pudo abrir el micrófono: \(error.localizedDescription)")
            return
        }
        let motor = AVAudioEngine()
        let entrada = motor.inputNode
        let formato = entrada.outputFormat(forBus: 0)
        guard formato.sampleRate > 0, formato.channelCount > 0 else {
            almacen.ponerError("No hay ningún micrófono.")
            return
        }
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-voz-\(UUID().uuidString).\(FormatoVoz.extensionFichero)")
        do {
            let escritor = try EscritorVoz(url: url, muestreoDeEntrada: formato.sampleRate)
            GrabadoraVoz.instalarGrifo(en: entrada, formato: formato, escritor: escritor)
            motor.prepare()
            try motor.start()
            self.motor = motor
            self.escritor = escritor
            estado = .grabando(chat: chat, desde: Date())
        } catch {
            entrada.removeTap(onBus: 0)
            motor.stop()
            try? FileManager.default.removeItem(at: url)
            almacen.ponerError("No se pudo grabar: \(error.localizedDescription)")
        }
    }

    /// Para, guarda la nota en su chat y la pasa a texto.
    public func parar(almacen: Almacen) async {
        guard case let .grabando(chat, _) = estado, let motor, let escritor else { return }
        motor.inputNode.removeTap(onBus: 0)
        motor.stop()
        self.motor = nil
        self.escritor = nil
        GrabadoraVoz.soltarMicrofono()
        estado = .guardando
        defer { estado = .quieta }

        // Cerrar el fichero escribe el indice del MPEG-4: fuera del principal.
        let nota = await Task.detached(priority: .userInitiated) { escritor.terminar() }.value
        defer { try? FileManager.default.removeItem(at: nota.url) }
        let avisar = alGuardar
        alGuardar = nil
        guard nota.duracionMs >= FormatoVoz.minimoMs else {
            // Un resbalon en el boton, no una nota (`Voz.MINIMO_MS`).
            avisar?(nil)
            return
        }
        let mensaje = await almacen.adjuntarVoz(nota, en: chat)
        avisar?(mensaje)
        guard let mensaje else { return }
        estado = .transcribiendo
        await almacen.transcribir(mensaje, en: chat)
    }

    /// Para sin guardar nada (al cerrar la ventana del chat, por ejemplo).
    public func cancelar() {
        guard case .grabando = estado, let motor, let escritor else { return }
        motor.inputNode.removeTap(onBus: 0)
        motor.stop()
        self.motor = nil
        self.escritor = nil
        GrabadoraVoz.soltarMicrofono()
        estado = .quieta
        let url = escritor.terminar().url
        try? FileManager.default.removeItem(at: url)
    }

    /// El grifo del microfono, fuera del actor principal: AVAudioEngine lo
    /// llama en su propio hilo con cada bloque.
    private nonisolated static func instalarGrifo(en nodo: AVAudioInputNode, formato: AVAudioFormat,
                                                   escritor: EscritorVoz) {
        nodo.installTap(onBus: 0, bufferSize: 4096, format: formato) { bloque, _ in
            try? escritor.escribir(bloque)
        }
    }

    /// Suelta el microfono: la sesion vuelve a la de escuchar (iOS).
    private static func soltarMicrofono() {
        let sesion = AVAudioSession.sharedInstance()
        try? sesion.setActive(false, options: .notifyOthersOnDeactivation)
        try? sesion.setCategory(.playback, mode: .spokenAudio)
    }

    /// El permiso del microfono (el aviso del sistema solo la primera vez).
    nonisolated static func permisoMicrofono() async -> Bool {
        switch AVAudioApplication.shared.recordPermission {
        case .granted: return true
        case .undetermined: return await AVAudioApplication.requestRecordPermission()
        default: return false
        }
    }
}
