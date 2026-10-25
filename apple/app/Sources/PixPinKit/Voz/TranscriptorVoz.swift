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
    /// Lo reconocido de una nota y lo seguro que estaba el reconocedor.
    struct Reconocido: Sendable {
        var segmentos: [SegmentoVoz]
        var confianza: Double
    }

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
        segmentos(trozos.map { (desde: $0.desde, dura: 0, texto: $0.texto) })
    }

    /// Con lo que dura cada trozo: Rust lo usa para partir las lineas de
    /// 15 s tras un punto o un silencio, como Windows (7df5532). Sin
    /// duracion (0), el trozo cuenta como un instante.
    public static func segmentos(_ trozos: [(desde: TimeInterval, dura: TimeInterval, texto: String)]) -> [SegmentoVoz] {
        trozos.compactMap { t in
            let texto = t.texto.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !texto.isEmpty else { return nil }
            let desde = Int64((max(t.desde, 0) * 1000).rounded())
            let hasta = t.dura > 0 ? Int64((max(t.desde + t.dura, 0) * 1000).rounded()) : 0
            return SegmentoVoz(desdeMs: desde, texto: texto, hastaMs: hasta)
        }
    }

    /// Transcribe el audio de `url` en el aparato. Lanza si falta permiso o
    /// reconocedor; si no se oye a nadie, devuelve una lista vacia (y la
    /// nota queda con estado `mal`, como en Windows).
    public static func transcribir(_ url: URL, idioma: Locale? = nil) async throws -> [SegmentoVoz] {
        try await transcribirConConfianza(url, idioma: idioma).segmentos
    }

    /// Lo mismo, con lo seguro que esta el reconocedor (0 a 1, la media de
    /// sus palabras): con dos idiomas se queda el que mejor entendio.
    static func transcribirConConfianza(_ url: URL, idioma: Locale? = nil) async throws -> Reconocido {
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
            try await withCheckedThrowingContinuation { (c: CheckedContinuation<Reconocido, Error>) in
                unaVez.poner(c)
                let tarea = reconocedor.recognitionTask(with: pedido) { resultado, error in
                    // El reconocedor tiene que vivir hasta el final.
                    _ = reconocedor
                    if let resultado, resultado.isFinal {
                        let partes = resultado.bestTranscription.segments
                        // Con lo que dura cada trozo (el Mac, f3d0752) y la confianza (el iPad, dos idiomas).
                        let trozos = partes.map { (desde: $0.timestamp, dura: $0.duration, texto: $0.substring) }
                        let confianza = partes.isEmpty ? 0 : partes.map { Double($0.confidence) }.reduce(0, +) / Double(partes.count)
                        unaVez.resolver(.success(Reconocido(segmentos: segmentos(trozos), confianza: confianza)))
                    } else if let error {
                        if TranscriptorVoz.esSilencio(error) {
                            unaVez.resolver(.success(Reconocido(segmentos: [], confianza: 0)))
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
    private var continuacion: CheckedContinuation<TranscriptorVoz.Reconocido, Error>?
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

    func poner(_ c: CheckedContinuation<TranscriptorVoz.Reconocido, Error>) {
        cerrojo.lock()
        let yaCancelada = cancelada
        if !yaCancelada { continuacion = c }
        cerrojo.unlock()
        if yaCancelada { c.resume(throwing: CancellationError()) }
    }

    func resolver(_ r: Result<TranscriptorVoz.Reconocido, Error>) {
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
