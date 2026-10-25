// Escribe el `.m4a` de una nota de voz a partir de los bloques de sonido
// que llegan (del microfono o, en las pruebas, sintetizados): los pasa a
// mono al muestreo del fichero, los codifica en AAC con AVFoundation (el
// codificador del sistema, por hardware donde lo hay) y saca a la vez los
// picos de la onda y la duracion, contando muestras.
//
// No sabe nada del microfono ni de la interfaz, para poder probarlo sin
// permisos (`VozTests`). Se usa desde UN hilo cada vez (el del grifo del
// motor de sonido mientras graba, y despues quien lo cierra); el cerrojo es
// por si acaso, no para repartir trabajo.

import AVFoundation
import Foundation

/// Lo que queda de una grabacion: justo lo que va al mensaje `VOZ`.
public struct NotaGrabada: Sendable {
    public var url: URL
    public var duracionMs: Int64
    public var picos: [Int32]
}

public enum ErrorVoz: Error, Equatable {
    case sinMicrofono
    case sinPermisoMicrofono
    case sinPermisoTranscribir
    /// No hay reconocedor en el aparato para el idioma (sin enviar a
    /// internet no se transcribe).
    case sinReconocedor
    case formato(String)
    case muyCorta
}

public final class EscritorVoz {
    public let url: URL
    /// El muestreo del fichero (y del flujo mono que se escribe).
    public let muestreo: Double

    private var fichero: AVAudioFile?
    private let formatoMono: AVAudioFormat
    private var conversor: AVAudioConverter?
    private var entradaDelConversor: AVAudioFormat?
    private var reparto: RepartoPicos
    private var muestras: Int64 = 0
    private let cerrojo = NSLock()

    /// Crea el fichero en `url` (lo pisa si ya existe) para un flujo que
    /// llega a `muestreoDeEntrada` hercios.
    public init(url: URL, muestreoDeEntrada: Double) throws {
        let hz = FormatoVoz.muestreo(deLaTarjeta: muestreoDeEntrada)
        try? FileManager.default.removeItem(at: url)
        let f: AVAudioFile
        do {
            f = try AVAudioFile(forWriting: url, settings: FormatoVoz.ajustes(muestreo: hz),
                                commonFormat: .pcmFormatFloat32, interleaved: false)
        } catch {
            // El codificador no admite 32 kbps a este muestreo: que elija el.
            try? FileManager.default.removeItem(at: url)
            f = try AVAudioFile(forWriting: url, settings: FormatoVoz.ajustes(muestreo: hz, conBitrate: false),
                                commonFormat: .pcmFormatFloat32, interleaved: false)
        }
        self.url = url
        muestreo = hz
        fichero = f
        formatoMono = f.processingFormat
        reparto = RepartoPicos(muestreo: hz)
    }

    /// Muestras mono escritas hasta ahora.
    public var muestrasEscritas: Int64 {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return muestras
    }

    /// Cuanto lleva grabado, en milisegundos.
    public var llevadoMs: Int64 { FormatoVoz.duracionMs(muestras: muestrasEscritas, muestreo: muestreo) }

    /// Escribe un bloque en cualquier formato PCM: lo pasa a mono al
    /// muestreo del fichero si hace falta.
    public func escribir(_ bloque: AVAudioPCMBuffer) throws {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        guard let fichero, bloque.frameLength > 0 else { return }
        guard let mono = try monoDe(bloque), mono.frameLength > 0 else { return }
        if let datos = mono.floatChannelData {
            reparto.empuja(UnsafeBufferPointer(start: datos[0], count: Int(mono.frameLength)))
        }
        try fichero.write(from: mono)
        muestras += Int64(mono.frameLength)
    }

    /// Cierra el fichero (el indice del MPEG-4 se escribe al soltarlo) y
    /// devuelve lo que va al mensaje. Despues no se puede escribir mas.
    public func terminar() -> NotaGrabada {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        // `AVAudioFile` cierra y escribe el indice al liberarse: aqui se
        // suelta la unica referencia.
        fichero = nil
        conversor = nil
        let picos = reparto.terminar()
        return NotaGrabada(url: url, duracionMs: FormatoVoz.duracionMs(muestras: muestras, muestreo: muestreo),
                           picos: picos)
    }

    // MARK: - Pasar a mono

    /// El bloque en el formato del fichero (mono, `Float32`, `muestreo`).
    /// Lo normal (mismo muestreo, `Float32` sin entrelazar) se hace a mano
    /// promediando canales, sin conversor; lo demas, con `AVAudioConverter`.
    private func monoDe(_ b: AVAudioPCMBuffer) throws -> AVAudioPCMBuffer? {
        let f = b.format
        if f.sampleRate == muestreo, f.commonFormat == .pcmFormatFloat32, !f.isInterleaved,
           let origen = b.floatChannelData {
            if f.channelCount == 1, f.isEqual(formatoMono) { return b }
            guard let mono = AVAudioPCMBuffer(pcmFormat: formatoMono, frameCapacity: b.frameLength),
                  let destino = mono.floatChannelData?[0] else { return nil }
            let n = Int(b.frameLength)
            let canales = Int(f.channelCount)
            let peso = 1 / Float(max(canales, 1))
            for i in 0..<n {
                var suma: Float = 0
                for c in 0..<canales { suma += origen[c][i] }
                destino[i] = suma * peso
            }
            mono.frameLength = b.frameLength
            return mono
        }
        return try convertir(b)
    }

    private func convertir(_ b: AVAudioPCMBuffer) throws -> AVAudioPCMBuffer? {
        if conversor == nil || entradaDelConversor?.isEqual(b.format) != true {
            guard let c = AVAudioConverter(from: b.format, to: formatoMono) else {
                throw ErrorVoz.formato("no se puede convertir \(b.format) a \(formatoMono)")
            }
            c.downmix = true
            conversor = c
            entradaDelConversor = b.format
        }
        guard let conversor else { return nil }
        let razon = formatoMono.sampleRate / b.format.sampleRate
        let capacidad = AVAudioFrameCount((Double(b.frameLength) * razon).rounded(.up)) + 32
        guard let salida = AVAudioPCMBuffer(pcmFormat: formatoMono, frameCapacity: capacidad) else { return nil }
        var dado = false
        var fallo: NSError?
        let estado = conversor.convert(to: salida, error: &fallo) { _, situacion in
            if dado {
                situacion.pointee = .noDataNow
                return nil
            }
            dado = true
            situacion.pointee = .haveData
            return b
        }
        if estado == .error {
            throw ErrorVoz.formato(fallo?.localizedDescription ?? "fallo al convertir")
        }
        return salida
    }
}
