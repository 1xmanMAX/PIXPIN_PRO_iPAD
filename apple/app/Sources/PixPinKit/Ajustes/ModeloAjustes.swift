// Lo que se guarda de los Ajustes del iPad, cada cosa en su sitio (copia de
// `ModeloAjustes.swift` del Mac sin AppKit ni `SMAppService`):
//
// - En `pixpinmax.toml` (el MISMO fichero de Windows, por el nucleo de Rust
//   `Nucleo.ajustes`/`guardarAjustes`), lo que vale en todos los sistemas:
//   `valores` (idioma, caducidad de la galeria, sincronizar...) y `pdf`
//   (`[pdf]`, por `ajustesPdf`).
// - En este iPad: el tema (UserDefaults) y el nombre del aparato (que es de
//   la identidad del grupo, por el `Sincronizador`).
//
// Como en Windows, cada cambio se puede deshacer (hasta 100 pasos). Leer y
// escribir el TOML va fuera del hilo principal (§4bis.3), y los guardados
// van uno detras de otro.

import Foundation
import PixPinCore
import SwiftUI
import UIKit

/// El aspecto de la app. De fabrica sigue al sistema.
public enum Tema: String, CaseIterable, Identifiable, Sendable {
    case sistema
    case claro
    case oscuro

    public var id: String { rawValue }

    public var nombre: String {
        switch self {
        case .sistema: return "Como el iPad"
        case .claro: return "Claro"
        case .oscuro: return "Oscuro"
        }
    }

    static let clave = "tema"

    public static func guardado(_ d: UserDefaults = .standard) -> Tema {
        d.string(forKey: clave).flatMap(Tema.init(rawValue:)) ?? .sistema
    }

    public func guardar(_ d: UserDefaults = .standard) {
        if self == .sistema { d.removeObject(forKey: Tema.clave) } else { d.set(rawValue, forKey: Tema.clave) }
    }

    /// Para `preferredColorScheme`: `nil` sigue al sistema.
    public var esquema: ColorScheme? {
        switch self {
        case .sistema: return nil
        case .claro: return .light
        case .oscuro: return .dark
        }
    }

    /// Para `UIWindow.overrideUserInterfaceStyle`.
    public var estilo: UIUserInterfaceStyle {
        switch self {
        case .sistema: return .unspecified
        case .claro: return .light
        case .oscuro: return .dark
        }
    }

    /// **Lo pone en todas las ventanas abiertas** (con Stage Manager puede
    /// haber varias): asi vale tambien para hojas y menus, y los colores de
    /// `Paleta` que tienen par de dia y de noche cambian solos.
    @MainActor
    public func aplicar() {
        for escena in UIApplication.shared.connectedScenes {
            guard let ventanas = (escena as? UIWindowScene)?.windows else { continue }
            for v in ventanas { v.overrideUserInterfaceStyle = estilo }
        }
    }

    private static var vigilando = false

    /// **Al arrancar**: aplica el tema guardado y lo vuelve a aplicar a cada
    /// ventana nueva (una escena que se activa). Una vez basta.
    @MainActor
    public static func aplicarAlArrancar() {
        guardado().aplicar()
        guard !vigilando else { return }
        vigilando = true
        _ = NotificationCenter.default.addObserver(forName: UIScene.didActivateNotification, object: nil,
                                               queue: .main) { _ in
            MainActor.assumeIsolated { Tema.guardado().aplicar() }
        }
    }
}

/// Lo que los Ajustes avisan a otras partes de la app sin depender de ellas.
/// Sin gancho, no pasa nada.
@MainActor
public enum GanchosAjustes {
    /// Se conecto un servicio del asistente (`FilaConexion`): su id, como
    /// `google_calendar` (para que los recordatorios lean el calendario).
    public static var alConectar: ((String) -> Void)?
}

@MainActor
public final class ModeloAjustes: ObservableObject {
    /// Lo de `pixpinmax.toml` (los de fabrica hasta `cargar`).
    @Published public private(set) var valores: AjustesMac
    /// Los de fabrica de Windows (`Ajustes::default`).
    public let fabrica: AjustesMac
    /// `[pdf]` de `pixpinmax.toml` (`nil` hasta `cargar`).
    @Published public private(set) var pdf: AjustesPdf?
    @Published public var tema: Tema {
        didSet {
            tema.guardar(defaults)
            tema.aplicar()
        }
    }
    @Published public private(set) var error: String?
    @Published public private(set) var cargado = false
    /// Lo que «Deshacer» devuelve, el mas reciente al final.
    @Published public private(set) var historial: [Paso] = []

    /// Un paso de «Deshacer»: como estaba antes de un cambio.
    public enum Paso: Equatable {
        case toml(AjustesMac)
        case pdf(AjustesPdf)
    }

    /// Cuantos pasos guarda «Deshacer» (los de Windows).
    public static let pasosDeHistorial = 100

    /// Los de fabrica de `[pdf]` (`Pdf::default` de Windows).
    public static let pdfDeFabrica = AjustesPdf(aligerarAlEntrar: true, nivel: .equilibrado)

    public let nucleo: Nucleo
    /// La sincronizacion de la app, para encender o apagar la automatica.
    public weak var sincro: Sincronizador?
    private let defaults: UserDefaults

    private static var unico: ModeloAjustes?

    /// **El de la app**: uno solo para Ajustes y Aparatos (dos copias del
    /// TOML en memoria se pisarian al guardar). Otro nucleo (las pruebas),
    /// otro modelo.
    public static func compartido(nucleo: Nucleo, sincro: Sincronizador?) -> ModeloAjustes {
        if let m = unico, m.nucleo === nucleo {
            if m.sincro == nil { m.sincro = sincro }
            return m
        }
        let m = ModeloAjustes(nucleo: nucleo, sincro: sincro)
        unico = m
        return m
    }

    public init(nucleo: Nucleo, sincro: Sincronizador? = nil, defaults: UserDefaults = .standard) {
        self.nucleo = nucleo
        self.sincro = sincro
        self.defaults = defaults
        fabrica = nucleo.ajustesDeFabrica()
        valores = fabrica
        tema = Tema.guardado(defaults)
    }

    /// La carpeta de datos (la de `proyectos/`, `sincro/`, `pixpinmax.toml`).
    public var carpetaDatos: URL { URL(fileURLWithPath: nucleo.raiz(), isDirectory: true) }

    /// `pixpinmax.toml`.
    public var ficheroAjustes: String { nucleo.rutaAjustes() }

    public var diasCaducidad: Int { Int(valores.diasCaducidad) }
    public var sincroAutomatica: Bool { valores.sincroAutomatica }

    /// Lee el TOML y pasa la sincronizacion sola al `Sincronizador`.
    public func cargar() async {
        let nucleo = self.nucleo
        let r = await Task.detached(priority: .utility) { () -> Result<(AjustesMac, AjustesPdf), Error> in
            do { return .success((try nucleo.ajustes(), try nucleo.ajustesPdf())) } catch { return .failure(error) }
        }.value
        switch r {
        case let .success((a, p)):
            valores = a
            pdf = p
            sincro?.automatica = a.sincroAutomatica
            error = nil
        case let .failure(e):
            error = "No se pudo leer pixpinmax.toml: \(ModeloAjustes.texto(de: e))"
        }
        cargado = true
    }

    /// **Cambia lo del TOML** y lo guarda (en la cola de guardados). Lo de
    /// antes queda para «Deshacer».
    public func cambiar(_ cambio: (inout AjustesMac) -> Void) {
        var nuevo = valores
        cambio(&nuevo)
        guard nuevo != valores else { return }
        apuntar(.toml(valores))
        poner(nuevo)
    }

    /// Cambia `[pdf]` y lo guarda.
    public func cambiarPdf(_ cambio: (inout AjustesPdf) -> Void) {
        let antes = pdf ?? ModeloAjustes.pdfDeFabrica
        var nuevo = antes
        cambio(&nuevo)
        guard nuevo != antes else { return }
        apuntar(.pdf(antes))
        ponerPdf(nuevo)
    }

    /// Deshace el ultimo cambio.
    public func deshacer() {
        guard let paso = historial.popLast() else { return }
        switch paso {
        case let .toml(a): poner(a)
        case let .pdf(p): ponerPdf(p)
        }
    }

    public var sePuedeDeshacer: Bool { !historial.isEmpty }

    /// Si algo del TOML no esta como de fabrica.
    public var hayCambios: Bool { valores != fabrica || (pdf ?? ModeloAjustes.pdfDeFabrica) != ModeloAjustes.pdfDeFabrica }

    /// Todo lo del TOML de esta pantalla a fabrica, en un paso de «Deshacer»
    /// (lo que el iPad no ensena, como la captura, se queda como estaba).
    public func restablecer() {
        let f = fabrica
        cambiar {
            $0.diasCaducidad = f.diasCaducidad
            $0.sincroAutomatica = f.sincroAutomatica
            $0.sincroPresencia = f.sincroPresencia
            $0.sincroLoMioManda = f.sincroLoMioManda
            $0.idioma = f.idioma
        }
        cambiarPdf { $0 = ModeloAjustes.pdfDeFabrica }
    }

    private func apuntar(_ paso: Paso) {
        historial.append(paso)
        if historial.count > ModeloAjustes.pasosDeHistorial {
            historial.removeFirst(historial.count - ModeloAjustes.pasosDeHistorial)
        }
    }

    private func poner(_ nuevo: AjustesMac) {
        let antes = valores
        valores = nuevo
        sincro?.automatica = nuevo.sincroAutomatica
        if nuevo.idioma != antes.idioma {
            ModeloAjustes.aplicar(idioma: nuevo.idioma, en: defaults)
        }
        let guardado = programarGuardado()
        // La presencia la abre o la cierra el `Sincronizador` al releer el
        // TOML: despues de escribirlo.
        if nuevo.sincroPresencia != antes.sincroPresencia, let sincro {
            Task { [weak sincro] in
                await guardado.value
                await sincro?.refrescar()
            }
        }
    }

    private func ponerPdf(_ nuevo: AjustesPdf) {
        pdf = nuevo
        let previo = ultimoGuardado
        let nucleo = self.nucleo
        ultimoGuardado = Task { [weak self] in
            await previo?.value
            let fallo = await Task.detached(priority: .utility) { () -> String? in
                do { try nucleo.guardarAjustesPdf(nuevos: nuevo); return nil } catch { return ModeloAjustes.texto(de: error) }
            }.value
            self?.error = fallo.map { "No se pudo guardar: \($0)" }
        }
    }

    /// El idioma de la app (`AppleLanguages` de este programa): vale desde el
    /// proximo arranque, como en Windows. «sistema» lo deja en el del iPad.
    nonisolated static func aplicar(idioma: String, en defaults: UserDefaults) {
        switch idioma {
        case "es", "en": defaults.set([idioma], forKey: "AppleLanguages")
        default: defaults.removeObject(forKey: "AppleLanguages")
        }
    }

    public func fijar(diasCaducidad dias: Int) {
        let d = UInt32(max(0, min(90, dias)))
        cambiar { $0.diasCaducidad = d }
    }

    public func fijar(sincroAutomatica s: Bool) {
        cambiar { $0.sincroAutomatica = s }
    }

    /// El ultimo guardado en marcha: los guardados van **uno detras de
    /// otro** (dos escrituras a la vez de `pixpinmax.toml` se pisaban).
    private var ultimoGuardado: Task<Void, Never>?

    @discardableResult
    private func programarGuardado() -> Task<Void, Never> {
        let previo = ultimoGuardado
        let t = Task { [weak self] in
            await previo?.value
            await self?.guardarAhora()
        }
        ultimoGuardado = t
        return t
    }

    /// Escribe lo de este modelo en `pixpinmax.toml`, conservando lo demas
    /// (despues de los guardados que ya esperaban).
    public func guardar() async {
        await programarGuardado().value
    }

    private func guardarAhora() async {
        let nuevos = valores
        let nucleo = self.nucleo
        let fallo = await Task.detached(priority: .utility) { () -> String? in
            do {
                try nucleo.guardarAjustes(nuevos: nuevos)
                return nil
            } catch {
                return ModeloAjustes.texto(de: error)
            }
        }.value
        error = fallo.map { "No se pudo guardar: \($0)" }
    }

    /// La version de la app («0.1.0 (1)»), la del Info.plist.
    public nonisolated static var version: String {
        let info = Bundle.main.infoDictionary ?? [:]
        let corta = info["CFBundleShortVersionString"] as? String ?? "—"
        let numero = info["CFBundleVersion"] as? String
        return numero.map { "\(corta) (\($0))" } ?? corta
    }

    nonisolated static func texto(de error: Error) -> String {
        if let e = error as? ErrorNucleo {
            switch e {
            case let .Disco(d), let .Lienzo(d), let .Sincro(d), let .Argumento(d), let .Ajustes(d), let .Asistente(d):
                return d
            case let .NoExiste(que):
                return "no existe: \(que)"
            }
        }
        return error.localizedDescription
    }
}
