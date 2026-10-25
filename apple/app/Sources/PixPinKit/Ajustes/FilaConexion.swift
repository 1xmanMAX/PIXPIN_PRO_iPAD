// Ajustes → Asistente → Conexiones (Windows, 17-oct): el ID de cliente que
// el usuario crea una vez, «Conectar» (abre el navegador y espera la
// vuelta) y «Desconectar». Los tokens van al Llavero.
//
// iPad: la pagina de permiso de Google se abre en una
// `ASWebAuthenticationSession` (la hoja de Safari para iniciar sesion, que
// Google admite; un `WKWebView` no). La vuelta es la de Windows y el Mac: el
// nucleo escucha en `http://127.0.0.1:<puerto>/` mientras tanto (la app esta
// delante, asi que el puerto local contesta) y, al llegar el permiso, se
// cierra la hoja. Para eso el nucleo tiene que dar la URL a Swift en vez de
// abrirla el con `open` (que en iOS no existe): ver `AbridorDeConexion`.

import AuthenticationServices
import PixPinCore
import SwiftUI
import UIKit

struct FilaConexion: View {
    let id: String
    @State private var estado: ConexionIa?
    @State private var pegado = ""
    @State private var conectando = false
    @State private var aviso: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let e = estado {
                if e.conectado {
                    Text("Conectado\(e.cuenta.map { " como \($0)" } ?? "")").foregroundStyle(.green)
                } else if let c = e.cliente {
                    Text("ID de cliente: \(c)").font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            HStack {
                TextField("ID de cliente (o el JSON de Google)", text: $pegado)
                    .textFieldStyle(.roundedBorder)
                Button("Guardar") { guardar() }.disabled(pegado.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            HStack {
                if conectando {
                    ProgressView().controlSize(.small)
                    Text("Esperando al navegador…").font(.caption)
                    Button("Cancelar") { iaCancelarConexion() }
                } else if estado?.conectado == true {
                    Button("Desconectar") { desconectar() }
                } else {
                    Button("Conectar…") { conectar() }.disabled(estado?.cliente == nil)
                }
            }
            if let aviso { Text(aviso).font(.caption).foregroundStyle(.secondary) }
        }
        // iPad: dentro de una fila de `Form`, cada boton con su toque.
        .buttonStyle(.borderless)
        .onAppear(perform: leer)
    }

    private func leer() {
        LlaveroDelSistema.instalar()
        estado = iaConexiones().first { $0.id == id }
    }

    private func guardar() {
        do {
            _ = try iaGuardarCliente(servicioId: id, pegado: pegado)
            pegado = ""
            aviso = nil
        } catch {
            aviso = "\(error)"
        }
        leer()
    }

    private func conectar() {
        conectando = true
        aviso = nil
        TransporteHttps.instalar()
        let id = self.id
        Task {
            let r = await Task.detached { Result { try iaConectar(servicioId: id) } }.value
            conectando = false
            SesionWebConexion.compartida.cerrar()
            if case let .failure(e) = r { aviso = textoDeFalloAlConectar(e) }
            // iPad: el calendario lo lee otra zona (`GanchosAjustes`).
            if case .success = r { GanchosAjustes.alConectar?(id) }
            leer()
        }
    }

    private func desconectar() {
        do { try iaDesconectar(servicioId: id) } catch { aviso = "\(error)" }
        leer()
    }
}

/// El fallo de «Conectar…» en palabras. En el iPad, el nucleo de ahora
/// intenta abrir el navegador con `open` (como en el Mac) y eso iOS no lo
/// deja: se dice claro en vez del error del sistema.
func textoDeFalloAlConectar(_ e: Error) -> String {
    let t = "\(e)"
    if t.contains("no se pudo abrir el navegador") {
        return "Aún no se puede conectar desde el iPad: hazlo en el PC o el Mac y vuelve a intentarlo cuando se actualice PixPin."
    }
    if t.contains("cancelado") { return "Cancelado." }
    return t
}

/// **La hoja de inicio de sesion** para la pagina de permiso de una
/// conexion: una sola a la vez. Al cerrarla el usuario, se corta la espera
/// del nucleo (`iaCancelarConexion`).
@MainActor
final class SesionWebConexion: NSObject, ASWebAuthenticationPresentationContextProviding {
    static let compartida = SesionWebConexion()
    private var sesion: ASWebAuthenticationSession?
    /// La cerro PixPin (llego el permiso), no el usuario.
    private var cerrandoYo = false

    /// Abre `url` en la hoja. La vuelta llega al puerto local del nucleo, no
    /// aqui: sin esquema propio, la hoja se cierra con `cerrar()`.
    func abrir(_ url: URL) {
        cerrar()
        let s = ASWebAuthenticationSession(url: url, callbackURLScheme: nil) { _, error in
            let cancelada = error != nil
            DispatchQueue.main.async {
                MainActor.assumeIsolated { SesionWebConexion.compartida.acabo(cancelada: cancelada) }
            }
        }
        s.presentationContextProvider = self
        // Con las cookies de Safari: si ya hay una cuenta de Google abierta,
        // basta con elegirla.
        s.prefersEphemeralWebBrowserSession = false
        sesion = s
        if !s.start() {
            sesion = nil
            iaCancelarConexion()
        }
    }

    private func acabo(cancelada: Bool) {
        let yo = cerrandoYo
        cerrandoYo = false
        sesion = nil
        // El usuario la cerro sin acabar: el nucleo deja de esperar.
        if !yo, cancelada { iaCancelarConexion() }
    }

    /// Cierra la hoja (llego el permiso, o fallo).
    func cerrar() {
        guard let s = sesion else { return }
        cerrandoYo = true
        s.cancel()
    }

    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            let escenas = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            let activa = escenas.first { $0.activationState == .foregroundActive } ?? escenas.first
            if let v = activa?.windows.first(where: \.isKeyWindow) ?? activa?.windows.first { return v }
            return ASPresentationAnchor()
        }
    }
}

/// **Quien abre la pagina de permiso** desde el nucleo (en su hilo): la pasa
/// a la hoja en el hilo principal y dice que si al momento (la espera la
/// lleva el nucleo).
///
/// Pendiente del nucleo: cuando `apple/nucleo` exporte
/// `ia_conectar_con(servicio_id, abrir: Box<dyn AbrirNavegador>)` (ver
/// `docs/faltantes-ipad.md` §5.2), esta clase se declara `AbrirNavegador` y
/// `FilaConexion.conectar` llama a `iaConectarCon(servicioId:abrir:)` en vez
/// de `iaConectar(servicioId:)`.
final class AbridorDeConexion: @unchecked Sendable {
    func abrir(url: String) -> Bool {
        guard let u = URL(string: url) else { return false }
        DispatchQueue.main.async {
            MainActor.assumeIsolated { SesionWebConexion.compartida.abrir(u) }
        }
        return true
    }
}
