// «Sacar texto» (OCR con Vision, `sacarTexto(de:)`), desde un pin o desde el
// editor de captura: una hoja con el texto reconocido, editable, para
// copiarlo o dejarlo flotando como una nota (lo que en Windows hace
// `pixpin-ocr` con su panel de texto).

import SwiftUI
import UIKit

struct VistaTextoSacado: View {
    let imagen: CGImage
    let cerrar: () -> Void

    @State private var texto = ""
    @State private var leyendo = true
    @State private var fallo: String?

    /// Ensena la hoja encima de todo y empieza a leer.
    @MainActor
    static func sacar(de imagen: CGImage) {
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 520, height: 560)) { cerrar in
            VistaTextoSacado(imagen: imagen, cerrar: cerrar)
        }
    }

    var body: some View {
        NavigationStack {
            Group {
                if leyendo {
                    VStack(spacing: 12) {
                        ProgressView()
                        Text("Leyendo el texto…").foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let fallo {
                    ContentUnavailableView("No se pudo leer", systemImage: "exclamationmark.triangle",
                                           description: Text(fallo))
                } else if texto.isEmpty {
                    ContentUnavailableView("No hay texto", systemImage: "text.viewfinder",
                                           description: Text("No se ha encontrado texto en la imagen."))
                } else {
                    TextEditor(text: $texto)
                        .font(.body)
                        .padding(8)
                }
            }
            .navigationTitle("Texto")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cerrar", action: cerrar).keyboardShortcut(.cancelAction)
                }
                ToolbarItemGroup(placement: .primaryAction) {
                    Button {
                        GestorPinesIPad.compartido.pinearNota(texto: texto)
                        cerrar()
                    } label: {
                        Label("Pinear como nota", systemImage: "pin")
                    }
                    .disabled(texto.isEmpty)
                    Button {
                        UIPasteboard.general.string = texto
                        UINotificationFeedbackGenerator().notificationOccurred(.success)
                        cerrar()
                    } label: {
                        Label("Copiar", systemImage: "doc.on.doc")
                    }
                    .keyboardShortcut("c", modifiers: .command)
                    .disabled(texto.isEmpty)
                }
            }
        }
        .task {
            do {
                texto = try await sacarTexto(de: imagen)
            } catch {
                fallo = "\(error.localizedDescription)"
            }
            leyendo = false
        }
    }
}
