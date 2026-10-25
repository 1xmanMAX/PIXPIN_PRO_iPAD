// Sacar el texto de una imagen con Vision (lo que en Windows hace
// `pixpin-ocr` con Windows.Media.Ocr). Vision usa el Neural Engine cuando lo
// hay (§4bis.4 del plan) y corre fuera del hilo principal.
//
// La usan el pin («Sacar texto») y la captura.

import CoreGraphics
import Foundation
import Vision

/// El texto de `imagen`, una linea por renglon reconocido y en orden de
/// lectura. Cadena vacia si no hay texto.
public func sacarTexto(de imagen: CGImage) async throws -> String {
    try await withCheckedThrowingContinuation { (continuacion: CheckedContinuation<String, Error>) in
        DispatchQueue.global(qos: .userInitiated).async {
            let peticion = VNRecognizeTextRequest()
            peticion.recognitionLevel = .accurate
            peticion.recognitionLanguages = ["es-ES", "en-US"]
            peticion.usesLanguageCorrection = true
            let manejador = VNImageRequestHandler(cgImage: imagen, options: [:])
            do {
                try manejador.perform([peticion])
                let lineas = (peticion.results ?? []).compactMap { $0.topCandidates(1).first?.string }
                continuacion.resume(returning: lineas.joined(separator: "\n"))
            } catch {
                continuacion.resume(throwing: error)
            }
        }
    }
}
