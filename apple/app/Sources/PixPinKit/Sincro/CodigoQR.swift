// Lo que necesitan los QR de PixPin (invitar a un chat compartido, enviar y
// recibir con codigo, y el codigo del grupo): pintar el QR con CoreImage
// (`CIQRCodeGenerator`, sin bibliotecas) y saber la IPv4 de este iPad en la
// red local para meterla en el texto. Copia de `CodigoQR.swift` del Mac con
// `UIImage` en vez de `NSImage`.
//
// Por que solo IPv4 en el QR: el texto es `pixpin-chat:1:<codigo>:<ip>:<puerto>`
// y Windows y Android lo parten por `:`; una IPv6 lo romperia. Sin IPv4 el
// host va vacio y el otro busca en Bonjour por la etiqueta del codigo.

import CoreImage
import CoreImage.CIFilterBuiltins
import Darwin
import PixPinCore
import UIKit

/// **El QR de `texto`**, nitido (escalado sin suavizar) y de `lado` puntos.
/// `nil` si CoreImage no pudo (un texto demasiado largo para un QR).
public func imagenQR(_ texto: String, lado: CGFloat = 220) -> UIImage? {
    let filtro = CIFilter.qrCodeGenerator()
    filtro.message = Data(texto.utf8)
    // «M»: se lee bien desde una pantalla y deja el QR pequeno.
    filtro.correctionLevel = "M"
    guard let salida = filtro.outputImage, salida.extent.width > 0 else { return nil }
    let escala = max(1, (lado / salida.extent.width).rounded(.down))
    let grande = salida.transformed(by: CGAffineTransform(scaleX: escala, y: escala))
    let contexto = CIContext(options: [.useSoftwareRenderer: false])
    guard let cg = contexto.createCGImage(grande, from: grande.extent) else { return nil }
    return UIImage(cgImage: cg)
}

/// **La IPv4 de este iPad en la red local** (la Wi-Fi), la que va en un QR.
/// Primero `en0`…`enN` y despues lo demas que este arriba, sin la de bucle
/// ni las de enlace local (169.254). `nil` si no hay ninguna.
public func ipv4Local() -> String? {
    var lista: UnsafeMutablePointer<ifaddrs>?
    guard getifaddrs(&lista) == 0, let primera = lista else { return nil }
    defer { freeifaddrs(lista) }
    var candidatas: [(interfaz: String, ip: String)] = []
    var p: UnsafeMutablePointer<ifaddrs>? = primera
    while let actual = p {
        defer { p = actual.pointee.ifa_next }
        let flags = Int32(actual.pointee.ifa_flags)
        guard (flags & IFF_UP) != 0, (flags & IFF_LOOPBACK) == 0,
              let dir = actual.pointee.ifa_addr, dir.pointee.sa_family == UInt8(AF_INET) else { continue }
        var texto = [CChar](repeating: 0, count: Int(NI_MAXHOST))
        guard getnameinfo(dir, socklen_t(dir.pointee.sa_len), &texto, socklen_t(texto.count),
                          nil, 0, NI_NUMERICHOST) == 0 else { continue }
        let ip = String(cString: texto)
        let interfaz = String(cString: actual.pointee.ifa_name)
        if ip.hasPrefix("169.254.") { continue }
        // La red del movil (pdp_ip) no lleva a ningun aparato de casa.
        if interfaz.hasPrefix("pdp_ip") { continue }
        candidatas.append((interfaz, ip))
    }
    return elegirIPv4(candidatas)
}

/// Entre las IPv4 de las interfaces, la de la red local: `en*` primero (en
/// orden de nombre), y si no hay, la primera. Pura, para probarla.
public func elegirIPv4(_ candidatas: [(interfaz: String, ip: String)]) -> String? {
    let en = candidatas
        .filter { $0.interfaz.hasPrefix("en") }
        .sorted { $0.interfaz.localizedStandardCompare($1.interfaz) == .orderedAscending }
    return (en.first ?? candidatas.first)?.ip
}

/// **El codigo de grupo que hay en un texto** (lo tecleado, lo pegado o lo
/// leido de un QR): el QR del grupo que ensena el iPad lleva el codigo tal
/// cual (`K7Q2M-9XMPA`), y tambien vale un texto con el codigo dentro
/// (`pixpin-grupo:K7Q2M-9XMPA`, una frase copiada). Devuelve el codigo como
/// se teclea, o `nil` si no hay ninguno valido (lo decide el nucleo,
/// `codigoValido`). Pura, para probarla.
public func codigoDeGrupoEn(_ texto: String) -> String? {
    let limpio = texto.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !limpio.isEmpty else { return nil }
    if codigoValido(codigo: limpio) { return limpio }
    // Los trozos de signos y guiones: el codigo puede venir entre otras cosas.
    let trozos = limpio.split(whereSeparator: { !($0.isLetter || $0.isNumber || $0 == "-") })
    for t in trozos.reversed() where codigoValido(codigo: String(t)) {
        return String(t)
    }
    return nil
}
