// **El sonido por el motor de audio de Windows** (38204cb y 06f842c): las
// notas de voz, el microfono de las ventanitas y «Grabar clase» graban con
// la misma mezcla y el mismo tratamiento que Windows (`MezclaDeAudio` del
// nucleo, que es `pixpin-motor-audio`): remuestreo con la deriva de cada
// tarjeta, paso alto de 80 Hz y realce de la voz en el microfono, y el
// limitador del final. Asi una nota del Mac suena como una del PC.
//
// - El sistema da los bloques (el grifo de AVAudioEngine, ScreenCaptureKit)
//   y cada uno cruza al nucleo **entero**, nunca muestra a muestra
//   (`MezclaVoz.empujar`).
// - Una cola con un temporizador de 20 ms pide lo mezclado
//   (`MezclaDeAudio.mezclar`): sale lo que marca el reloj de la pared, y eso
//   es lo que se escribe en el `.m4a` (`EscritorVoz`, mono a 48 kHz).
//
// Rendimiento (§4bis): nada se guarda en memoria mas alla de los anillos
// del nucleo (dos segundos por fuente); el motor de sonido solo existe
// mientras se graba.

import AVFoundation
import Foundation
import PixPinCore

/// Pasa bloques de cualquier formato PCM a mono `Float32` a `muestreo`: el
/// ultimo recurso para un bloque que no llega en `Float32` (lo normal en el
/// Mac es que si). Lo usa un solo hilo cada vez.
final class AMono {
    let formato: AVAudioFormat
    private var conversor: AVAudioConverter?
    private var entrada: AVAudioFormat?

    init?(muestreo: Double) {
        guard let f = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: muestreo, channels: 1,
                                    interleaved: false) else { return nil }
        formato = f
    }

    func muestras(_ b: AVAudioPCMBuffer) -> [Float] {
        guard b.frameLength > 0 else { return [] }
        let f = b.format
        if conversor == nil || entrada?.isEqual(f) != true {
            conversor = AVAudioConverter(from: f, to: formato)
            conversor?.downmix = true
            entrada = f
        }
        guard let conversor else { return [] }
        let capacidad = AVAudioFrameCount((Double(b.frameLength) * formato.sampleRate / f.sampleRate).rounded(.up)) + 32
        guard let salida = AVAudioPCMBuffer(pcmFormat: formato, frameCapacity: capacidad) else { return [] }
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
        guard estado != .error, let d = salida.floatChannelData else { return [] }
        return Array(UnsafeBufferPointer(start: d[0], count: Int(salida.frameLength)))
    }
}

/// Lo comun de grabar por la mezcla del nucleo.
enum MezclaVoz {
    /// El muestreo de lo que se escribe: el de Windows (`Salida::default`).
    static let muestreo: Double = 48_000

    /// **Un bloque del sistema a la fuente `fuente`**, tal cual: en
    /// `Float32` (lo normal) cruza con sus canales, separados o
    /// intercalados; en otro formato se pasa antes a mono con `alterno` (que
    /// va al muestreo de esa fuente).
    static func empujar(_ b: AVAudioPCMBuffer, fuente: UInt32, en mezcla: MezclaDeAudio, alterno: AMono?) {
        let n = Int(b.frameLength)
        guard n > 0 else { return }
        let f = b.format
        let canales = Int(f.channelCount)
        if f.commonFormat == .pcmFormatFloat32, canales > 0, let datos = b.floatChannelData {
            if f.isInterleaved || canales == 1 {
                let v = Array(UnsafeBufferPointer(start: datos[0], count: n * canales))
                _ = mezcla.empujar(fuente: fuente, muestras: v, canales: UInt16(canales), separadas: false)
            } else {
                var v = [Float]()
                v.reserveCapacity(n * canales)
                for c in 0..<canales { v.append(contentsOf: UnsafeBufferPointer(start: datos[c], count: n)) }
                _ = mezcla.empujar(fuente: fuente, muestras: v, canales: UInt16(canales), separadas: true)
            }
            return
        }
        guard let alterno else { return }
        let v = alterno.muestras(b)
        guard !v.isEmpty else { return }
        _ = mezcla.empujar(fuente: fuente, muestras: v, canales: 1, separadas: false)
    }

    /// Lo mezclado como bloque mono `Float32` a `formato` (el de la nota).
    static func bloque(_ v: [Float], formato: AVAudioFormat) -> AVAudioPCMBuffer? {
        guard !v.isEmpty, let b = AVAudioPCMBuffer(pcmFormat: formato, frameCapacity: AVAudioFrameCount(v.count)),
              let d = b.floatChannelData else { return nil }
        v.withUnsafeBufferPointer { p in
            if let base = p.baseAddress { d[0].update(from: base, count: v.count) }
        }
        b.frameLength = AVAudioFrameCount(v.count)
        return b
    }
}

/// **El microfono que graba un `.m4a` por la mezcla** (la `Grabadora` de
/// Windows con `Opciones::nota_de_voz`): lo usan `GrabadoraVoz` y
/// `MicrofonoSuelto`. Lo suyo pasa en `cola`.
final class MicrofonoConMezcla: @unchecked Sendable {
    let escritor: EscritorVoz
    private let motor: AVAudioEngine
    private let mezcla: MezclaDeAudio
    private let alterno: AMono?
    private let formatoNota: AVAudioFormat
    private let cola = DispatchQueue(label: "PixPin microfono", qos: .userInitiated)
    private var tic: DispatchSourceTimer?
    /// En `cola`: ya se saco lo ultimo.
    private var cerrado = false

    /// Abre el microfono de por defecto para grabar en `url` (aun no
    /// graba). Lanza si no hay microfono.
    init(url: URL) throws {
        let motor = AVAudioEngine()
        let formato = motor.inputNode.outputFormat(forBus: 0)
        guard formato.sampleRate > 0, formato.channelCount > 0,
              let nota = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: MezclaVoz.muestreo, channels: 1,
                                       interleaved: false) else { throw ErrorVoz.sinMicrofono }
        let fuente = FuenteDeMezcla(papel: .microfono, muestreo: UInt32(formato.sampleRate.rounded()),
                                    canales: UInt16(formato.channelCount))
        let mezcla: MezclaDeAudio
        do {
            mezcla = try MezclaDeAudio.nueva(fuentes: [fuente], muestreo: UInt32(MezclaVoz.muestreo))
        } catch {
            throw ErrorVoz.formato("\(error)")
        }
        self.motor = motor
        self.mezcla = mezcla
        alterno = AMono(muestreo: formato.sampleRate)
        formatoNota = nota
        escritor = try EscritorVoz(url: url, muestreoDeEntrada: MezclaVoz.muestreo)
    }

    /// Lo llevado, sin las pausas (lo que de verdad se grabo).
    var llevadoMs: Int64 { mezcla.llevadoMs() }

    /// El pico de lo ultimo que entro, de 0 a 1.
    var nivel: Float { mezcla.nivel() }

    /// Empieza a grabar.
    func empezar() throws {
        let entrada = motor.inputNode
        let formato = entrada.outputFormat(forBus: 0)
        let mezcla = self.mezcla
        let alterno = self.alterno
        // El grifo corre en su hilo: cada bloque cruza entero al nucleo.
        entrada.installTap(onBus: 0, bufferSize: 4096, format: formato) { bloque, _ in
            MezclaVoz.empujar(bloque, fuente: 0, en: mezcla, alterno: alterno)
        }
        motor.prepare()
        do {
            try motor.start()
        } catch {
            entrada.removeTap(onBus: 0)
            throw ErrorVoz.sinMicrofono
        }
        // El cero: lo que el microfono dio mientras se abria no cuenta.
        cola.sync { _ = mezcla.mezclar() }
        let periodo = Int(periodoDeLaMezclaMs())
        let t = DispatchSource.makeTimerSource(queue: cola)
        t.schedule(deadline: .now() + .milliseconds(periodo), repeating: .milliseconds(periodo))
        t.setEventHandler { [weak self] in self?.vuelta() }
        t.resume()
        tic = t
    }

    /// Pausa o sigue: en pausa no se graba nada y el reloj no corre.
    func pausar(_ si: Bool) {
        mezcla.pausar(pausa: si)
    }

    /// (iPad) Vuelve a arrancar el motor de sonido tras una interrupcion
    /// (una llamada, Siri): el grifo sigue puesto. `false` si no volvio.
    func volverAArrancar() -> Bool {
        guard !motor.isRunning else { return true }
        do {
            try motor.start()
            return true
        } catch {
            return false
        }
    }

    private func vuelta() {
        guard !cerrado else { return }
        escribir(mezcla.mezclar())
    }

    private func escribir(_ b: BloqueMezclado) {
        guard let pcm = MezclaVoz.bloque(b.muestras, formato: formatoNota) else { return }
        try? escritor.escribir(pcm)
    }

    /// **Para y cierra el `.m4a`** con lo que quedaba. Bloquea (escribir el
    /// indice del MPEG-4 tarda): mejor fuera del hilo principal.
    func terminar() -> NotaGrabada {
        motor.inputNode.removeTap(onBus: 0)
        motor.stop()
        return cola.sync {
            tic?.cancel()
            tic = nil
            if !cerrado {
                cerrado = true
                escribir(mezcla.terminar())
            }
            return escritor.terminar()
        }
    }
}
