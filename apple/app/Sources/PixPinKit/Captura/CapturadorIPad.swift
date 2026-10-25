// La captura del iPad. iOS no deja capturar otras apps (ni la pantalla
// entera), asi que lo que en Windows es «elegir una zona de la pantalla»
// aqui entra por cinco sitios:
//
// - **Hacer foto**: la camara (`UIImagePickerController`).
// - **Escanear documento**: el escaner de VisionKit
//   (`VNDocumentCameraViewController`), con las paginas enderezadas; si se
//   escanean varias, se editan una detras de otra.
// - **Desde Fotos**: el selector del sistema (`PHPickerViewController`, sin
//   pedir permiso a la fototeca), una o varias.
// - **Pegar**: la imagen del portapapeles.
// - **Capturar PixPin**: la ventana de la propia app
//   (`UIGraphicsImageRenderer` + `drawHierarchy`), con sus pines.
//
// Cada una acaba en el editor de captura a pantalla completa
// (`EditorCapturaIPad`): anotar y luego Copiar, Pinear, Al chat, Texto o
// Guardar en la galeria de capturas (`capturas/` de los datos, la carpeta
// que da el nucleo, `Nucleo.carpetaCapturas`, la misma que Windows y el Mac).
//
// Puntos de entrada publicos: `BotonCapturar()` (un menu con las cinco) y
// `CapturadorIPad.compartido.empezar()` (una hoja para elegir).

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import VisionKit

@MainActor
public final class CapturadorIPad: ObservableObject {
    public static let compartido = CapturadorIPad()

    /// De donde sale la captura.
    public enum Fuente: String, CaseIterable, Identifiable, Sendable {
        case camara, escaner, fotos, pegar, app

        public var id: String { rawValue }

        public var titulo: String {
            switch self {
            case .camara: "Hacer foto"
            case .escaner: "Escanear documento"
            case .fotos: "Desde Fotos"
            case .pegar: "Pegar"
            case .app: "Capturar PixPin"
            }
        }

        public var icono: String {
            switch self {
            case .camara: "camera"
            case .escaner: "doc.viewfinder"
            case .fotos: "photo.on.rectangle"
            case .pegar: "doc.on.clipboard"
            case .app: "rectangle.dashed"
            }
        }

        /// Si este iPad la tiene (un simulador no tiene camara).
        @MainActor
        public var disponible: Bool {
            switch self {
            case .camara: UIImagePickerController.isSourceTypeAvailable(.camera)
            case .escaner: VNDocumentCameraViewController.isSupported
            case .fotos, .app: true
            case .pegar: UIPasteboard.general.hasImages
            }
        }
    }

    /// El almacen de la app (para «Al chat» y la carpeta de capturas). Lo
    /// pone la capa de pines al arrancar.
    weak var almacen: Almacen?
    /// Avisa de cada captura guardada en la galeria (la galeria ya vigila la
    /// carpeta; esto es para quien quiera enterarse, como las
    /// automatizaciones «al capturar»).
    public var alGuardar: ((URL) -> Void)?

    /// Las fotos que esperan su turno en el editor (varias paginas del
    /// escaner o varias de Fotos).
    private var pendientes: [CGImage] = []
    private var editorAbierto = false
    private var delegado: DelegadoCaptura?

    public init() {}

    /// La hoja para elegir de donde capturar.
    public func empezar() {
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 380, height: 420)) { cerrar in
            VistaElegirCaptura { fuente in
                cerrar()
                guard let fuente else { return }
                // Que se vaya la hoja antes de abrir la camara (o de fotografiar la app).
                Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(450))
                    CapturadorIPad.compartido.capturar(fuente)
                }
            }
        }
    }

    /// Captura desde `fuente`.
    public func capturar(_ fuente: Fuente) {
        switch fuente {
        case .camara:
            guard UIImagePickerController.isSourceTypeAvailable(.camera) else { return avisarFalloPin() }
            let c = UIImagePickerController()
            c.sourceType = .camera
            c.cameraCaptureMode = .photo
            let d = DelegadoCaptura()
            delegado = d
            c.delegate = d
            c.modalPresentationStyle = .fullScreen
            ArribaUIKit.presentar(c)
        case .escaner:
            guard VNDocumentCameraViewController.isSupported else { return avisarFalloPin() }
            let c = VNDocumentCameraViewController()
            let d = DelegadoCaptura()
            delegado = d
            c.delegate = d
            c.modalPresentationStyle = .fullScreen
            ArribaUIKit.presentar(c)
        case .fotos:
            var ajustes = PHPickerConfiguration()
            ajustes.filter = .images
            ajustes.selectionLimit = 10
            ajustes.preferredAssetRepresentationMode = .current
            let c = PHPickerViewController(configuration: ajustes)
            let d = DelegadoCaptura()
            delegado = d
            c.delegate = d
            ArribaUIKit.presentar(c)
        case .pegar:
            guard let foto = UIPasteboard.general.image else { return avisarFalloPin() }
            editar(foto)
        case .app:
            capturarApp()
        }
    }

    /// La ventana de PixPin tal como se ve (con sus pines).
    public func capturarApp() {
        guard let ventana = ArribaUIKit.ventana() else { return avisarFalloPin() }
        let formato = UIGraphicsImageRendererFormat()
        formato.scale = ventana.screen.scale
        formato.opaque = true
        let foto = UIGraphicsImageRenderer(bounds: ventana.bounds, format: formato).image { _ in
            ventana.drawHierarchy(in: ventana.bounds, afterScreenUpdates: false)
        }
        // Un destello, como la captura del sistema.
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        editar(foto)
    }

    /// Abre el editor con una foto de UIKit.
    public func editar(_ foto: UIImage) {
        guard let cg = ImagenIPad.derecha(foto) else { return avisarFalloPin() }
        editar(cg)
    }

    /// Abre el editor (o la deja esperando si ya hay uno abierto). Con
    /// `alAnotarPin`, «Listo» devuelve la foto anotada al pin.
    func editar(_ imagen: CGImage, alAnotarPin: ((CGImage) -> Void)? = nil) {
        if editorAbierto, alAnotarPin == nil {
            pendientes.append(imagen)
            return
        }
        editorAbierto = true
        let modo: ModoEditorCaptura
        if let alAnotarPin { modo = .anotarPin(alAnotarPin) } else { modo = .captura }
        ArribaUIKit.presentar(estilo: .fullScreen) { cerrar in
            EditorCapturaIPad(imagen: imagen, modo: modo) {
                cerrar()
                CapturadorIPad.compartido.editorCerrado()
            }
        }
    }

    /// Ya se cerro el editor: la siguiente que espere, si la hay.
    private func editorCerrado() {
        editorAbierto = false
        guard !pendientes.isEmpty else { return }
        let siguiente = pendientes.removeFirst()
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(500))
            CapturadorIPad.compartido.editar(siguiente)
        }
    }

    /// Varias a la vez (escaner, Fotos): la primera al editor, las demas
    /// detras.
    fileprivate func recibir(_ fotos: [CGImage]) {
        delegado = nil
        guard let primera = fotos.first else { return }
        pendientes.append(contentsOf: fotos.dropFirst())
        editar(primera)
    }

    fileprivate func cancelado() {
        delegado = nil
    }

    // MARK: Galeria de capturas

    /// La carpeta de la galeria: la que dice el nucleo (`capturas/`).
    var carpetaDeCapturas: URL {
        if let almacen { return URL(fileURLWithPath: almacen.nucleo.carpetaCapturas(), isDirectory: true) }
        return carpetaDeDatos().appendingPathComponent("capturas", isDirectory: true)
    }

    /// Guarda la captura en la galeria como PNG con el nombre de la hora
    /// (`captura-AAAAMMDD-HHMMSS.png`, como Windows). Devuelve donde.
    @discardableResult
    public func guardarEnGaleria(_ imagen: CGImage) async -> URL? {
        let carpeta = carpetaDeCapturas
        let fecha = Date()
        let hecha = await ImagenIPad.fuera { () -> URL? in
            guard let png = ImagenIPad.png(imagen) else { return nil }
            do {
                try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                let destino = NombresCaptura.rutaNueva(en: carpeta, fecha: fecha)
                try png.write(to: destino, options: .atomic)
                return destino
            } catch {
                return nil
            }
        }
        if let hecha { alGuardar?(hecha) }
        return hecha
    }
}

/// Lo que contestan la camara, el escaner y Fotos. UIKit los llama en el
/// hilo principal.
private final class DelegadoCaptura: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate,
    VNDocumentCameraViewControllerDelegate, PHPickerViewControllerDelegate {
    // Camara

    func imagePickerController(_ picker: UIImagePickerController,
                               didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
        let foto = info[.originalImage] as? UIImage
        MainActor.assumeIsolated {
            picker.dismiss(animated: true) {
                MainActor.assumeIsolated {
                    let cg = foto.flatMap { ImagenIPad.derecha($0) }
                    CapturadorIPad.compartido.recibir(cg.map { [$0] } ?? [])
                }
            }
        }
    }

    func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
        MainActor.assumeIsolated {
            picker.dismiss(animated: true)
            CapturadorIPad.compartido.cancelado()
        }
    }

    // Escaner

    func documentCameraViewController(_ controller: VNDocumentCameraViewController,
                                      didFinishWith scan: VNDocumentCameraScan) {
        MainActor.assumeIsolated {
            var paginas: [CGImage] = []
            for i in 0 ..< scan.pageCount {
                if let cg = ImagenIPad.derecha(scan.imageOfPage(at: i)) { paginas.append(cg) }
            }
            controller.dismiss(animated: true) {
                MainActor.assumeIsolated { CapturadorIPad.compartido.recibir(paginas) }
            }
        }
    }

    func documentCameraViewControllerDidCancel(_ controller: VNDocumentCameraViewController) {
        MainActor.assumeIsolated {
            controller.dismiss(animated: true)
            CapturadorIPad.compartido.cancelado()
        }
    }

    func documentCameraViewController(_ controller: VNDocumentCameraViewController, didFailWithError error: Error) {
        MainActor.assumeIsolated {
            controller.dismiss(animated: true)
            CapturadorIPad.compartido.cancelado()
            avisarFalloPin()
        }
    }

    // Fotos

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        let proveedores = results.map(\.itemProvider)
        MainActor.assumeIsolated {
            picker.dismiss(animated: true)
        }
        guard !proveedores.isEmpty else {
            MainActor.assumeIsolated { CapturadorIPad.compartido.cancelado() }
            return
        }
        Task { @MainActor in
            var fotos: [CGImage] = []
            for p in proveedores {
                if let datos = await DelegadoCaptura.datos(de: p),
                   let cg = await Miniatura.fueraDelPrincipal({ Miniatura.de(datos: datos, ladoMaximo: 6000) }) {
                    fotos.append(cg)
                }
            }
            if fotos.isEmpty { avisarFalloPin() }
            CapturadorIPad.compartido.recibir(fotos)
        }
    }

    /// Los bytes de la foto (HEIC, JPEG, PNG...) tal cual, para leerla con
    /// ImageIO ya derecha.
    private static func datos(de proveedor: NSItemProvider) async -> Data? {
        await withCheckedContinuation { (c: CheckedContinuation<Data?, Never>) in
            proveedor.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { datos, _ in
                c.resume(returning: datos)
            }
        }
    }
}

/// La hoja de elegir: las cinco, grandes, para el dedo.
struct VistaElegirCaptura: View {
    let alElegir: (CapturadorIPad.Fuente?) -> Void

    var body: some View {
        NavigationStack {
            List {
                ForEach(CapturadorIPad.Fuente.allCases) { f in
                    Button { alElegir(f) } label: {
                        Label(f.titulo, systemImage: f.icono)
                            .font(.title3)
                            .padding(.vertical, 8)
                    }
                    .disabled(!f.disponible)
                }
                Section {
                    Text("iPadOS no deja capturar otras apps: haz una foto, escanea, elige de Fotos, pega o captura PixPin.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Capturar")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancelar") { alElegir(nil) }.keyboardShortcut(.cancelAction)
                }
            }
        }
    }
}

/// **El boton de capturar** para la barra lateral o la barra de
/// herramientas: un menu con las cinco entradas.
public struct BotonCapturar: View {
    /// Con el titulo («Capturar») o solo el icono.
    let conTitulo: Bool

    public init(conTitulo: Bool = true) {
        self.conTitulo = conTitulo
    }

    public var body: some View {
        Menu {
            ForEach(CapturadorIPad.Fuente.allCases) { f in
                Button {
                    // Que se cierre el menu antes de fotografiar la app.
                    Task { @MainActor in
                        try? await Task.sleep(for: .milliseconds(f == .app ? 350 : 50))
                        CapturadorIPad.compartido.capturar(f)
                    }
                } label: {
                    Label(f.titulo, systemImage: f.icono)
                }
                .disabled(!f.disponible)
            }
        } label: {
            if conTitulo {
                Label("Capturar", systemImage: "camera.viewfinder")
            } else {
                Image(systemName: "camera.viewfinder")
            }
        }
        .help("Capturar")
        .accessibilityLabel("Capturar")
    }
}
