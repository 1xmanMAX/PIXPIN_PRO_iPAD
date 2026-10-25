// **Los marcadores del lienzo con su vista** (Windows,
// `ventana_editor/marcas.rs`, 25-oct-2026, como las vistas con nombre de
// AutoCAD; del Mac, `Lienzo/MarcasLienzo.swift`). Las cuentas y los ficheros
// estan en Rust (`pixpin-nucleo/src/marcas_lienzo.rs`, `MarcasDelLienzo`):
// la linea `pins/draw/<d>.marcas` y el `pins/draw/<d>.vistas.json` del chat,
// los mismos que Windows, el Mac y el movil. Aqui queda lo de la pantalla:
//
// - **Poner**: el boton del marcador de arriba del riel (o ⌘M) abre la
//   tira de emoticonos; se elige uno y el siguiente toque en el lienzo lo
//   planta ahi, con lo que se ve en ese momento como su vista.
// - **Ir**: un toque en su punto del riel (o ⌘1…⌘9 para los nueve
//   primeros), o en el marcador con cualquier herramienta menos la flecha.
//   Con vista, la camara viaja hasta ella (encuadre y aumento); sin ella,
//   hasta dejarla arriba.
// - **Su vista, la de ahora** y **Quitar**: mantener pulsado su punto del
//   riel (el ⇧clic y el clic derecho del Mac).

import PixPinCore
import SwiftUI

extension ModeloEditorLienzo {
    /// Lee los marcadores del dibujo (fuera del hilo principal).
    func cargarMarcas() {
        let ruta = editor.ruta()
        _ = Task<Void, Never> { [weak self] in
            let m = await Task.detached(priority: .utility) { () -> (MarcasDelLienzo, [MarcaDelLienzo]) in
                let m = MarcasDelLienzo.juntoAlDibujo(ruta: ruta)
                return (m, m.lista())
            }.value
            guard let self else { return }
            self.marcasDelLienzo = m.0
            self.marcas = m.1
            self.vista?.setNeedsDisplay()
        }
    }

    /// Lo que llego de otro aparato.
    public func releerMarcas() {
        guard let m = marcasDelLienzo, m.releer() else { return }
        refrescarMarcas()
    }

    /// La lista otra vez desde Rust, y el lienzo repintado.
    func refrescarMarcas() {
        marcas = marcasDelLienzo?.lista() ?? []
        vista?.setNeedsDisplay()
    }

    /// Planta el marcador que se estaba poniendo en `(x, y)` del mundo, con
    /// lo que se ve ahora como su vista.
    func plantarMarca(x: Double, y: Double, emoji: String) {
        poniendoMarca = nil
        guard let m = marcasDelLienzo, let v = vista else { return }
        _ = m.poner(x: x, y: y, emoji: emoji, camara: v.camaraDeMarcas,
                    anchoPx: Float(v.bounds.width), altoPx: Float(v.bounds.height))
        refrescarMarcas()
    }

    func quitarMarca(_ id: Int64) {
        guard let m = marcasDelLienzo, m.quitar(id: id) else { return }
        refrescarMarcas()
    }

    /// **La vista de la marca pasa a ser la de ahora.**
    func fijarVistaDeMarca(_ id: Int64) {
        guard let m = marcasDelLienzo, let v = vista,
              m.fijarVista(id: id, camara: v.camaraDeMarcas, anchoPx: Float(v.bounds.width),
                           altoPx: Float(v.bounds.height)) else { return }
        refrescarMarcas()
    }

    /// **El boton del marco del riel**: coge el marco y, pulsado otra vez,
    /// vuelve a la herramienta de antes (`alternarMarco` del Mac).
    func alternarMarco() {
        if herramienta == "marco" {
            elegir(antesDelMarco.flatMap { $0 == "marco" ? nil : $0 } ?? "seleccion")
            return
        }
        antesDelMarco = herramienta
        elegir("marco")
    }

    /// Va al marcador `i` del riel (desde 0), si lo hay.
    func irAMarca(_ i: Int) {
        guard marcas.indices.contains(i) else { return }
        vista?.ir(aMarca: marcas[i].id)
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
            .help(modelo.poniendoMarca != nil ? "Dejar de poner el marcador" : "Marcador (⌘M)")
            .keyboardShortcut("m", modifiers: .command)
            .popover(isPresented: $eligiendo) {
                TiraEmojisLienzo(emojis: modelo.marcasDelLienzo?.emojis() ?? emojisDeMarcas()) { emoji in
                    eligiendo = false
                    modelo.poniendoMarca = emoji
                }
            }
            // El marco (Windows 22-oct y el Mac: salio de la barra al riel,
            // `DestinoRiel::Imprimir`): lo coge y, otra vez, vuelve a la de antes.
            Button { modelo.alternarMarco() } label: {
                Image(systemName: "rectangle.dashed")
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(modelo.herramienta == "marco" ? tema.activoIcono : tema.icono)
                    .frame(width: 36, height: 36)
                    .background(modelo.herramienta == "marco" ? tema.activoFondo : Color.clear,
                                in: RoundedRectangle(cornerRadius: 8))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Marcos para imprimir (C)")
            if let e = modelo.poniendoMarca {
                Text(e).font(.system(size: 16))
                    .help("Pulsa en el lienzo donde va la marca")
            }
            ForEach(Array(modelo.marcas.enumerated()), id: \.element.id) { i, m in
                Button { modelo.irAMarca(i) } label: {
                    Text(m.emoji)
                        .font(.system(size: 14))
                        .frame(width: 30, height: 30)
                        .background(Circle().fill(Color(.sRGB, red: 0x14 / 255, green: 0x18 / 255, blue: 0x2B / 255,
                                                        opacity: 0.9)))
                        // La que guarda su vista lleva el aro.
                        .overlay { if m.conVista { Circle().strokeBorder(Color.white.opacity(0.7), lineWidth: 1.5) } }
                        .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .help(i < 9 ? "Ir al marcador (⌘\(i + 1))" : "Ir al marcador")
                .contextMenu {
                    Button("Guardar aquí la vista de ahora") { modelo.fijarVistaDeMarca(m.id) }
                    Button("Quitar", role: .destructive) { modelo.quitarMarca(m.id) }
                }
            }
        }
        .padding(4)
        .modifier(IslaLienzo(tema: tema))
        .background(atajos)
    }

    /// ⌘1…⌘9: los nueve primeros; ⌘⇧1…9 les guarda la vista de ahora.
    private var atajos: some View {
        ZStack {
            ForEach(1 ... 9, id: \.self) { n in
                Button("") { modelo.irAMarca(n - 1) }
                    .keyboardShortcut(KeyEquivalent(Character("\(n)")), modifiers: .command)
                Button("") {
                    if modelo.marcas.indices.contains(n - 1) { modelo.fijarVistaDeMarca(modelo.marcas[n - 1].id) }
                }
                .keyboardShortcut(KeyEquivalent(Character("\(n)")), modifiers: [.command, .shift])
            }
        }
        .frame(width: 0, height: 0)
        .opacity(0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// La tira de emoticonos de los marcadores (los de Windows, `EMOJIS`).
struct TiraEmojisLienzo: View {
    var emojis: [String] = emojisDeMarcas()
    let elegir: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("¿Con qué lo marcas?").font(.caption).foregroundStyle(.secondary)
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(40)), count: 6), spacing: 6) {
                ForEach(emojis, id: \.self) { e in
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
