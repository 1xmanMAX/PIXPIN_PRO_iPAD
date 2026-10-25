// Grabar una nota de voz con el microfono (AVAudioEngine) y mandarla al
// chat abierto: un toque empieza, otro para (el `GrabadoraActivity` del
// movil y el microfono flotante de Windows).
//
// Al parar: se cierra el `.m4a` (en una carpeta temporal, nunca a medias
// dentro del proyecto), se guarda en el chat como `VOZ` y se pasa a texto en
// el aparato. Una a la vez en toda la app.
//
// El sonido pasa por la mezcla del motor de audio de Windows
// (`MicrofonoConMezcla`, Windows 06f842c): paso alto y realce de la voz,
// como una nota grabada en el PC.
//
// Rendimiento (§4bis): el motor de sonido solo existe mientras se graba; el
// grifo da cada bloque entero al nucleo y una cola lo escribe al
// codificador AAC del sistema sin guardar el sonido en memoria; los picos
// son 256 enteros como mucho. Lo que se ensena del tiempo lo pinta un
// `TimelineView` solo mientras se graba.

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

    private var microfono: MicrofonoConMezcla?

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
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-voz-\(UUID().uuidString).\(FormatoVoz.extensionFichero)")
        let m: MicrofonoConMezcla
        do {
            m = try MicrofonoConMezcla(url: url)
        } catch ErrorVoz.sinMicrofono {
            GrabadoraVoz.soltarMicrofono()
            almacen.ponerError("No hay ningún micrófono.")
            return
        } catch {
            GrabadoraVoz.soltarMicrofono()
            try? FileManager.default.removeItem(at: url)
            almacen.ponerError("No se pudo grabar: \(error.localizedDescription)")
            return
        }
        do {
            try m.empezar()
            microfono = m
            estado = .grabando(chat: chat, desde: Date())
        } catch {
            _ = m.terminar()
            GrabadoraVoz.soltarMicrofono()
            try? FileManager.default.removeItem(at: url)
            almacen.ponerError("No se pudo grabar: \(error.localizedDescription)")
        }
    }

    /// Para, guarda la nota en su chat y la pasa a texto.
    public func parar(almacen: Almacen) async {
        guard case let .grabando(chat, _) = estado, let microfono else { return }
        self.microfono = nil
        estado = .guardando
        defer { estado = .quieta }

        // Cerrar el fichero escribe el indice del MPEG-4: fuera del principal.
        let nota = await Task.detached(priority: .userInitiated) { microfono.terminar() }.value
        GrabadoraVoz.soltarMicrofono()
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
        guard case .grabando = estado, let microfono else { return }
        self.microfono = nil
        estado = .quieta
        let url = microfono.terminar().url
        GrabadoraVoz.soltarMicrofono()
        try? FileManager.default.removeItem(at: url)
    }

    /// Suelta el microfono: la sesion vuelve a la de escuchar (iOS).
    static func soltarMicrofono() {
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
