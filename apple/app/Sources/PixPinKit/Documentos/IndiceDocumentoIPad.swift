// **El indice del lector** (Windows, `lector_pin.rs`; el Mac,
// `LectorDocumento.swift`): los capitulos del documento con su sangria y su
// pagina; tocar uno lleva a el. En un PDF es su esquema
// (`PDFDocument.outlineRoot`); en un Word o un EPUB reflujados, sus titulos
// (`EntradaIndice` hecha por el visor de lectura seguida).

import PDFKit
import SwiftUI

/// Una fila del indice.
struct EntradaIndice: Identifiable, Equatable {
    let id: Int
    let titulo: String
    /// 0 los capitulos, 1 sus apartados...
    let nivel: Int
    /// La pagina (desde 0) o, en un texto reflujado, el parrafo.
    let destino: Int

    /// **El esquema de un PDF**, aplanado en orden de lectura.
    static func de(_ doc: PDFDocument?) -> [EntradaIndice] {
        guard let doc, let raiz = doc.outlineRoot else { return [] }
        var v: [EntradaIndice] = []
        func bajar(_ o: PDFOutline, nivel: Int) {
            for i in 0 ..< o.numberOfChildren {
                guard let hijo = o.child(at: i) else { continue }
                let titulo = (hijo.label ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                let pagina = hijo.destination?.page.map { doc.index(for: $0) } ?? -1
                if !titulo.isEmpty, pagina >= 0 {
                    v.append(EntradaIndice(id: v.count, titulo: titulo, nivel: nivel, destino: pagina))
                }
                if v.count < 2000 { bajar(hijo, nivel: nivel + 1) }
            }
        }
        bajar(raiz, nivel: 0)
        return v
    }

    /// La entrada en la que se esta: la ultima que empieza en `actual` o antes.
    static func actual(_ lista: [EntradaIndice], en actual: Int) -> Int? {
        lista.last(where: { $0.destino <= actual })?.id
    }
}

/// La hoja del indice.
struct HojaIndiceIPad: View {
    let entradas: [EntradaIndice]
    /// Donde se esta (pagina o parrafo), para marcar el capitulo.
    let actual: Int
    /// Como se dice el destino («pág. 4»); `nil` no lo dice.
    var rotulo: ((Int) -> String)? = { "\($0 + 1)" }
    let ir: (EntradaIndice) -> Void
    @Environment(\.dismiss) private var cerrar

    var body: some View {
        NavigationStack {
            Group {
                if entradas.isEmpty {
                    ContentUnavailableView("Sin índice", systemImage: "list.bullet.indent",
                                           description: Text("Este documento no tiene índice."))
                } else {
                    let marcada = EntradaIndice.actual(entradas, en: actual)
                    ScrollViewReader { lector in
                        List(entradas) { e in
                            Button {
                                ir(e)
                                cerrar()
                            } label: {
                                HStack {
                                    Text(e.titulo)
                                        .font(e.nivel == 0 ? .body.weight(.semibold) : .body)
                                        .lineLimit(2)
                                        .padding(.leading, CGFloat(min(e.nivel, 5)) * 16)
                                    Spacer()
                                    if let rotulo {
                                        Text(rotulo(e.destino)).font(.caption.monospacedDigit())
                                            .foregroundStyle(.secondary)
                                    }
                                }
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .listRowBackground(e.id == marcada ? Paleta.v2.acento.opacity(0.15) : nil)
                            .id(e.id)
                        }
                        .listStyle(.plain)
                        .onAppear { if let marcada { lector.scrollTo(marcada, anchor: .center) } }
                    }
                }
            }
            .navigationTitle("Índice")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cerrar") { cerrar() } }
            }
        }
    }
}
