// **Los marcadores del lienzo y su riel** (F5; `ventana_editor/marcas.rs`
// de Windows, `pixpin_ui::riel_marcas`): sitios del dibujo con su
// emoticono, y a la derecha el riel comun de los visores con un punto por
// marcador.
//
// - **Poner**: el boton del marcador de arriba del riel (o ⌘M) abre la
//   tira de emoticonos; se elige uno y el siguiente toque en el lienzo lo
//   planta ahi.
// - **Ir**: un toque en su punto del riel (o ⌘1…⌘9 para los nueve
//   primeros), o en el marcador con cualquier herramienta menos la flecha:
//   la vista viaja hasta dejarlo arriba y centrado, con el mismo aumento.
// - **Quitar**: mantener pulsado su punto del riel.
//
// Se guardan junto al dibujo como en Windows (`EditorLienzo.marcas`, la
// linea del movil `id:x:y:emoji`), y viajan con el dibujo de un chat.

import PixPinCore
import SwiftUI

extension ModeloEditorLienzo {
    /// Lee los marcadores del dibujo (fuera del hilo principal).
    func cargarMarcas() {
        let editor = self.editor
        _ = Task<Void, Never> { [weak self] in
            let leidas = await Task.detached(priority: .utility) { editor.marcas() }.value
            self?.marcas = leidas
        }
    }

    /// Cambia los marcadores con `cambio` (en el disco, fuera del hilo
    /// principal) y ensena como quedan.
    private func cambiarMarcas(_ cambio: @escaping @Sendable (EditorLienzo) throws -> [MarcaDocumento]) {
        let editor = self.editor
        _ = Task<Void, Never> { [weak self] in
            let r = await Task.detached(priority: .userInitiated) { try? cambio(editor) }.value
            guard let self else { return }
            if let r {
                self.marcas = r
                self.vista?.setNeedsDisplay()
            } else {
                self.error = "No se pudieron guardar los marcadores."
            }
        }
    }

    /// Planta el marcador que se estaba poniendo en `(x, y)` del mundo.
    func plantarMarca(x: Double, y: Double, emoji: String) {
        poniendoMarca = nil
        cambiarMarcas { try $0.ponerMarca(x: x, y: y, emoji: emoji) }
    }

    func quitarMarca(_ id: Int64) {
        cambiarMarcas { try $0.quitarMarca(id: id) }
    }

    /// Va al marcador `i` del riel (desde 0), si lo hay.
    func irAMarca(_ i: Int) {
        guard marcas.indices.contains(i) else { return }
        vista?.ir(a: marcas[i])
    }
}

/// **El riel de los marcadores**, a la derecha: arriba el boton de poner
/// uno (con su tira de emoticonos) y debajo un punto por marcador.
struct RielMarcasLienzo: View {
    @ObservedObject var modelo: ModeloEditorLienzo
    @State private var eligiendo = false

    var body: some View {
        let tema = TemaBarraLienzo.de(modelo)
        VStack(spacing: 6) {
            Button {
                if modelo.poniendoMarca != nil { modelo.poniendoMarca = nil } else { eligiendo = true }
            } label: {
                Image(systemName: modelo.poniendoMarca != nil ? "bookmark.slash" : "bookmark")
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(tema.icono)
                    .frame(width: 36, height: 36)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(modelo.poniendoMarca != nil ? "Dejar de poner el marcador" : "Poner un marcador (⌘M)")
            .keyboardShortcut("m", modifiers: .command)
            .popover(isPresented: $eligiendo) {
                TiraEmojisLienzo { emoji in
                    eligiendo = false
                    modelo.poniendoMarca = emoji
                }
            }
            if let e = modelo.poniendoMarca {
                Text(e).font(.system(size: 16))
                    .help("Toca el lienzo donde va el marcador")
            }
            ForEach(Array(modelo.marcas.enumerated()), id: \.element.id) { i, m in
                Button { modelo.irAMarca(i) } label: {
                    Text(m.emoji)
                        .font(.system(size: 14))
                        .frame(width: 30, height: 30)
                        .background(Circle().fill(Color(.sRGB, red: 0x14 / 255, green: 0x18 / 255, blue: 0x2B / 255,
                                                        opacity: 0.9)))
                        .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .help(i < 9 ? "Ir al marcador (⌘\(i + 1))" : "Ir al marcador")
                .contextMenu {
                    Button("Quitar el marcador", role: .destructive) { modelo.quitarMarca(m.id) }
                }
            }
        }
        .padding(4)
        .modifier(IslaLienzo(tema: tema))
        .background(atajos)
    }

    /// ⌘1…⌘9: los nueve primeros.
    private var atajos: some View {
        ZStack {
            ForEach(1 ... 9, id: \.self) { n in
                Button("") { modelo.irAMarca(n - 1) }
                    .keyboardShortcut(KeyEquivalent(Character("\(n)")), modifiers: .command)
            }
        }
        .frame(width: 0, height: 0)
        .opacity(0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// La tira de emoticonos de los marcadores (los del movil, `emojisDeMarcas`).
struct TiraEmojisLienzo: View {
    let elegir: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Elige un marcador").font(.caption).foregroundStyle(.secondary)
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(40)), count: 6), spacing: 6) {
                ForEach(emojisDeMarcas(), id: \.self) { e in
                    Button { elegir(e) } label: {
                        Text(e).font(.system(size: 22)).frame(width: 40, height: 40)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .padding(12)
    }
}
