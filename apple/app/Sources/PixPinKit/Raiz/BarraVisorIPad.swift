// La barra comun de todos los visores (la de los pines sin bordes de
// Windows y el Mac): tipo y nombre a la izquierda, los botones propios del
// visor en medio, y a la derecha luna/sol (modo noche), «Fijar como pin» y
// cerrar. Va flotando arriba, plana, y se esconde con un toque en el
// contenido si el visor lo pide.
//
// Todos los visores del iPad la usan, para que se vean igual que en
// Windows: `BarraVisorIPad(tipo:titulo:) { botones propios }`.

import AVKit
import PixPinCore
import QuickLook
import SwiftUI

/// El modo noche comun a todos los visores (como en Windows: uno para todos).
@MainActor
public final class ModoNoche: ObservableObject {
    public static let compartido = ModoNoche()
    @Published public var activo: Bool {
        didSet { UserDefaults.standard.set(activo, forKey: "pixpin.modoNoche") }
    }

    private init() {
        activo = UserDefaults.standard.bool(forKey: "pixpin.modoNoche")
    }
}

public struct BarraVisorIPad<Botones: View>: View {
    let tipo: String
    let icono: String
    let titulo: String
    /// «Fijar como pin»: deja el contenido flotando en la capa de pines
    /// (nil si este visor no se puede fijar).
    var alFijar: (() -> Void)?
    var alCerrar: (() -> Void)?
    let botones: Botones

    @ObservedObject private var noche = ModoNoche.compartido
    @EnvironmentObject private var nav: Navegacion

    public init(tipo: String, icono: String, titulo: String,
                alFijar: (() -> Void)? = nil, alCerrar: (() -> Void)? = nil,
                @ViewBuilder botones: () -> Botones) {
        self.tipo = tipo
        self.icono = icono
        self.titulo = titulo
        self.alFijar = alFijar
        self.alCerrar = alCerrar
        self.botones = botones()
    }

    public var body: some View {
        HStack(spacing: 10) {
            Label {
                VStack(alignment: .leading, spacing: 0) {
                    Text(tipo).font(.caption2).foregroundStyle(.secondary)
                    Text(titulo).font(.subheadline.weight(.semibold)).lineLimit(1)
                }
            } icon: {
                Image(systemName: icono).foregroundStyle(Paleta.v2.acento)
            }
            Spacer(minLength: 8)
            botones
            Divider().frame(height: 22)
            // «Abrir en otra ventana» (solo en la principal, con Stage Manager).
            BotonOtraVentana()
            Button { noche.activo.toggle() } label: {
                Image(systemName: noche.activo ? "sun.max" : "moon")
            }
            .help(noche.activo ? "Modo día" : "Modo noche")
            if let alFijar {
                Button(action: alFijar) { Image(systemName: "pin") }
                    .help("Fijar como pin")
            }
            Button {
                if let alCerrar { alCerrar() } else { nav.cerrarVisor() }
            } label: {
                Image(systemName: "xmark").fontWeight(.semibold)
            }
            .keyboardShortcut(.cancelAction)
            .help("Cerrar (Esc)")
        }
        .buttonStyle(.borderless)
        .font(.body)
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(.regularMaterial, in: Capsule())
        .overlay(Capsule().strokeBorder(.separator.opacity(0.5)))
        .padding(.horizontal, 12)
        .padding(.top, 6)
    }
}

public extension BarraVisorIPad where Botones == EmptyView {
    init(tipo: String, icono: String, titulo: String,
         alFijar: (() -> Void)? = nil, alCerrar: (() -> Void)? = nil) {
        self.init(tipo: tipo, icono: icono, titulo: titulo, alFijar: alFijar, alCerrar: alCerrar) { EmptyView() }
    }
}

// MARK: - Video y audio

/// Video o audio con el reproductor del sistema (con Picture in Picture: el
/// «pin de video» del iPad, que flota encima de otras apps).
public struct VisorMedio: View {
    let url: URL
    let titulo: String
    @State private var reproductor: AVPlayer?

    public init(url: URL, titulo: String) {
        self.url = url
        self.titulo = titulo
    }

    public var body: some View {
        ZStack(alignment: .top) {
            Color.black.ignoresSafeArea()
            if let reproductor {
                ReproductorConPiP(reproductor: reproductor).ignoresSafeArea()
            }
            BarraVisorIPad(tipo: "Vídeo", icono: "play.rectangle", titulo: titulo,
                           alFijar: { _ = GestorPinesIPad.compartido.pinear(url: url) })
        }
        .onAppear {
            try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback)
            let r = AVPlayer(url: url)
            reproductor = r
            r.play()
        }
        .onDisappear { reproductor?.pause() }
    }
}

/// `AVPlayerViewController` con Picture in Picture encendido.
struct ReproductorConPiP: UIViewControllerRepresentable {
    let reproductor: AVPlayer

    func makeUIViewController(context: Context) -> AVPlayerViewController {
        let c = AVPlayerViewController()
        c.player = reproductor
        c.allowsPictureInPicturePlayback = true
        c.canStartPictureInPictureAutomaticallyFromInline = true
        return c
    }

    func updateUIViewController(_ c: AVPlayerViewController, context: Context) {
        c.player = reproductor
    }
}

// MARK: - Vista Rapida

/// Lo que no tiene visor propio, con Vista Rapida (`QLPreviewController`).
public struct VistaRapida: View {
    let url: URL

    public init(url: URL) { self.url = url }

    public var body: some View {
        ZStack(alignment: .top) {
            VistaRapidaUIKit(url: url).ignoresSafeArea(edges: .bottom)
                .padding(.top, 56)
            BarraVisorIPad(tipo: "Vista rápida", icono: "eye", titulo: url.lastPathComponent)
        }
    }
}

struct VistaRapidaUIKit: UIViewControllerRepresentable {
    let url: URL

    func makeCoordinator() -> Fuente { Fuente(url: url) }

    func makeUIViewController(context: Context) -> QLPreviewController {
        let c = QLPreviewController()
        c.dataSource = context.coordinator
        return c
    }

    func updateUIViewController(_ c: QLPreviewController, context: Context) {
        if context.coordinator.url != url {
            context.coordinator.url = url
            c.reloadData()
        }
    }

    final class Fuente: NSObject, QLPreviewControllerDataSource {
        var url: URL
        init(url: URL) { self.url = url }
        func numberOfPreviewItems(in controller: QLPreviewController) -> Int { 1 }
        func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
            url as NSURL
        }
    }
}

// MARK: - Abrir desde fuera

/// Lo que llega de otras apps: un `pixpinmax://` o un fichero abierto con
/// PixPin (Archivos, «Abrir en...»). Un fichero se copia a «Mensajes
/// guardados» y se abre con su visor.
@MainActor
public enum AbrirDesdeFuera {
    /// Lo que hace un enlace `pixpinmax://` (los pedidos de Windows: nueva
    /// nota, abrir proyecto...). Lo pone quien sabe hacer los pedidos.
    public static var alEnlace: ((URL, Almacen) -> Void)?

    public static func abrir(_ url: URL, entorno: EntornoApp) {
        guard let almacen = entorno.almacen else { return }
        if url.scheme == "pixpinmax" {
            alEnlace?(url, almacen)
            return
        }
        guard url.isFileURL else { return }
        let accedido = url.startAccessingSecurityScopedResource()
        Task {
            defer { if accedido { url.stopAccessingSecurityScopedResource() } }
            await almacen.cargarChats()
            guard let guardados = almacen.chats.first(where: \.esGuardados)?.id else { return }
            // Cada tipo a su sitio (`.excalidraw` como lienzo, LandXML al
            // 3D, `.pixpin`...): `Sistema/AbrirFicheroDeFuera.swift`.
            await AbrirFicheroDeFuera.abrir(url, en: guardados, almacen: almacen)
        }
    }
}
