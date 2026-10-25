// **El centro de avisos comun** (tanda C4): lo que en Windows es
// `notificaciones.rs` + `pixpin_shell::notificacion` (el toast con botones)
// + `aviso::Aviso` (el globo de siempre, si el toast no sale).
//
// - **Categorias y botones**: cada clase de aviso es una
//   `UNNotificationCategory` con sus `UNNotificationAction`, las de
//   Windows: un recordatorio, «+10 min» y «Abrir» (y «Hecha» si es de una
//   tarea); una captura que se va, «Conservar» y «Abrir»; varias, «Ver la
//   galeria». Lo que hace cada boton lo pone quien avisa
//   (`alResponder`): el centro solo lo entrega.
// - **Un solo delegado** de `UNUserNotificationCenter` para toda la app
//   (antes cada uno ponia el suyo y el ultimo pisaba a los demas).
// - **El permiso, sin bloquear**: se pide una vez, sin esperar a la
//   respuesta. Mientras no lo den (o si lo niegan, o fuera de una `.app`,
//   como en `swift run`), lo que habia que avisar **ya** sale en una
//   ventanita de la app con los mismos botones (`VentanaAvisos`): el
//   globo de Windows cuando el toast no sale.
// - **Lo que llega antes de tiempo**: si la app se abre pulsando una
//   notificacion, la respuesta puede llegar antes de que nadie se haya
//   apuntado a su categoria; se guarda y se entrega al apuntarse.
//
// Rendimiento: nada sondea. Las notificaciones las programa el sistema.
//
// En el iPad (del Mac casi tal cual): la «ventanita» es una capa encima de
// la app (una `UIWindow` que deja pasar los toques fuera de las tarjetas),
// arriba a la derecha, como el globo de Windows.

import AudioToolbox
import SwiftUI
import UIKit
import UserNotifications

/// Un boton de un aviso (o pulsar el aviso entero).
public enum AccionAviso: String, CaseIterable, Sendable {
    case hecha = "pixpin.hecha"
    case posponer = "pixpin.posponer"
    case abrir = "pixpin.abrir"
    case conservar = "pixpin.conservar"
    case galeria = "pixpin.galeria"
    /// Cerrar el aviso sin hacer nada (solo en la ventanita de la app).
    case cerrar = "pixpin.cerrar"

    /// Lo que pone el boton (`notif-*` del catalogo de Windows).
    public var rotulo: String {
        switch self {
        case .hecha: "Hecha"
        case .posponer: "+\(CentroAvisos.posponerMinutos) min"
        case .abrir: "Abrir"
        case .conservar: "Conservar"
        case .galeria: "Ver la galería"
        case .cerrar: "Cerrar"
        }
    }
}

/// La clase de un aviso: que botones lleva y que hace pulsarlo entero.
public enum CategoriaAviso: String, CaseIterable, Sendable {
    /// Un recordatorio de un mensaje (`recuerdaEn`).
    case recordatorio = "pixpin.recordatorio"
    /// Un recordatorio de una tarea (`⏰`).
    case recordatorioTarea = "pixpin.recordatorio-tarea"
    /// Una captura que se va en las proximas 24 h.
    case captura = "pixpin.captura"
    /// Varias capturas que se van.
    case capturas = "pixpin.capturas"
    /// Un temporizador o una alarma de una mini-app (`AvisosMini`): sin
    /// botones, suena en su ventana.
    case mini = "pixpin.mini"

    /// Los botones, en el orden de Windows (`notificaciones::de_recordatorio`,
    /// `de_capturas`).
    public var acciones: [AccionAviso] {
        switch self {
        case .recordatorio: [.posponer, .abrir]
        case .recordatorioTarea: [.hecha, .posponer, .abrir]
        case .captura: [.conservar, .abrir]
        case .capturas: [.galeria]
        case .mini: []
        }
    }

    /// Lo que hace pulsar el aviso (el `al_pulsar` del toast de Windows).
    public var alPulsar: AccionAviso? {
        switch self {
        case .recordatorio, .recordatorioTarea: .abrir
        case .captura, .capturas: .galeria
        case .mini: nil
        }
    }

    /// Si con la app delante el aviso ya sale en una ventana suya (el pin
    /// del recordatorio, la ventana que suena de la mini-app) y la
    /// notificacion del sistema sobraria.
    public var suenaEnLaApp: Bool {
        switch self {
        case .recordatorio, .recordatorioTarea, .mini: true
        case .captura, .capturas: false
        }
    }
}

/// Un aviso: lo que dice, cuando y lo que necesita quien atiende sus
/// botones (`datos`, que viajan en el `userInfo` de la notificacion).
public struct Aviso: Equatable, Sendable, Identifiable {
    /// El identificador de la notificacion: ponerla otra vez con el mismo
    /// la cambia en vez de duplicarla.
    public var id: String
    public var categoria: CategoriaAviso
    public var titulo: String
    public var texto: String
    /// Una imagen para ensenar con el aviso (la captura que se va).
    public var imagen: URL?
    /// Cuando sale; `nil`, ya.
    public var cuando: Date?
    public var datos: [String: String]

    public init(id: String, categoria: CategoriaAviso, titulo: String, texto: String,
                imagen: URL? = nil, cuando: Date? = nil, datos: [String: String] = [:]) {
        self.id = id
        self.categoria = categoria
        self.titulo = titulo
        self.texto = texto
        self.imagen = imagen
        self.cuando = cuando
        self.datos = datos
    }
}

@MainActor
public final class CentroAvisos: NSObject, ObservableObject {
    public static let compartido = CentroAvisos()

    /// Lo que se pospone con «+10 min» (`notificaciones::POSPONER_MIN`).
    public nonisolated static let posponerMinutos = 10

    /// Si el sistema deja ensenar notificaciones.
    public enum Permiso: Equatable, Sendable {
        /// Fuera de una `.app` (`swift run`, las pruebas): no hay centro.
        case sinPaquete
        /// Aun no se ha preguntado, o se pregunto y no han contestado.
        case sinRespuesta
        case concedido
        case denegado
    }

    public typealias Respuesta = @MainActor (AccionAviso, [String: String]) -> Void

    @Published public private(set) var permiso: Permiso = .sinRespuesta
    private var preparado = false
    private var manejadores: [CategoriaAviso: Respuesta] = [:]
    /// Respuestas que llegaron antes de que nadie atendiera su categoria.
    private var guardadas: [(CategoriaAviso, AccionAviso, [String: String])] = []
    private lazy var ventana = VentanaAvisos()

    /// El centro de notificaciones, solo dentro de una `.app` (sin paquete,
    /// como en las pruebas, `UNUserNotificationCenter` se cae).
    nonisolated static var centro: UNUserNotificationCenter? {
        Bundle.main.bundleURL.pathExtension == "app" && Bundle.main.bundleIdentifier != nil
            ? UNUserNotificationCenter.current() : nil
    }

    /// **Al arrancar** (y antes de cualquier aviso; repetirlo no hace nada):
    /// el delegado, las categorias con sus botones y el permiso. El permiso
    /// se pide sin esperar: si el usuario no contesta, lo urgente sale en la
    /// app.
    public func preparar() {
        guard !preparado else { return }
        preparado = true
        guard let centro = Self.centro else {
            permiso = .sinPaquete
            return
        }
        centro.delegate = self
        centro.setNotificationCategories(Set(CategoriaAviso.allCases.map(Self.categoriaDelSistema)))
        centro.getNotificationSettings { ajustes in
            let estado = ajustes.authorizationStatus
            Task { @MainActor in
                let c = CentroAvisos.compartido
                switch estado {
                case .notDetermined:
                    c.permiso = .sinRespuesta
                    c.pedirPermiso()
                case .denied:
                    c.permiso = .denegado
                default:
                    c.permiso = .concedido
                }
            }
        }
    }

    private func pedirPermiso() {
        Self.centro?.requestAuthorization(options: [.alert, .sound]) { concedido, _ in
            Task { @MainActor in
                CentroAvisos.compartido.permiso = concedido ? .concedido : .denegado
            }
        }
    }

    /// La categoria del sistema de una de las nuestras: sus botones, en
    /// su orden.
    nonisolated static func categoriaDelSistema(_ c: CategoriaAviso) -> UNNotificationCategory {
        let acciones = c.acciones.map { a in
            UNNotificationAction(identifier: a.rawValue, title: a.rotulo,
                                 options: a == .abrir || a == .galeria ? [.foreground] : [])
        }
        return UNNotificationCategory(identifier: c.rawValue, actions: acciones, intentIdentifiers: [])
    }

    /// **Pone una notificacion del sistema** (a su hora o ya). Con el mismo
    /// `id`, la cambia. Devuelve si el sistema la va a ensenar: si no
    /// (sin permiso, sin paquete), quien avisa lo dice en la app.
    @discardableResult
    public func poner(_ a: Aviso) -> Bool {
        preparar()
        guard let centro = Self.centro, permiso != .denegado else { return false }
        let contenido = UNMutableNotificationContent()
        contenido.title = a.titulo
        contenido.body = a.texto
        contenido.sound = .default
        contenido.categoryIdentifier = a.categoria.rawValue
        contenido.userInfo = a.datos
        if let imagen = a.imagen,
           let adjunto = try? UNNotificationAttachment(identifier: "imagen", url: Self.copiaParaAdjuntar(imagen))
        {
            contenido.attachments = [adjunto]
        }
        let disparador: UNNotificationTrigger? = a.cuando.flatMap { cuando in
            guard cuando > Date() else { return nil }
            let partes = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute, .second], from: cuando)
            return UNCalendarNotificationTrigger(dateMatching: partes, repeats: false)
        }
        centro.add(UNNotificationRequest(identifier: a.id, content: contenido, trigger: disparador)) { _ in }
        return permiso == .concedido
    }

    /// El sistema **se lleva** el fichero adjunto a su almacen: se le da una
    /// copia, nunca la captura del usuario.
    nonisolated static func copiaParaAdjuntar(_ url: URL) -> URL {
        let copia = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-aviso-\(UUID().uuidString)")
            .appendingPathExtension(url.pathExtension)
        return (try? FileManager.default.copyItem(at: url, to: copia)) != nil ? copia : url
    }

    /// Quita las notificaciones pendientes con esos ids.
    public func quitar(_ ids: [String]) {
        guard !ids.isEmpty else { return }
        Self.centro?.removePendingNotificationRequests(withIdentifiers: ids)
    }

    /// **Avisa ya**: con una notificacion del sistema si hay permiso, y si
    /// no, en la ventanita de la app (el globo de Windows).
    public func avisar(_ a: Aviso) {
        var ya = a
        ya.cuando = nil
        if !poner(ya) {
            mostrarEnLaApp(ya, sonar: false)
        }
    }

    /// **En la ventanita de la app**, con los mismos botones: lo que sale
    /// siempre al sonar un recordatorio (el pin de Windows) y, sin permiso,
    /// cualquier aviso.
    public func mostrarEnLaApp(_ a: Aviso, sonar: Bool) {
        ventana.mostrar(a, sonar: sonar) { accion in
            CentroAvisos.compartido.responder(a.categoria, accion, a.datos)
        }
    }

    /// Quita un aviso de la ventanita (lo atendio otro camino).
    public func quitarDeLaApp(_ id: String) {
        ventana.quitar(id)
    }

    /// Lo que hace cada boton de una categoria. Lo que llego antes de
    /// apuntarse se entrega ahora.
    public func alResponder(_ categorias: [CategoriaAviso], _ f: @escaping Respuesta) {
        for c in categorias {
            manejadores[c] = f
        }
        let ahora = guardadas.filter { categorias.contains($0.0) }
        guardadas.removeAll { categorias.contains($0.0) }
        for (c, a, d) in ahora {
            responder(c, a, d)
        }
    }

    func responder(_ categoria: CategoriaAviso, _ accion: AccionAviso, _ datos: [String: String]) {
        guard accion != .cerrar else { return }
        if let f = manejadores[categoria] {
            f(accion, datos)
        } else {
            guardadas.append((categoria, accion, datos))
        }
    }

    /// Lo que viaja en el `userInfo`: solo cadenas (lo demas se descarta).
    nonisolated static func datos(de userInfo: [AnyHashable: Any]) -> [String: String] {
        var d: [String: String] = [:]
        for (k, v) in userInfo {
            if let k = k as? String, let v = v as? String { d[k] = v }
        }
        return d
    }

    /// Que boton se pulso: el suyo, o pulsar el aviso entero.
    nonisolated static func accion(_ identificador: String, categoria: CategoriaAviso) -> AccionAviso? {
        if identificador == UNNotificationDefaultActionIdentifier { return categoria.alPulsar }
        return AccionAviso(rawValue: identificador)
    }
}

extension CentroAvisos: UNUserNotificationCenterDelegate {
    /// Con la app delante, lo que ya suena en una ventana suya no sale
    /// otra vez como notificacion (los de las mini-apps tambien por su
    /// prefijo, por si vienen sin categoria).
    public nonisolated func userNotificationCenter(
        _: UNUserNotificationCenter, willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        let peticion = notification.request
        let categoria = CategoriaAviso(rawValue: peticion.content.categoryIdentifier)
        let enLaApp = categoria?.suenaEnLaApp == true
            || peticion.identifier.hasPrefix("mini:") || peticion.identifier.hasPrefix("local:")
        completionHandler(enLaApp ? [] : [.banner, .list, .sound])
    }

    public nonisolated func userNotificationCenter(
        _: UNUserNotificationCenter, didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let contenido = response.notification.request.content
        let datos = CentroAvisos.datos(de: contenido.userInfo)
        let accionId = response.actionIdentifier
        if let categoria = CategoriaAviso(rawValue: contenido.categoryIdentifier),
           let accion = CentroAvisos.accion(accionId, categoria: categoria)
        {
            Task { @MainActor in
                CentroAvisos.compartido.responder(categoria, accion, datos)
            }
        }
        completionHandler()
    }
}

// MARK: - La ventanita de la app

/// Un aviso en la ventanita, con lo que hacen sus botones.
struct AvisoEnLaApp: Identifiable {
    let aviso: Aviso
    let responder: (AccionAviso) -> Void
    var id: String { aviso.id }
}

@MainActor
final class ModeloVentanaAvisos: ObservableObject {
    @Published var avisos: [AvisoEnLaApp] = []
}

/// La capa de los avisos: una `UIWindow` encima de la escena activa, que
/// solo recoge los toques de sus tarjetas. Cada aviso se queda hasta que se
/// pulsa uno de sus botones o la x.
@MainActor
final class VentanaAvisos {
    private let modelo = ModeloVentanaAvisos()
    private var ventana: UIWindow?

    func mostrar(_ a: Aviso, sonar: Bool, responder: @escaping (AccionAviso) -> Void) {
        modelo.avisos.removeAll { $0.id == a.id }
        modelo.avisos.append(AvisoEnLaApp(aviso: a, responder: { [weak self] accion in
            self?.quitar(a.id)
            responder(accion)
        }))
        if ventana == nil { ventana = crearVentana() }
        ventana?.isHidden = false
        if sonar {
            // El «tri-tono» de los avisos del sistema.
            AudioServicesPlaySystemSound(1007)
        }
    }

    func quitar(_ id: String) {
        modelo.avisos.removeAll { $0.id == id }
        if modelo.avisos.isEmpty { ventana?.isHidden = true }
    }

    private func crearVentana() -> UIWindow? {
        let escenas = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        guard let escena = escenas.first(where: { $0.activationState == .foregroundActive }) ?? escenas.first else {
            return nil
        }
        let v = VentanaQueDejaPasar(windowScene: escena)
        v.windowLevel = UIWindow.Level(rawValue: UIWindow.Level.alert.rawValue + 1)
        v.backgroundColor = .clear
        let raiz = UIHostingController(rootView: VistaAvisos(modelo: modelo))
        raiz.view.backgroundColor = .clear
        v.rootViewController = raiz
        v.isHidden = false
        return v
    }
}

/// Una ventana que solo se queda los toques que caen en algo suyo (las
/// tarjetas); lo demas pasa a la app de debajo.
final class VentanaQueDejaPasar: UIWindow {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let tocado = super.hitTest(point, with: event)
        return tocado === rootViewController?.view ? nil : tocado
    }
}

struct VistaAvisos: View {
    @ObservedObject var modelo: ModeloVentanaAvisos

    var body: some View {
        VStack(spacing: 8) {
            ForEach(modelo.avisos) { a in
                TarjetaAviso(a: a)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: 360)
        .padding(.top, 12)
        .padding(.trailing, 16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
        .animation(.easeOut(duration: 0.2), value: modelo.avisos.map(\.id))
    }
}

struct TarjetaAviso: View {
    let a: AvisoEnLaApp

    private var icono: String {
        switch a.aviso.categoria {
        case .recordatorio: "alarm.fill"
        case .recordatorioTarea: "checklist"
        case .captura, .capturas: "trash"
        case .mini: "bell.fill"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 10) {
                if let imagen = a.aviso.imagen, let foto = UIImage(contentsOfFile: imagen.path) {
                    Image(uiImage: foto).resizable().scaledToFill()
                        .frame(width: 44, height: 44).clipShape(RoundedRectangle(cornerRadius: 6))
                } else {
                    Image(systemName: icono).font(.title2).foregroundStyle(.orange)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(a.aviso.titulo).font(.headline)
                    Text(a.aviso.texto).font(.subheadline).lineLimit(4)
                }
                Spacer(minLength: 0)
                Button { a.responder(.cerrar) } label: {
                    Image(systemName: "xmark").font(.caption.bold())
                        .frame(width: 28, height: 28)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
                .accessibilityLabel("Cerrar")
            }
            if !a.aviso.categoria.acciones.isEmpty {
                HStack {
                    Spacer()
                    ForEach(a.aviso.categoria.acciones, id: \.self) { accion in
                        Button(accion.rotulo) { a.responder(accion) }
                            .buttonStyle(.bordered)
                    }
                }
            }
        }
        .padding(12)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(.separator))
        .shadow(color: .black.opacity(0.2), radius: 10, y: 4)
    }
}
