// **Las voces de Microsoft** («Leer en voz alta» de Edge), como Windows
// (`crates/pixpin-voz/src/edge.rs`) y el movil (`EdgeVoz.kt`): voces
// neuronales, gratis y sin cuenta. Se le habla como lo hace Edge: el
// `Sec-MS-GEC` (la hora redondeada a cinco minutos con el token, en
// SHA-256), un mensaje de ajustes y el texto en SSML; vuelve el MP3 a trozos.
//
// No es un servicio oficial: si falla, el lector vuelve a la voz del Mac.
// **El texto se manda a Microsoft.** Lo leido se guarda en la cache (un MP3
// por voz y texto): volver a oir un parrafo no gasta red.

import AVFoundation
import CryptoKit
import Foundation

enum EdgeVoz {
    static let token = "6A5AA1D4EAFF4E9FB37E23D68491D6F4"
    static let chromium = "143.0.3650.75"
    static let servidorDeHablar = "api.msedgeservices.com"
    static let rutaDeHablar = "/tts/cognitiveservices/websocket/v1"

    /// Si se usan (se puede apagar en los ajustes: `PixPinVocesMicrosoft`).
    static var activas: Bool {
        UserDefaults.standard.object(forKey: "PixPinVocesMicrosoft") as? Bool ?? true
    }

    /// **La voz de Microsoft para un idioma**: la natural de su lengua; si
    /// no se conoce, una multilingue (lee cualquier idioma).
    static func voz(para idioma: String) -> String {
        let l = idioma.lowercased()
        let lengua = String(l.prefix(2))
        let porPais: [String: String] = [
            "es-es": "es-ES-ElviraNeural", "es-mx": "es-MX-DaliaNeural", "es-pe": "es-PE-CamilaNeural",
            "es-ar": "es-AR-ElenaNeural", "es-co": "es-CO-SalomeNeural", "es-us": "es-US-PalomaNeural",
            "en-gb": "en-GB-SoniaNeural", "pt-pt": "pt-PT-RaquelNeural",
        ]
        if let v = porPais[String(l.replacingOccurrences(of: "_", with: "-").prefix(5))] { return v }
        let porLengua: [String: String] = [
            "es": "es-MX-DaliaNeural", "en": "en-US-AvaNeural", "pt": "pt-BR-FranciscaNeural",
            "fr": "fr-FR-DeniseNeural", "de": "de-DE-KatjaNeural", "it": "it-IT-ElsaNeural",
            "ja": "ja-JP-NanamiNeural", "zh": "zh-CN-XiaoxiaoNeural", "ko": "ko-KR-SunHiNeural",
        ]
        return porLengua[lengua] ?? "en-US-AvaMultilingualNeural"
    }

    /// El nombre corto que se ensena: «Dalia (Microsoft)».
    static func nombre(_ voz: String) -> String {
        let partes = voz.split(separator: "-")
        let n = partes.count >= 3 ? String(partes[2]) : voz
        return n.replacingOccurrences(of: "MultilingualNeural", with: "").replacingOccurrences(of: "Neural", with: "")
            + " · Microsoft"
    }

    /// El `Sec-MS-GEC`: la hora de Windows (centenas de ns desde 1601)
    /// redondeada a cinco minutos, con el token, en SHA-256 y mayusculas.
    static func gec(_ unix: Double) -> String {
        var t = UInt64(max(0, (unix + 11_644_473_600).rounded(.down)))
        t -= t % 300
        let texto = "\(t)0000000" + token
        return SHA256.hash(data: Data(texto.utf8)).map { String(format: "%02X", $0) }.joined()
    }

    /// La fecha como la escribe JavaScript, en UTC.
    static func fecha(_ d: Date = Date()) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = "EEE MMM dd yyyy HH:mm:ss"
        return f.string(from: d) + " GMT+0000 (Coordinated Universal Time)"
    }

    static func escapar(_ t: String) -> String {
        let limpio = String(t.unicodeScalars.map { s -> Character in
            switch s.value { case 0 ... 8, 11 ... 12, 14 ... 31: return " "; default: return Character(s) }
        })
        return limpio.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;").replacingOccurrences(of: "'", with: "&apos;")
            .replacingOccurrences(of: "\"", with: "&quot;")
    }

    static func ajustes(_ fecha: String) -> String {
        "X-Timestamp:\(fecha)\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n"
            + "{\"context\":{\"synthesis\":{\"audio\":{\"metadataoptions\":{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"},"
            + "\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}\r\n"
    }

    static func texto(_ pedido: String, _ fecha: String, voz: String, _ t: String) -> String {
        "X-RequestId:\(pedido)\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:\(fecha)Z\r\nPath:ssml\r\n\r\n"
            + "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>"
            + "<voice name='\(escapar(voz))'><prosody pitch='+0Hz' rate='+0%' volume='+0%'>\(escapar(t))</prosody></voice></speak>"
    }

    /// El trozo de MP3 de una trama binaria (`None` si no es audio).
    static func audio(_ trama: Data) -> Data? {
        let b = [UInt8](trama)
        guard b.count >= 2 else { return nil }
        let largo = Int(b[0]) << 8 | Int(b[1])
        guard largo + 2 <= b.count else { return nil }
        let cab = String(decoding: b[2 ..< 2 + largo], as: UTF8.self)
        guard cab.components(separatedBy: "\r\n").contains("Path:audio"), largo + 2 < b.count else { return nil }
        return Data(b[(2 + largo)...])
    }

    static func uuid() -> String { UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased() }

    /// La carpeta de lo ya leido.
    static var cache: URL {
        let c = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("PixPin/voces-microsoft", isDirectory: true)
        try? FileManager.default.createDirectory(at: c, withIntermediateDirectories: true)
        return c
    }

    static func enCache(_ voz: String, _ t: String) -> URL {
        let h = SHA256.hash(data: Data((voz + "\u{0}" + t).utf8)).map { String(format: "%02x", $0) }.joined()
        return cache.appendingPathComponent(String(h.prefix(32)) + ".mp3")
    }

    /// **El MP3 de `t` con `voz`** (de la cache o de Microsoft).
    static func mp3(_ t: String, voz: String) async throws -> URL {
        let destino = enCache(voz, t)
        if FileManager.default.fileExists(atPath: destino.path) { return destino }
        let conexion = uuid()
        var c = URLComponents()
        c.scheme = "wss"
        c.host = servidorDeHablar
        c.path = rutaDeHablar
        c.queryItems = [
            URLQueryItem(name: "Ocp-Apim-Subscription-Key", value: token),
            URLQueryItem(name: "ConnectionId", value: conexion),
            URLQueryItem(name: "Sec-MS-GEC", value: gec(Date().timeIntervalSince1970)),
            URLQueryItem(name: "Sec-MS-GEC-Version", value: "1-\(chromium)"),
        ]
        guard let url = c.url else { throw URLError(.badURL) }
        var r = URLRequest(url: url, timeoutInterval: 15)
        let m = chromium.split(separator: ".").first.map(String.init) ?? chromium
        r.setValue("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/\(m).0.0.0 Safari/537.36 Edg/\(m).0.0.0",
                   forHTTPHeaderField: "User-Agent")
        r.setValue("en-US,en;q=0.9", forHTTPHeaderField: "Accept-Language")
        r.setValue("no-cache", forHTTPHeaderField: "Pragma")
        r.setValue("no-cache", forHTTPHeaderField: "Cache-Control")
        r.setValue("chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold", forHTTPHeaderField: "Origin")
        r.setValue("muid=\(uuid().uppercased());", forHTTPHeaderField: "Cookie")
        let ws = URLSession.shared.webSocketTask(with: r)
        ws.resume()
        defer { ws.cancel(with: .normalClosure, reason: nil) }
        let f = fecha()
        try await ws.send(.string(ajustes(f)))
        try await ws.send(.string(texto(uuid(), f, voz: voz, t)))
        var mp3 = Data()
        while true {
            switch try await ws.receive() {
            case let .data(d):
                if let a = audio(d) { mp3.append(a) }
            case let .string(s):
                if s.contains("Path:turn.end") {
                    guard !mp3.isEmpty else { throw URLError(.zeroByteResource) }
                    try mp3.write(to: destino, options: .atomic)
                    podar()
                    return destino
                }
            @unknown default:
                break
            }
        }
    }

    /// La cache no pasa de 150 MB: se van los mas viejos.
    static func podar() {
        let fm = FileManager.default
        guard let l = try? fm.contentsOfDirectory(at: cache, includingPropertiesForKeys: [.fileSizeKey, .contentAccessDateKey]) else { return }
        var total = 0
        let datos = l.compactMap { u -> (URL, Int, Date)? in
            guard let v = try? u.resourceValues(forKeys: [.fileSizeKey, .contentAccessDateKey]) else { return nil }
            total += v.fileSize ?? 0
            return (u, v.fileSize ?? 0, v.contentAccessDate ?? .distantPast)
        }.sorted { $0.2 < $1.2 }
        for (u, n, _) in datos where total > 150 << 20 {
            try? fm.removeItem(at: u)
            total -= n
        }
    }
}

/// **El altavoz con las voces de Microsoft** y, si no se puede (sin red,
/// Microsoft lo cambia), el del Mac. Las palabras se avisan por el tiempo que
/// llevan sonando (el servicio no las dice), para que el verde avance.
@MainActor
final class AltavozMicrosoft: NSObject, Altavoz, AVAudioPlayerDelegate {
    private let delMac = AltavozDelMac()
    private var reproductor: AVAudioPlayer?
    private var vuelta = 0
    private var reloj: Timer?
    private var alAcabar: (@MainActor () -> Void)?
    /// La voz de Microsoft que se usa ahora (para la barra).
    private(set) var vozDeAhora: String?
    /// Fallo una vez: se vuelve al Mac un rato.
    private var caidaHasta = Date.distantPast

    func hablar(_ texto: String, voz: AVSpeechSynthesisVoice?, tasa: Float,
                alDecir: @escaping @MainActor (NSRange) -> Void, alAcabar: @escaping @MainActor () -> Void) {
        callar()
        guard EdgeVoz.activas, Date() > caidaHasta, !texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return delMac.hablar(texto, voz: voz, tasa: tasa, alDecir: alDecir, alAcabar: alAcabar)
        }
        vuelta += 1
        let mia = vuelta
        let nombre = EdgeVoz.voz(para: voz?.language ?? VozAlta.idiomaDeLaApp)
        vozDeAhora = nombre
        self.alAcabar = alAcabar
        Task {
            do {
                let url = try await EdgeVoz.mp3(texto, voz: nombre)
                guard mia == self.vuelta else { return }
                let p = try AVAudioPlayer(contentsOf: url)
                p.enableRate = true
                // `tasa` es la de AVSpeech: 0,5 es la normal.
                p.rate = max(0.5, min(2, tasa / AVSpeechUtteranceDefaultSpeechRate))
                p.delegate = self
                self.reproductor = p
                p.play()
                self.seguirPalabras(texto, alDecir: alDecir, mia: mia)
            } catch {
                guard mia == self.vuelta else { return }
                self.caidaHasta = Date().addingTimeInterval(120)
                self.vozDeAhora = nil
                self.delMac.hablar(texto, voz: voz, tasa: tasa, alDecir: alDecir, alAcabar: alAcabar)
            }
        }
    }

    /// El verde por las palabras, repartiendo la duracion por sus letras.
    private func seguirPalabras(_ texto: String, alDecir: @escaping @MainActor (NSRange) -> Void, mia: Int) {
        let ns = texto as NSString
        var palabras: [NSRange] = []
        ns.enumerateSubstrings(in: NSRange(location: 0, length: ns.length), options: .byWords) { _, r, _, _ in palabras.append(r) }
        let total = max(1, palabras.reduce(0) { $0 + $1.length + 1 })
        reloj = Timer.scheduledTimer(withTimeInterval: 0.12, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, mia == self.vuelta, let p = self.reproductor, p.duration > 0 else { return }
                let objetivo = Int(Double(total) * p.currentTime / p.duration)
                var acum = 0
                for r in palabras {
                    acum += r.length + 1
                    if acum >= objetivo { alDecir(r); break }
                }
            }
        }
    }

    func pausar() {
        if reproductor != nil { reproductor?.pause() } else { delMac.pausar() }
    }

    func seguir() {
        if reproductor != nil { reproductor?.play() } else { delMac.seguir() }
    }

    func callar() {
        vuelta += 1
        reloj?.invalidate()
        reloj = nil
        reproductor?.stop()
        reproductor = nil
        alAcabar = nil
        delMac.callar()
    }

    nonisolated func audioPlayerDidFinishPlaying(_ p: AVAudioPlayer, successfully: Bool) {
        nonisolated(unsafe) let p = p
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                guard p === self.reproductor else { return }
                self.reloj?.invalidate()
                self.reloj = nil
                self.reproductor = nil
                let a = self.alAcabar
                self.alAcabar = nil
                a?()
            }
        }
    }
}
