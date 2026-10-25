// La extension de Compartir del iPad: mandar a un proyecto de PixPin desde
// cualquier app (Fotos, Safari, Archivos, la captura de pantalla del
// sistema...). Es el `compartir.rs`/«Imprimir a PixPin» de Windows y la
// `CompartirPixPin` del Mac, y la alternativa del iPad a capturar otras apps.
//
// Se elige el proyecto (con buscador), se puede escribir un comentario y
// «Enviar» deja lo compartido en el buzon del App Group
// (`BuzonCompartido`, el mismo fichero que compila la app): los ficheros y
// las fotos tal cual, y el texto y los enlaces para la nota. La app lo mete
// en el proyecto al abrirse (`Sistema/RecogedorCompartido.swift`), como si
// se hubiera soltado en el chat. La extension no abre la carpeta de datos
// ni lleva el nucleo: esa carpeta esta en la caja de arena de la app.
//
// Comentarios sin tildes como el resto; textos de la interfaz con tildes.

import SwiftUI
import UIKit
import UniformTypeIdentifiers

@objc(ControladorCompartir)
final class ControladorCompartir: UIViewController {
    private let modelo = ModeloCompartir()

    override func viewDidLoad() {
        super.viewDidLoad()
        modelo.alAcabar = { [weak self] enviado in
            guard let self else { return }
            if enviado {
                self.extensionContext?.completeRequest(returningItems: nil, completionHandler: nil)
            } else {
                self.extensionContext?.cancelRequest(withError: NSError(domain: NSCocoaErrorDomain, code: NSUserCancelledError))
            }
        }
        modelo.items = (extensionContext?.inputItems as? [NSExtensionItem]) ?? []
        let vista = UIHostingController(rootView: VistaCompartir(modelo: modelo))
        addChild(vista)
        vista.view.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(vista.view)
        NSLayoutConstraint.activate([
            vista.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            vista.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            vista.view.topAnchor.constraint(equalTo: view.topAnchor),
            vista.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        vista.didMove(toParent: self)
        preferredContentSize = CGSize(width: 540, height: 620)
        modelo.cargar()
    }
}

@MainActor
final class ModeloCompartir: ObservableObject {
    @Published var proyectos: [ProyectoCompartido] = []
    @Published var buscando = ""
    @Published var elegido: String?
    @Published var comentario = ""
    @Published var estado: String?
    @Published var enviando = false
    /// Sin App Group (compilado sin firmar) o sin abrir PixPin nunca.
    @Published var sinBuzon = false
    /// Que se comparte, para la cabecera («2 fotos y un enlace»).
    @Published var resumen = ""

    var items: [NSExtensionItem] = [] {
        didSet { resumen = ModeloCompartir.resumen(de: items) }
    }
    var alAcabar: ((Bool) -> Void)?

    var visibles: [ProyectoCompartido] { BuzonCompartido.ordenados(proyectos, buscando: buscando) }

    func cargar() {
        guard let raiz = BuzonCompartido.raiz() else {
            sinBuzon = true
            estado = "Esta copia de PixPin no tiene el grupo de apps (se instaló sin firmar): fírmala con tu Apple ID para poder compartir."
            return
        }
        // Lo escrito en la hoja de compartir (Safari pone ahi el titulo).
        let escrito = items.compactMap { $0.attributedContentText?.string }
            .joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        if !escrito.isEmpty { comentario = escrito }
        Task {
            let lista = await Task.detached(priority: .userInitiated) { BuzonCompartido.leerProyectos(en: raiz) }.value
            proyectos = lista
            if lista.isEmpty {
                sinBuzon = true
                estado = "Abre PixPin una vez en este iPad antes de compartir."
            } else {
                elegido = BuzonCompartido.ordenados(lista).first?.id
                // El ultimo proyecto elegido, si sigue.
                if let ultimo = UserDefaults(suiteName: BuzonCompartido.grupo)?.string(forKey: "compartir.ultimo"),
                   lista.contains(where: { $0.id == ultimo }) {
                    elegido = ultimo
                }
            }
        }
    }

    func cancelar() { alAcabar?(false) }

    func enviar() {
        guard let raiz = BuzonCompartido.raiz(), let chat = elegido,
              let p = proyectos.first(where: { $0.id == chat }) else { return }
        enviando = true
        estado = "Preparando…"
        let items = self.items
        let comentario = self.comentario
        Task {
            do {
                let carpeta = try BuzonCompartido.nuevaEntrega(en: raiz)
                var textos: [String] = []
                var ficheros: [String] = []
                for item in items {
                    for proveedor in item.attachments ?? [] {
                        switch await Entrega.leer(proveedor, en: carpeta) {
                        case let .fichero(n): ficheros.append(n)
                        case let .texto(t): textos.append(t)
                        case .nada: break
                        }
                    }
                }
                let ms = Int64(Date().timeIntervalSince1970 * 1000)
                let e = EntregaCompartida(chat: chat, nombreDelChat: p.nombre, comentario: comentario,
                                          textos: textos, ficheros: ficheros, creada: ms)
                guard !ficheros.isEmpty || BuzonCompartido.textoDeLaNota(e) != nil else {
                    carpeta.tirar()
                    estado = "No hay nada que mandar."
                    enviando = false
                    return
                }
                try carpeta.cerrar(e)
                UserDefaults(suiteName: BuzonCompartido.grupo)?.set(chat, forKey: "compartir.ultimo")
                // Por si PixPin esta delante (Split View): que lo recoja ya.
                CFNotificationCenterPostNotification(CFNotificationCenterGetDarwinNotifyCenter(),
                                                     CFNotificationName(BuzonCompartido.avisoDeEntrega as CFString),
                                                     nil, nil, true)
                alAcabar?(true)
            } catch {
                estado = "No se pudo preparar: \(error.localizedDescription)"
                enviando = false
            }
        }
    }

    /// «2 fotos, 1 enlace»: lo que se va a mandar.
    static func resumen(de items: [NSExtensionItem]) -> String {
        var fotos = 0, videos = 0, enlaces = 0, textos = 0, otros = 0
        for p in items.flatMap({ $0.attachments ?? [] }) {
            if p.hasItemConformingToTypeIdentifier(UTType.image.identifier) { fotos += 1 }
            else if p.hasItemConformingToTypeIdentifier(UTType.movie.identifier) { videos += 1 }
            else if p.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) { otros += 1 }
            else if p.hasItemConformingToTypeIdentifier(UTType.url.identifier) { enlaces += 1 }
            else if p.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) { textos += 1 }
            else { otros += 1 }
        }
        func n(_ c: Int, _ uno: String, _ varios: String) -> String? {
            c == 0 ? nil : (c == 1 ? "1 \(uno)" : "\(c) \(varios)")
        }
        return [n(fotos, "foto", "fotos"), n(videos, "vídeo", "vídeos"), n(otros, "archivo", "archivos"),
                n(enlaces, "enlace", "enlaces"), n(textos, "texto", "textos")]
            .compactMap { $0 }.joined(separator: ", ")
    }
}

/// Un adjunto de la hoja de compartir, dentro de la entrega.
enum Entrega {
    enum Leido { case fichero(String), texto(String), nada }

    /// Lo que se guarda como fichero aunque venga tambien como enlace (un
    /// PDF abierto en Safari), del mas concreto al mas general.
    static let tiposFuertes: [UTType] = [.pdf, .image, .movie, .audio]

    static func leer(_ p: NSItemProvider, en carpeta: CarpetaDeEntrega) async -> Leido {
        let registrados = p.registeredTypeIdentifiers.compactMap { UTType($0) }
        // 1. Una foto, un PDF, un video o un audio: tal cual.
        for general in tiposFuertes {
            guard let tipo = registrados.first(where: { $0.conforms(to: general) }) else { continue }
            if let n = await copiarFichero(p, tipo, en: carpeta) { return .fichero(n) }
            if let n = await copiarDatos(p, tipo, en: carpeta) { return .fichero(n) }
        }
        // 2. Un fichero de Archivos u otra app.
        if p.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier),
           let item = try? await cargar(p, .fileURL) {
            var url = item as? URL
            if url == nil, let d = item as? Data { url = URL(dataRepresentation: d, relativeTo: nil) }
            if let url, url.isFileURL {
                let accedido = url.startAccessingSecurityScopedResource()
                defer { if accedido { url.stopAccessingSecurityScopedResource() } }
                if let n = try? carpeta.anadir(fichero: url) { return .fichero(n) }
            }
        }
        // 3. Un enlace web: a la nota.
        if p.hasItemConformingToTypeIdentifier(UTType.url.identifier), let item = try? await cargar(p, .url) {
            if let url = item as? URL, !url.isFileURL { return .texto(url.absoluteString) }
            if let d = item as? Data, let url = URL(dataRepresentation: d, relativeTo: nil), !url.isFileURL {
                return .texto(url.absoluteString)
            }
        }
        // 4. Cualquier otro fichero (Word, Excel, planos...), sin los textos.
        if let tipo = registrados.first(where: {
            $0.conforms(to: .data) && !$0.conforms(to: .plainText) && !$0.conforms(to: .url)
        }) {
            if let n = await copiarFichero(p, tipo, en: carpeta) { return .fichero(n) }
            if let n = await copiarDatos(p, tipo, en: carpeta) { return .fichero(n) }
        }
        // 5. Una foto que solo viene como `UIImage` (algunas apps).
        if p.canLoadObject(ofClass: UIImage.self), let foto = await cargarFoto(p), let png = foto.pngData(),
           let n = try? carpeta.anadir(datos: png, nombre: Entrega.nombre(p, .png)) {
            return .fichero(n)
        }
        // 6. Un texto suelto: a la nota.
        if p.hasItemConformingToTypeIdentifier(UTType.plainText.identifier), let item = try? await cargar(p, .plainText) {
            if let s = item as? String { return .texto(s) }
            if let a = item as? NSAttributedString { return .texto(a.string) }
            if let d = item as? Data, let s = String(data: d, encoding: .utf8) { return .texto(s) }
        }
        return .nada
    }

    /// El nombre del adjunto con su extension (`compartido-<hora>.<ext>` si
    /// no trae).
    static func nombre(_ p: NSItemProvider, _ tipo: UTType) -> String {
        let ext = tipo.preferredFilenameExtension ?? "dat"
        if let s = p.suggestedName, !s.isEmpty {
            return (s as NSString).pathExtension.isEmpty ? "\(s).\(ext)" : s
        }
        return "compartido-\(Int64(Date().timeIntervalSince1970 * 1000)).\(ext)"
    }

    /// El fichero, copiado dentro mientras iOS lo deja abierto.
    static func copiarFichero(_ p: NSItemProvider, _ tipo: UTType, en carpeta: CarpetaDeEntrega) async -> String? {
        await withCheckedContinuation { (c: CheckedContinuation<String?, Never>) in
            _ = p.loadFileRepresentation(forTypeIdentifier: tipo.identifier) { url, _ in
                guard let url else { return c.resume(returning: nil) }
                // El nombre que da la app (sin extension, se le pone la del
                // fichero); si no, el del fichero.
                var n = url.lastPathComponent
                if let s = p.suggestedName, !s.isEmpty {
                    n = (s as NSString).pathExtension.isEmpty && !url.pathExtension.isEmpty ? "\(s).\(url.pathExtension)" : s
                }
                c.resume(returning: try? carpeta.anadir(fichero: url, nombre: n))
            }
        }
    }

    /// Los bytes (lo que no viene como fichero, p. ej. una captura recien
    /// hecha), escritos dentro.
    static func copiarDatos(_ p: NSItemProvider, _ tipo: UTType, en carpeta: CarpetaDeEntrega) async -> String? {
        await withCheckedContinuation { (c: CheckedContinuation<String?, Never>) in
            _ = p.loadDataRepresentation(forTypeIdentifier: tipo.identifier) { datos, _ in
                guard let datos else { return c.resume(returning: nil) }
                c.resume(returning: try? carpeta.anadir(datos: datos, nombre: Entrega.nombre(p, tipo)))
            }
        }
    }

    static func cargarFoto(_ p: NSItemProvider) async -> UIImage? {
        await withCheckedContinuation { (c: CheckedContinuation<UIImage?, Never>) in
            _ = p.loadObject(ofClass: UIImage.self) { objeto, _ in c.resume(returning: objeto as? UIImage) }
        }
    }

    static func cargar(_ p: NSItemProvider, _ tipo: UTType) async throws -> NSSecureCoding? {
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<NSSecureCoding?, Error>) in
            p.loadItem(forTypeIdentifier: tipo.identifier, options: nil) { item, error in
                if let error { c.resume(throwing: error) } else { c.resume(returning: item) }
            }
        }
    }
}

/// La hoja: lo que se manda, el buscador, la lista de proyectos y el
/// comentario.
struct VistaCompartir: View {
    @ObservedObject var modelo: ModeloCompartir
    @FocusState private var escribiendo: Bool

    var body: some View {
        NavigationStack {
            List {
                if !modelo.resumen.isEmpty {
                    Section {
                        Label(modelo.resumen, systemImage: "square.and.arrow.down")
                            .foregroundStyle(.secondary)
                    }
                }
                Section("Comentario") {
                    TextField("Escribe algo para acompañarlo (opcional)", text: $modelo.comentario, axis: .vertical)
                        .lineLimit(1...4)
                        .focused($escribiendo)
                }
                Section("Proyecto") {
                    if modelo.visibles.isEmpty, !modelo.sinBuzon, !modelo.proyectos.isEmpty {
                        Text("Ningún proyecto se llama así.").foregroundStyle(.secondary)
                    }
                    ForEach(modelo.visibles) { p in
                        Button {
                            modelo.elegido = p.id
                        } label: {
                            HStack {
                                Image(systemName: p.esGuardados ? "bookmark" : "bubble.left.and.bubble.right")
                                    .foregroundStyle(.tint)
                                    .frame(width: 24)
                                Text(p.nombre).foregroundStyle(.primary).lineLimit(1)
                                Spacer()
                                if modelo.elegido == p.id {
                                    Image(systemName: "checkmark").fontWeight(.semibold).foregroundStyle(.tint)
                                }
                            }
                        }
                    }
                }
                if let estado = modelo.estado {
                    Section { Text(estado).font(.footnote).foregroundStyle(.secondary) }
                }
            }
            .searchable(text: $modelo.buscando, placement: .navigationBarDrawer(displayMode: .always),
                        prompt: "Buscar proyecto")
            .navigationTitle("Mandar a PixPin")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancelar") { modelo.cancelar() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    if modelo.enviando {
                        ProgressView()
                    } else {
                        Button("Enviar") { modelo.enviar() }
                            .fontWeight(.semibold)
                            .disabled(modelo.elegido == nil || modelo.sinBuzon)
                    }
                }
            }
        }
    }
}
