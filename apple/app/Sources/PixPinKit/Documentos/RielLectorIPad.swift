// **El riel del lector** a la derecha, como el pin del lector de Windows
// (`lector_pin.rs`, la isla con el marcador y el lapiz): arriba la isla
// (marcador y lapiz), debajo los marcadores puestos (un toque va, mantener
// quita) y, al anotar, las herramientas de la tinta: lapiz, resaltador,
// goma, los cinco colores, deshacer y rehacer, «Dibujar con el dedo» y los
// espacios para anotar a cada lado.

import PixPinCore
import SwiftUI

struct RielLectorIPad: View {
    let c: ControladorLectorIPad
    @ObservedObject var modelo: ModeloLectorIPad

    var body: some View {
        VStack(spacing: 10) {
            isla
            if modelo.anotando { herramientas }
            if !modelo.marcas.isEmpty { marcadores }
        }
        .fixedSize()
    }

    private var isla: some View {
        VStack(spacing: 2) {
            BotonRielIPad(simbolo: "bookmark", ayuda: "Poner un marcador", encendido: modelo.poniendoMarca) {
                modelo.poniendoMarca.toggle()
            }
            BotonRielIPad(simbolo: "pencil.tip", ayuda: modelo.anotando ? "Dejar de anotar" : "Anotar",
                          encendido: modelo.anotando) {
                c.alternarAnotar()
            }
        }
        .padding(3)
        .background(RoundedRectangle(cornerRadius: 9).fill(.regularMaterial))
        .shadow(color: .black.opacity(0.16), radius: 6, y: 2)
    }

    private var herramientas: some View {
        VStack(spacing: 2) {
            BotonRielIPad(simbolo: "pencil", ayuda: "Lápiz", encendido: modelo.herramienta == "lapiz") {
                c.elegir("lapiz")
            }
            BotonRielIPad(simbolo: "highlighter", ayuda: "Resaltador", encendido: modelo.herramienta == "resaltador") {
                c.elegir("resaltador")
            }
            BotonRielIPad(simbolo: "eraser", ayuda: "Goma", encendido: modelo.herramienta == "borrador") {
                c.elegir("borrador")
            }
            Divider().frame(width: 28)
            ForEach(ColorTinta.todos) { col in
                Button { c.ponerColor(col) } label: {
                    Circle().fill(col.color)
                        .frame(width: 22, height: 22)
                        .overlay(Circle().strokeBorder(Color.primary.opacity(0.25), lineWidth: 1))
                        .overlay(Circle().strokeBorder(Paleta.v2.acento, lineWidth: modelo.color == col ? 3 : 0)
                            .padding(-3))
                        .frame(width: 36, height: 32)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(col.id.capitalized)
            }
            Divider().frame(width: 28)
            BotonRielIPad(simbolo: "arrow.uturn.backward", ayuda: "Deshacer (⌘Z)") { c.deshacer() }
                .keyboardShortcut("z", modifiers: .command)
            BotonRielIPad(simbolo: "arrow.uturn.forward", ayuda: "Rehacer (⇧⌘Z)") { c.rehacer() }
                .keyboardShortcut("z", modifiers: [.command, .shift])
            Divider().frame(width: 28)
            BotonRielIPad(simbolo: "hand.point.up.left", ayuda: "Dibujar con el dedo", encendido: modelo.conDedo) {
                c.alternarDedo()
            }
            BotonRielIPad(simbolo: "rectangle.lefthalf.inset.filled", ayuda: "Espacio para anotar a la izquierda",
                          encendido: modelo.espacios & 1 != 0) {
                c.alternarEspacio(1)
            }
            BotonRielIPad(simbolo: "rectangle.righthalf.inset.filled", ayuda: "Espacio para anotar a la derecha",
                          encendido: modelo.espacios & 2 != 0) {
                c.alternarEspacio(2)
            }
        }
        .padding(3)
        .background(RoundedRectangle(cornerRadius: 9).fill(.regularMaterial))
        .shadow(color: .black.opacity(0.16), radius: 6, y: 2)
    }

    /// Los marcadores, en orden de lectura: un toque va; mantener, quitar.
    private var marcadores: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(spacing: 2) {
                ForEach(modelo.marcas, id: \.id) { m in
                    Button { c.ir(a: m) } label: {
                        VStack(spacing: 0) {
                            Text(m.emoji).font(.system(size: 18))
                            Text("\(Int(m.y.rounded(.down)) + 1)").font(.system(size: 9).monospacedDigit())
                                .foregroundStyle(.secondary)
                        }
                        .frame(width: 36, height: 38)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .contextMenu {
                        Button("Ir") { c.ir(a: m) }
                        Button("Quitar este marcador", role: .destructive) { c.quitarMarca(m) }
                        Button("Quitar todos", role: .destructive) { c.quitarMarcas() }
                    }
                }
            }
            .padding(3)
        }
        .frame(maxHeight: 260)
        .background(RoundedRectangle(cornerRadius: 9).fill(.regularMaterial))
        .shadow(color: .black.opacity(0.16), radius: 6, y: 2)
    }
}

/// Un boton del riel: 36 x 36 (comodo con el dedo), pulsado en azul como
/// en Windows.
struct BotonRielIPad: View {
    let simbolo: String
    let ayuda: String
    var encendido = false
    let accion: () -> Void

    var body: some View {
        Button(action: accion) {
            Image(systemName: simbolo)
                .font(.system(size: 16, weight: .medium))
                .foregroundStyle(encendido ? Color.white : Color.primary)
                .frame(width: 36, height: 36)
                .background(RoundedRectangle(cornerRadius: 8)
                    .fill(encendido ? Paleta.v2.azulLleno : Color.clear))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(ayuda)
        .accessibilityLabel(ayuda)
    }
}
