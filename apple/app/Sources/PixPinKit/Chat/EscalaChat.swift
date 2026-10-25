// **La escala del chat** (la «escala de la interfaz» de Windows, como en
// Telegram): ⌘+ agranda, ⌘− encoge y ⌘0 vuelve a la de siempre. Va por
// escalones y no a ojo (`pixpin-ui/src/chat.rs`, `ESCALAS`): con un numero
// libre se acaba en 137 % y las medidas redondean distinto en cada trozo.
//
// Lo que crece: la letra de las burbujas y de la caja de escribir, las fotos
// y dibujos de las burbujas y la propia caja. Se recuerda entre arranques
// (en Windows, `escala_interfaz` del estado de la ventana; aqui, en los
// ajustes del Mac: no viaja, es de esta pantalla).

import SwiftUI

public enum EscalaChat {
    /// Los escalones, en por ciento (los de Windows).
    public static let escalones: [Int] = [75, 90, 100, 110, 125, 150, 175, 200, 250]
    /// La de siempre: a la que vuelve ⌘0.
    public static let porDefecto = 100
    /// Donde se recuerda.
    public static let clave = "chat.escala"

    /// El escalon mas cercano a `cuanto` (lo que venga escrito a mano o de
    /// otra version acaba en uno conocido).
    public static func valida(_ cuanto: Int) -> Int {
        escalones.min { abs($0 - cuanto) < abs($1 - cuanto) } ?? porDefecto
    }

    /// El siguiente, o el mismo en el tope: seguir pulsando no da la vuelta.
    public static func siguiente(_ actual: Int) -> Int {
        let a = valida(actual)
        return escalones.first { $0 > a } ?? a
    }

    /// El anterior, o el mismo en el minimo.
    public static func anterior(_ actual: Int) -> Int {
        let a = valida(actual)
        return escalones.last { $0 < a } ?? a
    }

    /// Lo que se dice al cambiarla («Escala de la interfaz: 125 %»). Se
    /// dice siempre, aunque no cambie: sin aviso, pulsar en el tope parece
    /// que no funciona.
    public static func aviso(_ cuanto: Int) -> String {
        "Escala de la interfaz: \(cuanto) %"
    }

    /// Que hace una tecla con ⌘ (`+`, `=`, `-`, `0`), o `nil` si no es suya.
    /// El `=` es el `+` sin mayusculas del teclado de EE. UU.
    public static func pedida(tecla: String, actual: Int) -> Int? {
        switch tecla {
        case "+", "=": siguiente(actual)
        case "-", "−": anterior(actual)
        case "0": porDefecto
        default: nil
        }
    }
}

private struct ClaveEscalaChat: EnvironmentKey {
    static let defaultValue: CGFloat = 1
}

extension EnvironmentValues {
    /// La escala del chat como factor (1 = 100 %), para lo que tiene medida
    /// fija en una burbuja (fotos, dibujos).
    var escalaChat: CGFloat {
        get { self[ClaveEscalaChat.self] }
        set { self[ClaveEscalaChat.self] = newValue }
    }
}
