// Pasar una nota de voz a texto EN EL APARATO, con `SFSpeechRecognizer`
// (`requiresOnDeviceRecognition`): sin internet, sin bajar modelos (lo que en
// Windows hace Whisper). Si el Mac no tiene reconocedor local para el idioma
// no se manda a ningun servidor: se dice y ya.
//
// El idioma es espanol de Espana (el de PixPin) y si el Mac no lo tiene, el
// del sistema. El reconocedor trabaja en su propia cola con prioridad baja
// (§4bis.3 del plan): el hilo principal no espera.
//
// Lo reconocido sale en `SegmentoVoz` (desde que milisegundo y que dice); el
// formato con minutos (`[1:23] ...`) lo pone el nucleo de Rust al guardarlo,
// igual que Windows.

import Foundation
import PixPinCore
import Speech

public enum TranscriptorVoz {
    /// El idioma preferido de las notas.
    public static let idiomaPreferido = Locale(identifier: "es-ES")

    /// Un reconocedor que funciona sin internet: el de `es-ES` y si no, el
    /// del sistema. `nil` si ninguno lo hace en el aparato.
    /// Con `idioma` (el que se practica en Pronunciar) solo vale ese: pasar
    /// ingles con el reconocedor de espanol daria otra cosa.
    public static func reconocedorLocal(idioma: Locale? = nil) -> SFSpeechRecognizer? {
        if let idioma {
            let r = SFSpeechRecognizer(locale: idioma)
            return r?.supportsOnDeviceRecognition == true ? r : nil
        }
        if let r = SFSpeechRecognizer(locale: idiomaPreferido), r.supportsOnDeviceRecognition { return r }
        if let r = SFSpeechRecognizer(), r.supportsOnDeviceRecognition { return r }
        return nil
    }

    /// Pide el permiso de reconocer voz si aun no se pidio.
    public static func autorizar() async -> Bool {
        switch SFSpeechRecognizer.authorizationStatus() {
        case .authorized: return true
        case .denied, .restricted: return false
        default: break
        }
        return await withCheckedContinuation { (c: CheckedContinuation<Bool, Never>) in
            SFSpeechRecognizer.requestAuthorization { estado in
                c.resume(returning: estado == .authorized)
            }
        }
    }

    /// Los segmentos de una transcripcion, en milisegundos desde el
    /// principio del audio. Puro: lo prueban las pruebas.
    public static func segmentos(_ trozos: [(desde: TimeInterval, texto: String)]) -> [SegmentoVoz] {
        trozos.compactMap { t in
            let texto = t.texto.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !texto.isEmpty else { return nil }
            return SegmentoVoz(desdeMs: Int64((max(t.desde, 0) * 1000).rounded()), texto: texto)
        }
    }

    /// Transcribe el audio de `url` en el aparato. Lanza si falta permiso o
    /// reconocedor; si no se oye a nadie, devuelve una lista vacia (y la
    /// nota queda con estado `mal`, como en Windows).
    public static func transcribir(_ url: URL, idioma: Locale? = nil) async throws -> [SegmentoVoz] {
        guard await autorizar() else { throw ErrorVoz.sinPermisoTranscribir }
        guard let reconocedor = reconocedorLocal(idioma: idioma) else { throw ErrorVoz.sinReconocedor }
        let cola = OperationQueue()
        cola.name = "PixPin transcribir"
        cola.qualityOfService = .utility
        cola.maxConcurrentOperationCount = 1
        reconocedor.queue = cola

        let pedido = SFSpeechURLRecognitionRequest(url: url)
        pedido.requiresOnDeviceRecognition = true
        pedido.shouldReportPartialResults = false
        pedido.addsPunctuation = true
        pedido.taskHint = .dictation

        let unaVez = UnaVezTranscripcion()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (c: CheckedContinuation<[SegmentoVoz], Error>) in
                unaVez.poner(c)
                let tarea = reconocedor.recognitionTask(with: pedido) { resultado, error in
                    // El reconocedor tiene que vivir hasta el final.
                    _ = reconocedor
                    if let resultado, resultado.isFinal {
                        let trozos = resultado.bestTranscription.segments.map { (desde: $0.timestamp, texto: $0.substring) }
                        unaVez.resolver(.success(segmentos(trozos)))
                    } else if let error {
                        if TranscriptorVoz.esSilencio(error) {
                            unaVez.resolver(.success([]))
                        } else {
                            unaVez.resolver(.failure(error))
                        }
                    }
                }
                unaVez.tarea = tarea
            }
        } onCancel: {
            unaVez.cancelar()
        }
    }

    /// «No se detecto voz» (`kAFAssistantErrorDomain` 1110): no es un fallo,
    /// es una nota sin nada que transcribir.
    static func esSilencio(_ error: Error) -> Bool {
        let e = error as NSError
        return e.domain == "kAFAssistantErrorDomain" && e.code == 1110
    }
}

/// La continuacion se resuelve una sola vez aunque el reconocedor llame
/// varias (resultado final y luego error, o cancelar a la vez).
private final class UnaVezTranscripcion: @unchecked Sendable {
    private let cerrojo = NSLock()
    private var continuacion: CheckedContinuation<[SegmentoVoz], Error>?
    private var _tarea: SFSpeechRecognitionTask?
    private var cancelada = false

    var tarea: SFSpeechRecognitionTask? {
        get { cerrojo.lock(); defer { cerrojo.unlock() }; return _tarea }
        set {
            cerrojo.lock()
            _tarea = newValue
            let yaCancelada = cancelada
            cerrojo.unlock()
            if yaCancelada { newValue?.cancel() }
        }
    }

    func poner(_ c: CheckedContinuation<[SegmentoVoz], Error>) {
        cerrojo.lock()
        let yaCancelada = cancelada
        if !yaCancelada { continuacion = c }
        cerrojo.unlock()
        if yaCancelada { c.resume(throwing: CancellationError()) }
    }

    func resolver(_ r: Result<[SegmentoVoz], Error>) {
        cerrojo.lock()
        let c = continuacion
        continuacion = nil
        _tarea = nil
        cerrojo.unlock()
        c?.resume(with: r)
    }

    func cancelar() {
        cerrojo.lock()
        cancelada = true
        let t = _tarea
        cerrojo.unlock()
        t?.cancel()
        resolver(.failure(CancellationError()))
    }
}
