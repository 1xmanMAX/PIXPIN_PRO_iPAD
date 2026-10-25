// **Leer un QR con la camara del iPad** (lo que en el movil hace ZXing,
// `Qr.kt`): el del grupo, el de un envio con codigo (`pixpin-envio:1:…`,
// `pixpin-recibe:1:…`) o el de una invitacion a un chat compartido
// (`pixpin-chat:1:…`). En Windows no hay camara y se pega el texto del QR;
// aqui se puede hacer las dos cosas.
//
// Con `AVCaptureMetadataOutput` (AVFoundation) y no con
// `DataScannerViewController` de VisionKit: este va en cualquier iPad con
// camara (VisionKit pide un A12 o mas nuevo) y no necesita el motor de texto.
// La sesion de la camara arranca y para fuera del hilo principal (Apple lo
// pide: `startRunning` bloquea).

import AVFoundation
import SwiftUI
import UIKit

/// La hoja de escanear: la camara a pantalla, un marco, el texto de ayuda y
/// «Cancelar». `acepta` dice si un texto leido vale (los que no, se
/// ignoran y se sigue mirando); el primero que vale se devuelve y la hoja se
/// cierra.
public struct HojaEscanerQR: View {
    let titulo: String
    let ayuda: String
    let acepta: (String) -> Bool
    let alLeer: (String) -> Void
    @Environment(\.dismiss) private var cerrar
    @State private var permiso = AVCaptureDevice.authorizationStatus(for: .video)
    @State private var noVale: String?

    public init(titulo: String, ayuda: String, acepta: @escaping (String) -> Bool,
                alLeer: @escaping (String) -> Void) {
        self.titulo = titulo
        self.ayuda = ayuda
        self.acepta = acepta
        self.alLeer = alLeer
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                Color.black.ignoresSafeArea()
                switch permiso {
                case .authorized:
                    CamaraQR { texto in
                        if acepta(texto) {
                            alLeer(texto)
                            cerrar()
                        } else {
                            noVale = "Ese QR no es de PixPin para esto."
                        }
                    }
                    .ignoresSafeArea()
                    RoundedRectangle(cornerRadius: 18)
                        .stroke(Color.white.opacity(0.9), lineWidth: 3)
                        .frame(width: 260, height: 260)
                case .notDetermined:
                    ProgressView().tint(.white)
                default:
                    sinPermiso
                }
                VStack {
                    Spacer()
                    VStack(spacing: 6) {
                        Text(ayuda)
                        if let noVale { Text(noVale).foregroundStyle(Paleta.v2.amarillo) }
                    }
                    .font(.callout)
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.white)
                    .padding(14)
                    .background(.black.opacity(0.55), in: RoundedRectangle(cornerRadius: 12))
                    .padding(24)
                }
            }
            .navigationTitle(titulo)
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancelar") { cerrar() }
                }
            }
        }
        .task {
            guard permiso == .notDetermined else { return }
            let si = await AVCaptureDevice.requestAccess(for: .video)
            permiso = si ? .authorized : .denied
        }
    }

    private var sinPermiso: some View {
        VStack(spacing: 14) {
            Image(systemName: "camera.fill").font(.largeTitle)
            Text("PixPin no puede usar la cámara.")
                .font(.headline)
            Text("Para escanear el QR, permite la cámara en Ajustes › PixPin. También puedes escribir el código a mano.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.white.opacity(0.8))
            Button("Abrir Ajustes") {
                if let u = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(u) }
            }
            .buttonStyle(.borderedProminent)
        }
        .foregroundStyle(.white)
        .padding(40)
    }
}

/// La vista de la camara que avisa con el texto de cada QR que ve.
struct CamaraQR: UIViewControllerRepresentable {
    let alVer: (String) -> Void

    func makeUIViewController(context: Context) -> ControladorCamaraQR {
        let c = ControladorCamaraQR()
        c.alVer = alVer
        return c
    }

    func updateUIViewController(_ c: ControladorCamaraQR, context: Context) {
        c.alVer = alVer
    }

    static func dismantleUIViewController(_ c: ControladorCamaraQR, coordinator: ()) {
        c.parar()
    }
}

/// La sesion de la camara con su capa de vista previa.
final class ControladorCamaraQR: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var alVer: ((String) -> Void)?
    private let sesion = AVCaptureSession()
    private let cola = DispatchQueue(label: "pixpin.camara.qr", qos: .userInitiated)
    private var previa: AVCaptureVideoPreviewLayer?
    /// El ultimo texto avisado y cuando: el mismo QR se ve 30 veces por
    /// segundo y basta con avisarlo una vez cada poco.
    private var ultimo: (String, Date)?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        montar()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previa?.frame = view.bounds
        orientar()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        let sesion = self.sesion
        cola.async { if !sesion.isRunning { sesion.startRunning() } }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        parar()
    }

    func parar() {
        let sesion = self.sesion
        cola.async { if sesion.isRunning { sesion.stopRunning() } }
    }

    private func montar() {
        // La de atras si hay; si no (algun iPad), la de delante.
        let camara = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back)
            ?? AVCaptureDevice.default(for: .video)
        guard let camara, let entrada = try? AVCaptureDeviceInput(device: camara),
              sesion.canAddInput(entrada) else { return }
        sesion.beginConfiguration()
        sesion.addInput(entrada)
        let salida = AVCaptureMetadataOutput()
        if sesion.canAddOutput(salida) {
            sesion.addOutput(salida)
            salida.setMetadataObjectsDelegate(self, queue: .main)
            if salida.availableMetadataObjectTypes.contains(.qr) {
                salida.metadataObjectTypes = [.qr]
            }
        }
        sesion.commitConfiguration()
        let capa = AVCaptureVideoPreviewLayer(session: sesion)
        capa.videoGravity = .resizeAspectFill
        capa.frame = view.bounds
        view.layer.addSublayer(capa)
        previa = capa
    }

    /// La vista previa gira con el iPad (en horizontal casi siempre).
    private func orientar() {
        guard let conexion = previa?.connection, let escena = view.window?.windowScene else { return }
        let angulo: CGFloat
        switch escena.interfaceOrientation {
        case .landscapeLeft: angulo = 180
        case .landscapeRight: angulo = 0
        case .portraitUpsideDown: angulo = 270
        default: angulo = 90
        }
        if conexion.isVideoRotationAngleSupported(angulo) {
            conexion.videoRotationAngle = angulo
        }
    }

    /// Llega en la cola principal (`setMetadataObjectsDelegate(queue: .main)`).
    nonisolated func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objetos: [AVMetadataObject],
                                    from connection: AVCaptureConnection) {
        let textos = objetos.compactMap { o -> String? in
            guard let qr = o as? AVMetadataMachineReadableCodeObject, qr.type == .qr,
                  let texto = qr.stringValue, !texto.isEmpty else { return nil }
            return texto
        }
        MainActor.assumeIsolated { self.vistos(textos) }
    }

    private func vistos(_ textos: [String]) {
        for texto in textos {
            if let ultimo, ultimo.0 == texto, Date().timeIntervalSince(ultimo.1) < 2 { continue }
            ultimo = (texto, Date())
            alVer?(texto)
            return
        }
    }
}
