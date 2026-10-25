// **Los colores de Windows**, en un sitio (15-oct-2026, el usuario: que el
// Mac (y el iPad) se vea como el PixPin de Windows, plano y limpio).
//
// Dos familias, como alla:
//
// - `Paleta.v2`: las ventanas de la barra comun (`cabecera.rs`): Galeria,
//   Tareas, Lecciones y Timeline. En Windows van **siempre en oscuro**, con
//   los colores de `v2/mod.rs` (el negro apagado de iOS, #1C1C1E). Aqui
//   igual: esas ventanas piden el aspecto oscuro (`ventanaOscura()`).
// - `Paleta.proyectos`: la ventana de Proyectos (`ventana_chat.rs`), que
//   **sigue el tema del sistema**: el claro y el oscuro de Telegram
//   (`CLARO` y `OSCURO`). Aqui cada color tiene su par y SwiftUI elige.

import SwiftUI
import UIKit

extension Color {
    /// Un color de `0xRRGGBB`, como `hex()` de Windows.
    init(hex: UInt32, alfa: Double = 1) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255,
                  opacity: alfa)
    }

    /// Uno de dia y otro de noche: cambia solo con el tema del sistema.
    init(claro: UInt32, oscuro: UInt32) {
        self.init(uiColor: UIColor { rasgos in
            let h = rasgos.userInterfaceStyle == .dark ? oscuro : claro
            return UIColor(red: CGFloat((h >> 16) & 0xFF) / 255,
                           green: CGFloat((h >> 8) & 0xFF) / 255,
                           blue: CGFloat(h & 0xFF) / 255, alpha: 1)
        })
    }
}

public enum Paleta {
    /// `v2/mod.rs` de Windows: las ventanas oscuras de la barra comun.
    public enum v2 {
        public static let fondo = Color(hex: 0x1C1C1E)
        public static let tarjeta = Color(hex: 0x2A2A2D)
        public static let tarjetaEncima = Color(hex: 0x313134)
        /// Los botones y las cajas de la barra.
        public static let caja = Color(hex: 0x2C2C2E)
        /// El elegido de un conmutador de iconos.
        public static let cajaElegida = Color(hex: 0x3A3A3C)
        /// La linea de debajo de la barra: blanco al 7 %.
        public static let linea = Color.white.opacity(0.07)
        public static let texto = Color(hex: 0xF5F5F7)
        public static let cuerpo = Color(hex: 0xD1D1D6)
        public static let gris = Color(hex: 0x98989D)
        public static let acento = Color(hex: 0x0A84FF)
        public static let azulLleno = Color(hex: 0x0060DF)
        public static let rojo = Color(hex: 0xFF453A)
        public static let verde = Color(hex: 0x30D158)
        public static let naranja = Color(hex: 0xFF9F0A)
        public static let amarillo = Color(hex: 0xFFD60A)
    }

    /// `CLARO` y `OSCURO` de `ventana_chat.rs` (los de Telegram): la ventana
    /// de Proyectos.
    public enum proyectos {
        /// La franja de arriba (`titleBg`).
        public static let barra = Color(claro: 0xF1F1F1, oscuro: 0x242F3D)
        /// Un boton de la franja con el raton encima, y la ✕ (roja).
        public static let botonEncima = Color(claro: 0xE5E5E5, oscuro: 0x2C3847)
        public static let cerrarEncima = Color(claro: 0xE81123, oscuro: 0xE92539)
        /// El fondo de la lista de proyectos.
        public static let lista = Color(claro: 0xFFFFFF, oscuro: 0x17212B)
        /// El papel del chat, arriba y abajo (el «mar» de `FONDOS`).
        public static let papelArriba = Color(claro: 0x86BFB2, oscuro: 0x151E27)
        public static let papelAbajo = Color(claro: 0x8AB8D2, oscuro: 0x10161D)
        public static let separador = Color(claro: 0xE0E0E0, oscuro: 0x101921)
        public static let texto = Color(claro: 0x222222, oscuro: 0xF5F5F5)
        public static let apagado = Color(claro: 0x999999, oscuro: 0x7F91A4)
        public static let filaEncima = Color(claro: 0xF1F1F1, oscuro: 0x202B36)
        public static let filaElegida = Color(claro: 0x419FD9, oscuro: 0x2B5278)
        /// La pildora de pendientes y el boton redondo de proyecto nuevo.
        public static let contador = Color(claro: 0x40A7E3, oscuro: 0x4082BC)
        public static let buscador = Color(claro: 0xF1F1F1, oscuro: 0x242F3D)
    }

    /// Los siete colores de los avatares de Telegram; cual toca sale del
    /// codigo del proyecto (`color_avatar` de Windows), asi que no cambia.
    public static func avatar(_ codigo: String) -> Color {
        let colores: [UInt32] = [0xE17076, 0xFAA774, 0xA695E7, 0x7BC862, 0x6EC9CB, 0x65AADD, 0xEE7AAE]
        let suma = codigo.utf8.reduce(0) { $0 + Int($1) }
        return Color(hex: colores[suma % colores.count])
    }

    /// La inicial de las dos primeras palabras (`iniciales` de Windows).
    public static func iniciales(_ nombre: String) -> String {
        nombre.split(whereSeparator: \.isWhitespace).prefix(2)
            .compactMap(\.first).map { String($0).uppercased() }.joined()
    }
}
