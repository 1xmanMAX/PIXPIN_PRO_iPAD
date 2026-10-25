// Anunciar este Mac en la Wi-Fi como un aparato de PixPin, igual que Windows
// (`anunciarse` + `mdns::anunciar`) y Android (`NsdManager`): la instancia
// `PixPin <nombre>` del tipo `_pixpin._tcp` con el TXT `g`/`id`/`l`/`n`, en
// el puerto donde escucha el nucleo de Rust.
//
// Por que `DNSServiceRegister` (la API C de `dns_sd.h`) y no otra cosa:
//
// - `NWListener.service` solo anuncia el puerto de ese mismo `NWListener`,
//   y el socket que escucha es el de Rust (`Nucleo.escuchar`): con
//   `NWListener` habria que mover el protocolo entero a Swift.
// - `NetService` hace lo mismo pero esta obsoleto desde macOS 12.
// - `DNSServiceRegister` anuncia cualquier puerto, no esta obsoleto, es lo
//   que hay debajo de los otros dos (mDNSResponder) y es una llamada.
//
// Se registra con una funcion de respuesta y la cola propia
// (`DNSServiceSetDispatchQueue`): asi mDNSResponder puede contestar (nombre
// final si hubo choque, errores) sin que nadie lea a mano su socket. Todo lo
// que toca el `DNSServiceRef` va en esa cola, como pide `dns_sd.h`.
//
// Si un SDK no trajera el modulo `dnssd`, se compila con `NetService`
// (mismo anuncio, obsoleto pero vigente): ver el `#else` del final.

import Foundation
import PixPinCore
#if canImport(dnssd)
import dnssd
#endif

#if canImport(dnssd)
public final class Anunciador: @unchecked Sendable {
    private let cola = DispatchQueue(label: "pixpin.sincro.anuncio", qos: .utility)
    /// Solo se toca en `cola`.
    private var registro: DNSServiceRef?
    /// Lo anunciado ahora (en `cola`), para no rehacerlo si no cambia.
    private var actual: (AnuncioBonjour, UInt16)?

    public init() {}

    deinit {
        if let registro { DNSServiceRefDeallocate(registro) }
    }

    /// Anuncia `anuncio` en `puerto`, sustituyendo lo anunciado antes. Si ya
    /// estaba igual, no hace nada (rehacerlo obliga a los demas a olvidarlo
    /// y volver a verlo, que en algunas Wi-Fi tarda minutos).
    public func anunciar(_ anuncio: AnuncioBonjour, puerto: UInt16) {
        cola.async { [self] in
            if let actual, actual.0 == anuncio, actual.1 == puerto { return }
            soltar()
            let txt = codificarTXT(anuncio.txt.map { ($0.clave, $0.valor) })
            var nuevo: DNSServiceRef?
            let resultado: DNSServiceErrorType = txt.withUnsafeBytes { bytes in
                DNSServiceRegister(&nuevo, 0, 0, anuncio.instancia, anuncio.tipo, nil, nil,
                                   puerto.bigEndian, UInt16(bytes.count), bytes.baseAddress,
                                   Anunciador.alResponder, nil)
            }
            // 0 es kDNSServiceErr_NoError.
            guard resultado == 0, let nuevo else {
                NSLog("PixPin: no se pudo anunciar en la red (DNSServiceRegister %d)", resultado)
                return
            }
            if DNSServiceSetDispatchQueue(nuevo, cola) != 0 {
                NSLog("PixPin: el anuncio no tiene cola para sus respuestas")
            }
            registro = nuevo
            actual = (anuncio, puerto)
        }
    }

    /// Deja de anunciar (al salir del grupo o cerrar la app).
    public func retirar() {
        cola.async { [self] in soltar() }
    }

    /// En `cola`.
    private func soltar() {
        if let registro { DNSServiceRefDeallocate(registro) }
        registro = nil
        actual = nil
    }

    /// La respuesta de mDNSResponder: solo se apunta si fue mal (un choque de
    /// nombre lo resuelve el solo anadiendo « (2)»).
    private static let alResponder: DNSServiceRegisterReply = { _, _, error, _, _, _, _ in
        if error != 0 {
            NSLog("PixPin: el anuncio fallo (%d)", error)
        }
    }
}

#else

public final class Anunciador: NSObject, @unchecked Sendable {
    /// Solo se toca en el hilo principal (`NetService` necesita su bucle).
    private var servicio: NetService?
    private var actual: (AnuncioBonjour, UInt16)?

    override public init() {}

    public func anunciar(_ anuncio: AnuncioBonjour, puerto: UInt16) {
        DispatchQueue.main.async { [self] in
            if let actual, actual.0 == anuncio, actual.1 == puerto { return }
            servicio?.stop()
            let s = NetService(domain: "", type: anuncio.tipo + ".", name: anuncio.instancia,
                               port: Int32(puerto))
            var txt: [String: Data] = [:]
            for par in anuncio.txt { txt[par.clave] = Data(par.valor.utf8) }
            s.setTXTRecord(NetService.data(fromTXTRecord: txt))
            s.publish()
            servicio = s
            actual = (anuncio, puerto)
        }
    }

    public func retirar() {
        DispatchQueue.main.async { [self] in
            servicio?.stop()
            servicio = nil
            actual = nil
        }
    }
}

#endif
