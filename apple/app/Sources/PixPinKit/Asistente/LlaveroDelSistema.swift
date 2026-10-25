// Las claves del asistente (las de la nube: Claude, DeepSeek) en el
// Llavero del Mac. Es el `LlaveroApp` que pide el nucleo de Rust
// (`pixpin-nucleo/src/llavero.rs`): Rust nunca guarda una clave en claro, y
// sin este llavero instalado no guarda ninguna.
//
// Contraseñas genericas (`kSecClassGenericPassword`) del Llavero de inicio
// de sesion: servicio `PixPinMax.asistente`, cuenta el proveedor. No se usa
// el llavero de proteccion de datos (`kSecUseDataProtectionKeychain`): pide
// un grupo de llaveros firmado con una cuenta de desarrollador, y la app se
// firma ad hoc.

import Foundation
import PixPinCore
import Security

public final class LlaveroDelSistema: LlaveroApp, @unchecked Sendable {
    /// Se anade al servicio: las pruebas usan uno propio para no tocar las
    /// claves de verdad.
    let sufijo: String

    public init(sufijo: String = "") {
        self.sufijo = sufijo
    }

    /// El de las pruebas (un servicio propio, para no tocar las claves de
    /// verdad): uno solo por proceso, porque el nucleo instala uno solo.
    static let dePruebas = LlaveroDelSistema(sufijo: ".pruebas-\(UUID().uuidString)")

    private static let cerrojo = NSLock()
    private static var instalado = false

    /// Instala el Llavero del Mac en el nucleo (una vez por proceso; las
    /// demas no hacen nada). Devuelve si quedo instalado.
    @discardableResult
    public static func instalar(_ llavero: LlaveroDelSistema = LlaveroDelSistema()) -> Bool {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        if instalado { return true }
        do {
            try instalarLlavero(llavero: llavero)
            instalado = true
        } catch {
            // Ya habia uno (otro camino lo instalo): vale igual.
            instalado = true
        }
        return instalado
    }

    private func consulta(_ servicio: String, _ cuenta: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: servicio + sufijo,
            kSecAttrAccount as String: cuenta,
        ]
    }

    static func texto(_ estado: OSStatus) -> String {
        let detalle = SecCopyErrorMessageString(estado, nil) as String? ?? "error \(estado)"
        return "Llavero: \(detalle) (\(estado))"
    }

    public func guardar(servicio: String, cuenta: String, clave: String) throws {
        let datos = Data(clave.utf8)
        let q = consulta(servicio, cuenta)
        var estado = SecItemUpdate(q as CFDictionary, [kSecValueData as String: datos] as CFDictionary)
        if estado == errSecItemNotFound {
            var nuevo = q
            nuevo[kSecValueData as String] = datos
            nuevo[kSecAttrLabel as String] = "PixPin: clave de \(cuenta)"
            estado = SecItemAdd(nuevo as CFDictionary, nil)
        }
        guard estado == errSecSuccess else {
            throw ErrorNucleo.Disco(detalle: LlaveroDelSistema.texto(estado))
        }
    }

    public func leer(servicio: String, cuenta: String) -> String? {
        var q = consulta(servicio, cuenta)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var resultado: CFTypeRef?
        let estado = SecItemCopyMatching(q as CFDictionary, &resultado)
        guard estado == errSecSuccess, let datos = resultado as? Data else { return nil }
        return String(data: datos, encoding: .utf8)
    }

    public func borrar(servicio: String, cuenta: String) throws {
        let estado = SecItemDelete(consulta(servicio, cuenta) as CFDictionary)
        guard estado == errSecSuccess || estado == errSecItemNotFound else {
            throw ErrorNucleo.Disco(detalle: LlaveroDelSistema.texto(estado))
        }
    }
}
