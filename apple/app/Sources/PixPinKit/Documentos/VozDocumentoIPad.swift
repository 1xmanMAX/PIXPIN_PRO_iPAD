// **Escuchar un PDF en voz alta** en el iPad (la L del lector de Windows y
// del Mac): el texto de cada hoja (`PDFPage.string`) se corta en trozos con
// `VozAlta.cortes` (en UTF-16, lo que pide `PDFPage.selection(for:)`) y se
// dice con `AVSpeechSynthesizer`, trozo a trozo, resaltando en la hoja lo que
// suena. El idioma y la voz salen de `VozAlta` (los mismos que el Mac).
//
// Es la voz del lector, no la de la app (`LeerEnVoz` del Mac, que en el iPad
// es de la zona de voz): suena mientras el visor esta abierto.

import AVFoundation
import Foundation
import PDFKit
import UIKit

@MainActor
final class VozDocumentoIPad: NSObject, ObservableObject {
    @Published private(set) var leyendo = false
    @Published private(set) var enPausa = false
    @Published private(set) var velocidad: Float = 1
    /// «Mónica · mejorada»: la voz que lee.
    @Published private(set) var nombreVoz = ""

    weak var vistaPDF: PDFView?
    /// Donde se paro (pagina y altura dentro de ella, 0 arriba): el lector
    /// pone ahi el marcador verde de la voz.
    var alParar: ((_ pagina: Int, _ altura: Double) -> Void)?

    private struct Trozo {
        let pagina: Int
        let rango: NSRange
        let texto: String
    }

    private let sintetizador = AVSpeechSynthesizer()
    private var trozos: [Trozo] = []
    private var actual = 0
    private var voz: AVSpeechSynthesisVoice?
    /// Cada frase que se da es otra vuelta: lo que avise una vieja no cuenta.
    private var vuelta = 0
    private var dicha: [ObjectIdentifier: Int] = [:]

    override init() {
        super.init()
        sintetizador.delegate = self
    }

    var vivo: Bool { leyendo || enPausa }

    /// Empieza a leer `doc` desde la hoja `pagina` (o sigue si estaba en
    /// pausa). Los trozos se cortan fuera del hilo principal.
    func alternar(doc: PDFDocument?, pagina: Int) {
        if leyendo {
            pausar()
            return
        }
        if enPausa {
            seguir()
            return
        }
        guard let doc else { return }
        var textos: [(Int, String)] = []
        for i in 0..<doc.pageCount {
            if let s = doc.page(at: i)?.string, !s.isEmpty { textos.append((i, s)) }
        }
        let hojas = textos
        Task {
            let hechos = await Task.detached(priority: .userInitiated) { () -> [Trozo] in
                var t: [Trozo] = []
                for (i, s) in hojas {
                    let ns = s as NSString
                    for r in VozAlta.cortes(s, tope: VozAlta.tope) {
                        let dicho = VozAlta.paraDecir(ns.substring(with: r))
                        if !dicho.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                            t.append(Trozo(pagina: i, rango: r, texto: dicho))
                        }
                    }
                }
                return t
            }.value
            guard !hechos.isEmpty else { return }
            self.trozos = hechos
            let muestra = hechos.prefix(12).map(\.texto).joined(separator: " ")
            let idioma = VozAlta.idiomaParaLeer(muestra: muestra, lang: "",
                                                delEquipo: Locale.current.identifier)
            self.voz = VozAlta.voz(para: idioma, preferida: VozAlta.preferida(lengua: idioma))
                ?? AVSpeechSynthesisVoice(language: VozAlta.idiomaDeLaApp)
            self.nombreVoz = self.voz.map { VozAlta.nombre(de: $0) } ?? ""
            self.actual = hechos.firstIndex { $0.pagina >= pagina } ?? 0
            try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
            try? AVAudioSession.sharedInstance().setActive(true)
            self.leyendo = true
            self.enPausa = false
            self.decir()
        }
    }

    func pausar() {
        guard leyendo else { return }
        sintetizador.pauseSpeaking(at: .word)
        leyendo = false
        enPausa = true
    }

    func seguir() {
        guard enPausa else { return }
        enPausa = false
        leyendo = true
        if sintetizador.isPaused { sintetizador.continueSpeaking() } else { decir() }
    }

    /// Para del todo y apunta donde se quedo.
    func parar() {
        let estaba = vivo
        vuelta += 1
        sintetizador.stopSpeaking(at: .immediate)
        leyendo = false
        enPausa = false
        vistaPDF?.clearSelection()
        if estaba, actual < trozos.count { apuntar(trozos[actual]) }
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    /// La velocidad siguiente (1×, 1,25×...): sigue desde el trozo que sonaba.
    func otraVelocidad() {
        velocidad = VozAlta.siguienteVelocidad(velocidad)
        if leyendo {
            vuelta += 1
            sintetizador.stopSpeaking(at: .immediate)
            decir()
        }
    }

    func siguiente() { saltar(1) }
    func anterior() { saltar(-1) }

    private func saltar(_ d: Int) {
        guard !trozos.isEmpty else { return }
        actual = max(0, min(actual + d, trozos.count - 1))
        if leyendo {
            vuelta += 1
            sintetizador.stopSpeaking(at: .immediate)
            decir()
        }
    }

    private func decir() {
        guard actual < trozos.count else {
            acabar()
            return
        }
        let t = trozos[actual]
        resaltar(t)
        let frase = AVSpeechUtterance(string: t.texto)
        frase.voice = voz
        frase.rate = VozAlta.tasaDeApple(velocidad)
        vuelta += 1
        dicha[ObjectIdentifier(frase)] = vuelta
        sintetizador.speak(frase)
    }

    private func resaltar(_ t: Trozo) {
        guard let v = vistaPDF, let p = v.document?.page(at: t.pagina),
              let sel = p.selection(for: t.rango) else { return }
        sel.color = UIColor.systemYellow.withAlphaComponent(0.45)
        v.setCurrentSelection(sel, animate: false)
        // Solo se mueve la vista si lo que suena no se ve.
        let caja = v.convert(sel.bounds(for: p), from: p)
        if !v.bounds.insetBy(dx: 0, dy: 60).intersects(caja) { v.go(to: sel) }
    }

    private func apuntar(_ t: Trozo) {
        guard let p = vistaPDF?.document?.page(at: t.pagina), let sel = p.selection(for: t.rango) else {
            alParar?(t.pagina, 0)
            return
        }
        let b = p.bounds(for: .cropBox), s = sel.bounds(for: p)
        let altura = b.height > 0 ? Double(min(max((b.maxY - s.maxY) / b.height, 0), 1)) : 0
        alParar?(t.pagina, altura)
    }

    private func acabar() {
        leyendo = false
        enPausa = false
        vistaPDF?.clearSelection()
        actual = 0
    }

    private func acabo(_ id: ObjectIdentifier) {
        guard leyendo, dicha.removeValue(forKey: id) == vuelta else { return }
        actual += 1
        decir()
    }
}

extension VozDocumentoIPad: AVSpeechSynthesizerDelegate {
    nonisolated func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) {
        let id = ObjectIdentifier(utterance)
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated { self?.acabo(id) }
        }
    }
}
