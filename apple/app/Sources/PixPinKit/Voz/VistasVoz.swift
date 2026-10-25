// Las vistas de las notas de voz (del Mac, `Voz/VistasVoz.swift`): la
// burbuja `VOZ` (reproducir, la onda con su avance, la duracion y lo que se
// dijo) y la barra de lo que suena (D3), que es la del movil
// (`BarraDelReproductor.kt`) y la de Windows: atras 10, play o pausa,
// adelante 10, la velocidad en ciclo, el titulo con el tiempo, el aspa y la
// linea de avance, que se toca o se arrastra para ir a un punto.

import PixPinCore
import SwiftUI

struct BurbujaVoz: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @ObservedObject private var reproductor = ReproductorVoz.compartido
    @State private var url: URL?
    @State private var transcribiendo = false
    @Environment(\.escalaChat) private var escala

    private var esLaActual: Bool { reproductor.actual == mensaje.id }
    private var anchoOnda: CGFloat { 180 * escala }

    /// El titulo con el que suena (el nombre sin su «.m4a»).
    static func titulo(de m: MensajeChat) -> String {
        let n = m.nombre.trimmingCharacters(in: .whitespaces)
        if n.isEmpty { return "Nota de voz" }
        return n.lowercased().hasSuffix(".m4a") ? String(n.dropLast(4)) : n
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                Button(action: alternar) {
                    Image(systemName: esLaActual && reproductor.sonando ? "pause.circle.fill" : "play.circle.fill")
                        .font(.system(size: 34 * escala))
                        .foregroundStyle(Paleta.chat.enviar)
                }
                .buttonStyle(.plain)
                .disabled(url == nil)
                .accessibilityLabel(esLaActual && reproductor.sonando ? "Pausa" : "Escuchar")

                Group {
                    if esLaActual && reproductor.sonando {
                        // Solo la que suena se repinta mientras suena.
                        TimelineView(.animation(minimumInterval: 0.1, paused: false)) { _ in
                            onda(avance: reproductor.fraccion)
                        }
                    } else {
                        onda(avance: esLaActual ? reproductor.fraccion : 0)
                    }
                }
                .frame(width: anchoOnda, height: 28 * escala)

                Text(FormatoVoz.marca(ms: mensaje.duracionMs))
                    .font(.caption.monospacedDigit()).opacity(0.7)
                // Pasar a texto, a la vista (como Windows y el movil).
                if url != nil {
                    Button(action: transcribir) {
                        if transcribiendo {
                            ProgressView().controlSize(.small)
                        } else {
                            Image(systemName: "captions.bubble").font(.system(size: 17))
                        }
                    }
                    .buttonStyle(.plain)
                    .opacity(0.7)
                    .disabled(transcribiendo)
                    .accessibilityLabel(mensaje.transcripcion?.isEmpty == false ? "Volver a pasar a texto" : "Pasar a texto")
                }
                // Una clase lleva al lado la pastilla de estudiarla (Windows
                // ce6c058): el video o la captura del minuto, el texto y las
                // capturas, que se siguen.
                if let clase = ClaseEnElChat.deUnaClase(mensaje, chat: chat, nucleo: almacen.nucleo) {
                    Button {
                        EstudiarClase.abrir(chat: chat, mensaje: clase.voz, ms: clase.ms, almacen: almacen)
                    } label: {
                        Image(systemName: "book").font(.system(size: 15))
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    .accessibilityLabel("Estudiar la clase")
                }
            }
            texto
        }
        .frame(maxWidth: 380, alignment: .leading)
        .task(id: mensaje.id) {
            guard let ruta = mensaje.ruta, let u = await almacen.url(deRuta: ruta, en: chat) else { return }
            let camino = u.path
            if await Task.detached(priority: .utility, operation: { FileManager.default.fileExists(atPath: camino) }).value {
                url = u
            }
        }
    }

    /// Lo que se dijo, en parrafos y sin los `[m:ss]` dentro del texto.
    @ViewBuilder private var texto: some View {
        let trozos = FormatoVoz.trozos(mensaje.transcripcion ?? "")
        if !trozos.isEmpty {
            Text(trozos.map { $0.texto }.joined(separator: "\n\n"))
                .fixedSize(horizontal: false, vertical: true)
        } else if transcribiendo {
            Text("Pasando a texto…").font(.caption).opacity(0.7)
        } else if !mensaje.texto.isEmpty {
            Text(mensaje.texto)
        }
    }

    /// La onda de los picos (normalizada contra el maximo de la nota, como
    /// `barras_de_onda` de Windows) con lo ya escuchado en color.
    private func onda(avance: Double) -> some View {
        let picos = mensaje.picos
        let ancho = anchoOnda
        return Canvas { ctx, tam in
            let barras = max(min(picos.count, Int(tam.width / 3)), 1)
            let maximo = max(Double(picos.max() ?? 0), 1)
            let paso = tam.width / CGFloat(barras)
            for i in 0..<barras {
                let valor: Double
                if picos.isEmpty {
                    valor = 0.15
                } else {
                    // Cada barra, el maximo de su tramo de picos.
                    let a = i * picos.count / barras
                    let b = max((i + 1) * picos.count / barras, a + 1)
                    valor = Double(picos[a..<min(b, picos.count)].max() ?? 0) / maximo
                }
                let alto = max(CGFloat(valor) * tam.height, 2)
                let rect = CGRect(x: CGFloat(i) * paso, y: (tam.height - alto) / 2,
                                  width: max(paso - 1, 1), height: alto)
                let hecho = (Double(i) + 0.5) / Double(barras) <= avance
                ctx.fill(Path(roundedRect: rect, cornerRadius: 1),
                         with: .color(hecho ? Paleta.chat.enviar : Color.secondary.opacity(0.5)))
            }
        }
        .contentShape(Rectangle())
        .gesture(SpatialTapGesture().onEnded { toque in
            guard esLaActual else { return }
            reproductor.saltar(mensaje.id, a: Double(toque.location.x / max(ancho, 1)))
        })
    }

    private func alternar() {
        guard let url else { return }
        reproductor.alternar(mensaje.id, url: url, titulo: BurbujaVoz.titulo(de: mensaje))
    }

    private func transcribir() {
        guard !transcribiendo else { return }
        transcribiendo = true
        Task {
            await almacen.transcribir(mensaje, en: chat)
            transcribiendo = false
        }
    }
}

/// **La barra de lo que suena**: sale solo si hay algo cargado. Va encima de
/// la caja de escribir del chat.
struct BarraReproductor: View {
    @ObservedObject private var r = ReproductorVoz.compartido
    /// Sin el aspa (quien la use tiene la suya).
    var conCerrar = true

    var body: some View {
        if r.hayAlgo {
            VStack(spacing: 4) {
                HStack(spacing: 2) {
                    boton("gobackward.10", "Atrás 10 s") { r.atras() }
                    boton(r.sonando ? "pause.fill" : "play.fill", r.sonando ? "Pausa" : "Escuchar") {
                        if r.sonando { r.pausar() } else { r.seguir() }
                    }
                    boton("goforward.10", "Adelante 10 s") { r.adelante() }
                    // La velocidad se escribe: «1,5×» dice mas que un icono.
                    Button { r.otraVelocidad() } label: {
                        Text(VozAlta.rotulo(r.velocidad))
                            .font(.system(size: 13, weight: .semibold).monospacedDigit())
                            .foregroundStyle(Paleta.chat.enviar)
                            .frame(minWidth: 44, minHeight: 34)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Velocidad")
                    Text(r.titulo)
                        .font(.system(size: 13))
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.leading, 4)
                    TiempoReproductor()
                    if !r.cola.isEmpty {
                        // La cola: lo que espera, para saltar a ello o quitarlo.
                        Menu {
                            Button { r.siguiente() } label: { Label("Siguiente", systemImage: "forward.end.fill") }
                            Section("En la cola") {
                                ForEach(r.cola, id: \.id) { p in
                                    Button(role: .destructive) { r.quitarDeLaCola(p.id) } label: {
                                        Label(p.titulo.isEmpty ? p.url.lastPathComponent : p.titulo,
                                              systemImage: "minus.circle")
                                    }
                                }
                            }
                        } label: {
                            Text("+\(r.cola.count)")
                                .font(.caption.weight(.semibold).monospacedDigit())
                                .foregroundStyle(.secondary)
                                .frame(minWidth: 34, minHeight: 34)
                                .contentShape(Rectangle())
                        }
                        .accessibilityLabel("La cola: \(r.cola.count)")
                    }
                    if conCerrar { boton("xmark", "Cerrar") { r.parar() } }
                }
                PistaAvance()
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 16))
        }
    }

    private func boton(_ simbolo: String, _ ayuda: String, _ accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            Image(systemName: simbolo).font(.system(size: 16)).frame(width: 38, height: 34)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(ayuda)
    }
}

/// «0:07 / 1:24», al dia mientras suena.
struct TiempoReproductor: View {
    @ObservedObject private var r = ReproductorVoz.compartido

    var body: some View {
        Group {
            if r.sonando {
                TimelineView(.periodic(from: .now, by: 0.25)) { _ in Text(texto) }
            } else {
                Text(texto)
            }
        }
        .font(.caption.monospacedDigit())
        .foregroundStyle(.secondary)
    }

    private var texto: String { "\(FormatoVoz.marca(ms: r.posicionMs)) / \(FormatoVoz.marca(ms: r.duracionMs))" }
}

/// La linea de avance con su bolita: tocar o arrastrar va a ese punto.
struct PistaAvance: View {
    @ObservedObject private var r = ReproductorVoz.compartido

    var body: some View {
        GeometryReader { g in
            Group {
                if r.sonando {
                    TimelineView(.animation(minimumInterval: 0.1, paused: false)) { _ in linea(g.size.width) }
                } else {
                    linea(g.size.width)
                }
            }
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0).onChanged { v in
                r.irA(fraccion: Double(v.location.x / max(g.size.width, 1)))
            })
        }
        .frame(height: 14)
        .accessibilityLabel("Ir a un punto")
    }

    private func linea(_ ancho: CGFloat) -> some View {
        let f = CGFloat(r.fraccion)
        return ZStack(alignment: .leading) {
            Capsule().fill(Color.secondary.opacity(0.3)).frame(height: 3)
            Capsule().fill(Paleta.chat.enviar).frame(width: max(ancho * f, 0), height: 3)
            Circle().fill(Paleta.chat.enviar).frame(width: 11, height: 11).offset(x: max(ancho * f - 5.5, 0))
        }
        .frame(height: 14)
    }
}
