// El HTTPS del asistente: el `TransporteApp` que pide el nucleo de Rust
// (`pixpin-nucleo/src/asistente/transporte.rs`). Rust arma la peticion de
// Claude o DeepSeek (la misma que en Windows) y aqui URLSession la manda
// con el TLS, el proxy y los certificados del sistema; la respuesta (SSE)
// vuelve a Rust a trozos mientras llega, y si Rust dice que pare (el boton
// «Parar»), se cancela la peticion.
//
// Rust llama a `enviar` desde un hilo suyo y espera: aqui se bloquea ese
// hilo (nunca el principal ni uno de Swift Concurrency) con un semaforo
// hasta que URLSession acaba.
//
// La clave va en las cabeceras y no se escribe en ningun sitio.

import Foundation
import PixPinCore

public final class TransporteHttps: TransporteApp, @unchecked Sendable {
    /// La configuracion de cada peticion. Las pruebas ponen una con un
    /// `URLProtocol` falso: nunca se llama a una API de pago.
    nonisolated(unsafe) static var configuracion: URLSessionConfiguration = {
        let c = URLSessionConfiguration.ephemeral
        // Los mismos plazos que WinHTTP en Windows: 20 s para conectar y
        // hasta 3 min sin recibir nada (pensar tarda).
        c.timeoutIntervalForRequest = 180
        c.timeoutIntervalForResource = 15 * 60
        c.waitsForConnectivity = false
        c.urlCache = nil
        return c
    }()

    private static let cerrojo = NSLock()
    nonisolated(unsafe) private static var instalado = false

    /// Instala el HTTPS en el nucleo (una vez por proceso; las demas no
    /// hacen nada). Lo llama el asistente y «Probar» antes de usar la nube.
    public static func instalar() {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        guard !instalado else { return }
        // Si ya habia uno (otro camino lo instalo), vale igual.
        try? instalarTransporte(transporte: TransporteHttps())
        instalado = true
    }

    public func enviar(url: String, cabeceras: [CabeceraHttp], cuerpo: Data, receptor: ReceptorHttp) -> String? {
        guard let destino = URL(string: url), destino.scheme == "https" else {
            return "dirección no válida"
        }
        var pedido = URLRequest(url: destino)
        pedido.httpMethod = "POST"
        pedido.setValue("application/json", forHTTPHeaderField: "Content-Type")
        pedido.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        for c in cabeceras {
            pedido.setValue(c.valor, forHTTPHeaderField: c.nombre)
        }
        pedido.httpBody = cuerpo
        let delegado = Delegado(receptor: receptor)
        let cola = OperationQueue()
        cola.maxConcurrentOperationCount = 1
        cola.qualityOfService = .utility
        let sesion = URLSession(configuration: TransporteHttps.configuracion, delegate: delegado, delegateQueue: cola)
        sesion.dataTask(with: pedido).resume()
        delegado.hecho.wait()
        sesion.finishTasksAndInvalidate()
        return delegado.error
    }

    /// Una peticion cualquiera de las conexiones (Google Drive y su OAuth):
    /// su metodo y solo sus cabeceras; la `Location` va a Rust antes que
    /// los datos (la subida reanudable de Drive la necesita).
    public func pedir(metodo: String, url: String, cabeceras: [CabeceraHttp], cuerpo: Data,
                      receptor: ReceptorHttp) -> String? {
        guard let destino = URL(string: url), destino.scheme == "https" else {
            return "dirección no válida"
        }
        var pedido = URLRequest(url: destino)
        pedido.httpMethod = metodo
        for c in cabeceras {
            pedido.setValue(c.valor, forHTTPHeaderField: c.nombre)
        }
        if !cuerpo.isEmpty { pedido.httpBody = cuerpo }
        let delegado = Delegado(receptor: receptor)
        let cola = OperationQueue()
        cola.maxConcurrentOperationCount = 1
        cola.qualityOfService = .utility
        let sesion = URLSession(configuration: TransporteHttps.configuracion, delegate: delegado, delegateQueue: cola)
        sesion.dataTask(with: pedido).resume()
        delegado.hecho.wait()
        sesion.finishTasksAndInvalidate()
        return delegado.error
    }

    /// Pasa la respuesta a Rust mientras llega.
    final class Delegado: NSObject, URLSessionDataDelegate, @unchecked Sendable {
        let receptor: ReceptorHttp
        let hecho = DispatchSemaphore(value: 0)
        var error: String?
        private var cortado = false

        init(receptor: ReceptorHttp) {
            self.receptor = receptor
        }

        func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                        completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
            let http = response as? HTTPURLResponse
            let espera = http?.value(forHTTPHeaderField: "retry-after").flatMap(Double.init) ?? -1
            receptor.estado(codigo: UInt32(http?.statusCode ?? 0), reintentarEnS: espera)
            if let donde = http?.value(forHTTPHeaderField: "Location") {
                receptor.ubicacion(valor: donde)
            }
            completionHandler(.allow)
        }

        func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
            guard !cortado else { return }
            if !receptor.datos(trozo: data) {
                cortado = true
                dataTask.cancel()
            }
        }

        func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
            if let error, !cortado {
                self.error = TransporteHttps.texto(de: error)
            }
            hecho.signal()
        }
    }

    /// El error de red en palabras (sin la URL, que no aporta).
    static func texto(de error: Error) -> String {
        let e = error as NSError
        switch e.code {
        case NSURLErrorNotConnectedToInternet, NSURLErrorNetworkConnectionLost: return "sin conexión a internet"
        case NSURLErrorTimedOut: return "tardó demasiado en contestar"
        case NSURLErrorCannotFindHost, NSURLErrorDNSLookupFailed: return "no se encuentra el servidor"
        case NSURLErrorSecureConnectionFailed, NSURLErrorServerCertificateUntrusted: return "la conexión segura falló"
        default: return e.localizedDescription
        }
    }
}
