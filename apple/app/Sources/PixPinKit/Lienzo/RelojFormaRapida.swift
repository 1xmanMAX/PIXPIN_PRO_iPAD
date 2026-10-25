// **El gesto de pararse** (forma rapida): dibujando a mano, quedarse quieto
// con el boton pulsado convierte el trazo en figura (`Gesto::convertir_en_forma`
// del motor, el mismo de Windows y del movil). El motor no tiene reloj: lo
// pone quien recibe el raton (`dibujo::mano::forma_rapida` en Windows).
//
// Rendimiento (§4bis, «nada corriendo sin motivo»): UNA sola espera de una
// vez (`DispatchWorkItem`), nunca un temporizador que se repite. Se programa
// al pulsar con un trazo a mano, se reprograma cuando el cursor se aleja mas
// que el temblor de la mano (8 px, `forma_rapida::TEMBLOR_PX`: la mano parada
// nunca esta quieta del todo) y se cancela al soltar. Si vence, avisa una vez;
// el siguiente movimiento vuelve a armarla, como en Windows.
//
// El programador se inyecta: las pruebas lo sustituyen por uno que no
// espera (sin dormir ni depender del reloj de la maquina).

import CoreGraphics
import Foundation

public final class RelojFormaRapida {
    /// Programa `accion` dentro de `segundos` y devuelve como cancelarla.
    public typealias Programar = (_ segundos: Double, _ accion: @escaping () -> Void) -> () -> Void

    /// El de la app: un `DispatchWorkItem` de una vez en la cola principal.
    public static let conDispatch: Programar = { segundos, accion in
        let tarea = DispatchWorkItem(block: accion)
        DispatchQueue.main.asyncAfter(deadline: .now() + segundos, execute: tarea)
        return { tarea.cancel() }
    }

    private let programar: Programar
    private let temblor: CGFloat
    /// Lo que pasa cuando vence la espera (convertir el trazo).
    public var alPararse: () -> Void = {}

    /// Donde se quedo quieto el cursor (puntos de la vista), si la espera
    /// esta armada.
    private var ancla: CGPoint?
    /// La espera del trazo de ahora: el medio segundo del lapiz o el
    /// segundo del grafito (lo dice el motor al pulsar). `nil`: no se traza
    /// a mano y no hay gesto de pararse.
    private var espera: Double?
    private var cancelar: (() -> Void)?

    /// Cuantas veces se programo la espera (para las pruebas y para medir).
    public private(set) var programadas = 0

    public init(temblorPx: CGFloat, programar: @escaping Programar = RelojFormaRapida.conDispatch) {
        self.temblor = max(0, temblorPx)
        self.programar = programar
    }

    /// Si hay una espera en marcha.
    public var armado: Bool { cancelar != nil }

    /// Al pulsar: `esperaMs` es lo que dice el motor (`nil` si la
    /// herramienta no traza a mano).
    public func pulsar(en punto: CGPoint, esperaMs: UInt64?) {
        parar()
        guard let esperaMs, esperaMs > 0 else {
            espera = nil
            return
        }
        espera = Double(esperaMs) / 1000
        armar(en: punto)
    }

    /// Cada aviso del arrastre. Dentro del temblor no cuenta como moverse.
    public func mover(a punto: CGPoint) {
        guard espera != nil else { return }
        if let ancla, hypot(punto.x - ancla.x, punto.y - ancla.y) <= temblor { return }
        armar(en: punto)
    }

    /// Al soltar o cancelar: ya no hay nada que convertir.
    public func soltar() {
        parar()
        espera = nil
    }

    private func armar(en punto: CGPoint) {
        guard let espera else { return }
        cancelar?()
        ancla = punto
        programadas += 1
        cancelar = programar(espera) { [weak self] in self?.vencio() }
    }

    private func vencio() {
        guard cancelar != nil else { return }
        // Una sola vez por pausa: el proximo movimiento la vuelve a armar.
        cancelar = nil
        ancla = nil
        alPararse()
    }

    private func parar() {
        cancelar?()
        cancelar = nil
        ancla = nil
    }
}
