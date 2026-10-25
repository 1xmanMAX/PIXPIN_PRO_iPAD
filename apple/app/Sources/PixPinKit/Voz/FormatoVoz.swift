// Las cuentas de las notas de voz que no tocan el microfono: el formato del
// fichero, los picos de la onda y como se ensena el tiempo. Puras a
// proposito: las prueba `VozTests` sin permisos.
//
// El formato es el de Windows y Android, para que la nota suene en todos:
// `.m4a` (MPEG-4) con AAC-LC MONO. El movil graba a 32 kbps (`Voz.kt:141`);
// Windows a 96 kbps porque Media Foundation no admite menos. Aqui se pide lo
// del movil (AVFoundation si lo admite), y si el codificador no lo acepta en
// ese muestreo, el que el elija. El muestreo es el de la tarjeta si el AAC
// lo admite y si no 48 kHz, como `mezcla::muestreo_de_grabacion` de Windows.
//
// Los picos son el contrato del movil al numero (`pixpin_audio::picos`):
// enteros de 0 a 32767, uno cada 50 ms, como mucho 256. Al llegar al tope se
// juntan de dos en dos y se mira uno de cada dos (asi una nota larga
// conserva su forma entera en vez de cortarse).

import AVFoundation
import Foundation

public enum FormatoVoz {
    /// La extension de las notas de voz en todos los aparatos.
    public static let extensionFichero = "m4a"
    /// Bits por segundo del AAC: los del movil.
    public static let bitsPorSegundo = 32_000
    /// Una nota mas corta que esto fue un resbalon en el boton
    /// (`Voz.MINIMO_MS`): no se guarda.
    public static let minimoMs: Int64 = 700
    /// Cada cuanto se anota un pico (`MS_ENTRE_PICOS`).
    public static let msEntrePicos = 50
    /// Cuantos picos se guardan como mucho (`PICOS_GUARDADOS`).
    public static let picosGuardados = 256
    /// Lo mas alto que vale un pico (una muestra de 16 bits).
    public static let picoMaximo: Int32 = 32767
    /// Los muestreos que admite el AAC de Windows y de Apple
    /// (`MUESTREOS_QUE_ADMITE_AAC`).
    public static let muestreosAAC: [Double] = [44_100, 48_000]

    /// El muestreo del fichero para una tarjeta que entrega `deLaTarjeta`.
    public static func muestreo(deLaTarjeta hz: Double) -> Double {
        muestreosAAC.contains(hz) ? hz : 48_000
    }

    /// Los ajustes de `AVAudioFile` para escribir el `.m4a`. Sin
    /// `conBitrate`, el codificador elige (por si no admite 32 kbps en ese
    /// muestreo).
    public static func ajustes(muestreo: Double, conBitrate: Bool = true) -> [String: Any] {
        var a: [String: Any] = [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: muestreo,
            AVNumberOfChannelsKey: 1,
        ]
        if conBitrate { a[AVEncoderBitRateKey] = bitsPorSegundo }
        return a
    }

    /// Cuanto duran `muestras` muestras mono, en milisegundos, hacia abajo
    /// (`mezcla::duracion_ms`: una nota de 6 999 ms se rotula `0:06` en
    /// todos los aparatos).
    public static func duracionMs(muestras: Int64, muestreo: Double) -> Int64 {
        guard muestreo > 0 else { return 0 }
        return Int64((Double(muestras) * 1000 / muestreo).rounded(.down))
    }

    /// `1:23`, `12:05`, `1:02:09`, como `marca_de_tiempo` (sin cero
    /// delante en la primera cifra).
    public static func marca(ms: Int64) -> String {
        let s = max(ms, 0) / 1000
        let h = s / 3600
        let m = (s % 3600) / 60
        let seg = s % 60
        return h > 0 ? String(format: "%lld:%02lld:%02lld", h, m, seg) : String(format: "%lld:%02lld", m, seg)
    }

    /// Una muestra en coma flotante como pico de 16 bits (0...32767).
    public static func pico(_ muestra: Float) -> Int32 {
        let a = min(abs(muestra), 1)
        return min(Int32((a * Float(picoMaximo)).rounded()), picoMaximo)
    }

    /// La transcripcion partida en trozos con su minuto (`voz::trozos` de
    /// Windows): parrafos separados por un renglon en blanco, cada uno con
    /// su `[m:ss]` delante si lo lleva. Para pintar la burbuja sin las
    /// marcas dentro del texto.
    public static func trozos(_ texto: String) -> [(ms: Int64?, texto: String)] {
        var salida: [(ms: Int64?, texto: String)] = []
        var bloque: [String] = []
        func cerrar() {
            let limpio = bloque.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
            bloque = []
            guard !limpio.isEmpty else { return }
            if let t = tiempo(de: limpio) {
                salida.append((ms: Optional(t.0), texto: t.1.trimmingCharacters(in: .whitespacesAndNewlines)))
            } else {
                salida.append((ms: nil, texto: limpio))
            }
        }
        for linea in texto.replacingOccurrences(of: "\r\n", with: "\n").components(separatedBy: "\n") {
            if linea.trimmingCharacters(in: .whitespaces).isEmpty {
                cerrar()
            } else {
                bloque.append(linea)
            }
        }
        cerrar()
        return salida
    }

    /// El milisegundo de un `[1:23] texto` y el texto sin la marca
    /// (`tiempo_de`). `nil` si no empieza con una marca valida.
    public static func tiempo(de linea: String) -> (Int64, String)? {
        let r = linea.drop(while: { $0 == " " || $0 == "\t" })
        guard r.first == "[", let cierre = r.firstIndex(of: "]") else { return nil }
        let dentro = r[r.index(after: r.startIndex)..<cierre]
        var despues = r[r.index(after: cierre)...]
        if despues.first == " " { despues = despues.dropFirst() }
        let partes = dentro.split(separator: ":", omittingEmptySubsequences: false)
        func numero(_ s: Substring) -> Int64? {
            guard !s.isEmpty, s.allSatisfy({ $0.isASCII && $0.isNumber }) else { return nil }
            return Int64(s)
        }
        func dos(_ s: Substring) -> Int64? { s.count == 2 ? numero(s) : nil }
        var h: Int64 = 0
        var m: Int64 = 0
        var s: Int64 = 0
        if partes.count == 2 {
            guard let a = numero(partes[0]), let b = dos(partes[1]) else { return nil }
            m = a
            s = b
        } else if partes.count == 3 {
            guard let a = numero(partes[0]), let b = dos(partes[1]), let c = dos(partes[2]) else { return nil }
            h = a
            m = b
            s = c
        } else {
            return nil
        }
        guard m <= 59, s <= 59 else { return nil }
        return ((h * 3600 + m * 60 + s) * 1000, String(despues))
    }
}

/// El acumulador de picos mientras se graba, acotado (`Picos` de
/// `pixpin_audio`). Lleva la cuenta en MUESTRAS, no en relojes, para que la
/// onda no dependa de lo cargado que estuviera el Mac (`Reparto`).
public struct RepartoPicos {
    private let porPico: Int
    private var llevadas = 0
    private var maximo: Int32 = 0
    private var lista: [Int32] = []
    private var paso = 1
    private var vistos = 0
    private let tope: Int

    /// `muestreo` del flujo mono que se le va a dar.
    public init(muestreo: Double, tope: Int = FormatoVoz.picosGuardados) {
        porPico = max(Int(muestreo * Double(FormatoVoz.msEntrePicos) / 1000), 1)
        self.tope = max(tope, 2)
        lista.reserveCapacity(self.tope)
    }

    /// Mete un bloque de muestras mono en coma flotante.
    public mutating func empuja(_ muestras: UnsafeBufferPointer<Float>) {
        for x in muestras {
            maximo = max(maximo, FormatoVoz.pico(x))
            llevadas += 1
            if llevadas >= porPico {
                anota(maximo)
                maximo = 0
                llevadas = 0
            }
        }
    }

    /// Cierra el ultimo pico a medias y devuelve la lista para el mensaje.
    public mutating func terminar() -> [Int32] {
        if llevadas > 0 {
            anota(maximo)
            llevadas = 0
            maximo = 0
        }
        return lista
    }

    private mutating func anota(_ pico: Int32) {
        vistos += 1
        guard vistos % paso == 0 else { return }
        lista.append(min(max(pico, 0), FormatoVoz.picoMaximo))
        if lista.count >= tope {
            lista = RepartoPicos.aMitad(lista)
            paso *= 2
            vistos = 0
        }
    }

    /// Junta de dos en dos promediando; el impar del final se queda.
    public static func aMitad(_ v: [Int32]) -> [Int32] {
        guard v.count >= 2 else { return v }
        var salida: [Int32] = []
        salida.reserveCapacity((v.count + 1) / 2)
        var i = 0
        while i < v.count {
            salida.append(i + 1 < v.count ? Int32((Int(v[i]) + Int(v[i + 1])) / 2) : v[i])
            i += 2
        }
        return salida
    }
}
