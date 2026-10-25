// Los cajetines del lienzo en el iPad (avisos con campos, `UIAlertController`):
// - **calibrar** y **la cota** (`CajetinMedida.swift` del Mac, `pedir_medida`
//   y `ventana_editor/cota.rs` de Windows);
// - **las figuras** (`FigurasLienzo.swift` del Mac, el boton `Figuras` de
//   Windows): la grafica de una funcion (las formulas), la tabla en blanco,
//   pegar una tabla de Excel, los instrumentos y la biblioteca.
// La geometria y la biblioteca son de Rust (`editor_figuras.rs`). Todo entra
// en medio de la vista, agrupado, elegido y en un paso de deshacer.

import PixPinCore
import UIKit

@MainActor
enum CajetinesLienzo {
    /// Un numero tecleado con coma o con punto. `nil` si no lo es.
    nonisolated static func numero(_ texto: String) -> Float? {
        let limpio = texto.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")
        guard let v = Float(limpio), v.isFinite else { return nil }
        return v
    }

    /// Una cifra para el cajetin, sin ceros de mas y con la coma de aqui.
    nonisolated static func escrito(_ v: Float) -> String {
        let f = NumberFormatter()
        f.locale = Locale.current
        f.minimumFractionDigits = 0
        f.maximumFractionDigits = 2
        return f.string(from: NSNumber(value: v)) ?? String(v)
    }

    /// El controlador de arriba del todo de la ventana de `vista`, para
    /// presentar encima.
    static func controlador(desde vista: UIView) -> UIViewController? {
        var c = vista.window?.rootViewController
        while let p = c?.presentedViewController, !p.isBeingDismissed { c = p }
        return c
    }

    static func presentar(_ c: UIViewController, desde vista: UIView) {
        if let pop = c.popoverPresentationController {
            pop.sourceView = vista
            pop.sourceRect = CGRect(x: vista.bounds.midX, y: vista.bounds.midY, width: 1, height: 1)
            pop.permittedArrowDirections = []
        }
        controlador(desde: vista)?.present(c, animated: true)
    }

    /// Un aviso con un solo boton.
    static func avisar(_ titulo: String, _ texto: String, desde vista: UIView) {
        let a = UIAlertController(title: titulo, message: texto, preferredStyle: .alert)
        a.addAction(UIAlertAction(title: "Aceptar", style: .default))
        presentar(a, desde: vista)
    }

    /// Un aviso con campos: `aceptar` recibe lo tecleado en cada uno.
    static func pedir(_ titulo: String, _ texto: String?, campos: [(nombre: String, valor: String, numerico: Bool)],
                      boton: String, desde vista: UIView, aceptar: @escaping ([String]) -> Void) {
        let a = UIAlertController(title: titulo, message: texto, preferredStyle: .alert)
        for c in campos {
            a.addTextField { t in
                t.placeholder = c.nombre
                t.text = c.valor
                t.clearButtonMode = .whileEditing
                t.autocorrectionType = .no
                t.autocapitalizationType = .none
                if c.numerico { t.keyboardType = .numbersAndPunctuation }
            }
        }
        a.addAction(UIAlertAction(title: "Cancelar", style: .cancel))
        a.addAction(UIAlertAction(title: boton, style: .default) { [weak a] _ in
            aceptar(a?.textFields?.map { $0.text ?? "" } ?? [])
        })
        presentar(a, desde: vista)
    }

    // MARK: Medir

    /// Pregunta cuanto mide la raya de calibrar. `hecho(valor, unidad)` solo
    /// si se acepta con algo que valga.
    static func calibrar(unidades: [String], desde vista: UIView, hecho: @escaping (Float, String) -> Void) {
        let lista = unidades.isEmpty ? ["m"] : unidades
        pedir("¿Cuánto mide de verdad?",
              "Desde ahora las cotas de este lienzo dirán sus medidas en esa unidad (\(lista.joined(separator: ", "))).",
              campos: [("Medida", "", true), ("Unidad", lista[0], false)], boton: "Calibrar", desde: vista) { v in
            guard v.count == 2, let valor = numero(v[0]), valor > 0 else { return }
            let u = v[1].trimmingCharacters(in: .whitespaces)
            hecho(valor, lista.first { $0.lowercased() == u.lowercased() } ?? lista[0])
        }
    }

    /// Pregunta el largo y el angulo de la cota recien trazada, rellenos con
    /// lo que mide. `hecho(largo, grados)` solo si se acepta.
    static func dictarCota(largo: Float, grados: Float, unidad: String, desde vista: UIView,
                           hecho: @escaping (Float, Float) -> Void) {
        let texto = unidad.isEmpty
            ? "Sin calibrar, el largo va en píxeles del lienzo."
            : "El largo va en \(unidad). El ángulo, como en un plano: 0° a la derecha, 90° hacia arriba."
        pedir("Medida de la cota", texto,
              campos: [(unidad.isEmpty ? "Largo" : "Largo (\(unidad))", escrito(largo), true),
                       ("Ángulo (°)", escrito(grados), true)],
              boton: "Aplicar", desde: vista) { v in
            guard v.count == 2, let l = numero(v[0]), l > 0, let g = numero(v[1]) else { return }
            hecho(l, g)
        }
    }
}

// MARK: - Figuras

extension ModeloEditorLienzo {
    /// Las figuras guardadas en la biblioteca (para el menu).
    func figurasGuardadas() -> [FiguraDeBiblioteca] {
        (try? nucleo?.figurasGuardadas()) ?? []
    }

    /// Un instrumento de graficar (1 la recta, 2 el plano, 3 el espacio).
    func estamparInstrumento(_ ejes: UInt8) {
        guard let vista else { return }
        let c = vista.centroMundo
        if editor.estamparInstrumento(ejes: ejes, centroX: Float(c.x), centroY: Float(c.y)) { vista.trasCambio() }
    }

    func estamparGuardada(_ id: String) {
        guard let vista, let nucleo else { return }
        let c = vista.centroMundo
        if (try? nucleo.estamparFigura(editor: editor, id: id, centroX: Float(c.x), centroY: Float(c.y))) == true {
            vista.trasCambio()
        }
    }

    func quitarFigura(_ id: String) {
        _ = try? nucleo?.quitarFigura(id: id)
        objectWillChange.send()
    }

    /// Una tabla con estas celdas en medio de la vista.
    func estamparTabla(_ celdas: [[String]], cabecera: Bool) {
        guard let vista else { return }
        let c = vista.centroMundo
        if editor.estamparTabla(celdas: celdas, cabecera: cabecera, centroX: Float(c.x), centroY: Float(c.y)) {
            vista.trasCambio()
        }
    }

    /// **Pegar tabla**: las celdas que trae el portapapeles (renglones y
    /// tabuladores, lo que deja copiar unas celdas de Excel o de Numbers).
    func pegarTabla() {
        guard let vista else { return }
        guard let texto = UIPasteboard.general.string, let celdas = celdasDeTexto(texto: texto) else {
            CajetinesLienzo.avisar("Figuras", "El portapapeles no trae una tabla", desde: vista)
            return
        }
        estamparTabla(celdas, cabecera: true)
    }

    /// **La tabla en blanco**: cuantas filas y columnas.
    func pedirTablaEnBlanco() {
        guard let vista else { return }
        CajetinesLienzo.pedir("Tabla en blanco", "La primera fila va de cabecera.",
                              campos: [("Filas", "4", true), ("Columnas", "3", true)],
                              boton: "Insertar", desde: vista) { [weak self] v in
            guard v.count == 2 else { return }
            let f = min(50, max(1, Int(v[0].trimmingCharacters(in: .whitespaces)) ?? 4))
            let c = min(20, max(1, Int(v[1].trimmingCharacters(in: .whitespaces)) ?? 3))
            self?.estamparTabla(Array(repeating: Array(repeating: "", count: c), count: f), cabecera: true)
        }
    }

    /// **La grafica de una funcion**: las formulas (una curva por renglon,
    /// separadas por «;;») y su ventana, como el cajetin de Windows; si algo
    /// no se entiende, lo dice y se vuelve a pedir con lo tecleado.
    func pedirGrafica(formulas: String = "sin(x)", limites: [String] = ["-5", "5", "-3", "3", "40"]) {
        guard let vista else { return }
        let campos: [(nombre: String, valor: String, numerico: Bool)] = [
            ("y = (x^2 - 2x · sin(x)/x · x^2 si x<0; 2x si x>=0)", formulas, false),
            ("x desde", limites[0], true), ("x hasta", limites[1], true),
            ("y desde", limites[2], true), ("y hasta", limites[3], true),
            ("Píxeles por unidad", limites[4], true),
        ]
        CajetinesLienzo.pedir("Gráfica de una función",
                              "Una curva por renglón (separadas por «;;»). Por partes: x^2 si x<0; 2x si x>=0",
                              campos: campos, boton: "Insertar", desde: vista) { [weak self] v in
            guard let self, let vista = self.vista, v.count == 6 else { return }
            let lineas = v[0].components(separatedBy: ";;")
            let n = v[1...].map { Double(CajetinesLienzo.numero($0) ?? .nan) }
            let c = vista.centroMundo
            do {
                if try self.editor.estamparGrafica(formulas: lineas, xDesde: n[0], xHasta: n[1], yDesde: n[2],
                                                   yHasta: n[3], escala: n[4], centroX: Float(c.x),
                                                   centroY: Float(c.y)) {
                    vista.trasCambio()
                }
            } catch {
                let aviso = avisoDeGrafica(formulas: lineas, xDesde: n[0], xHasta: n[1], yDesde: n[2],
                                           yHasta: n[3], escala: n[4]) ?? "\(error)"
                let a = UIAlertController(title: "Gráfica de una función", message: aviso, preferredStyle: .alert)
                a.addAction(UIAlertAction(title: "Aceptar", style: .default) { [weak self] _ in
                    self?.pedirGrafica(formulas: v[0], limites: Array(v[1...]))
                })
                CajetinesLienzo.presentar(a, desde: vista)
            }
        }
    }

    /// **Guardar lo elegido como figura**, con su nombre (como el Mac: una
    /// plantilla de serie trae ya el suyo con su categoria,
    /// `nombrePropuestoDeFigura`).
    func guardarComoFigura() {
        guard let vista, let nucleo, editor.cuantosElegidos() > 0 else { return }
        CajetinesLienzo.pedir("Guardar como figura", nil, campos: [("Nombre", editor.nombrePropuestoDeFigura(), false)],
                              boton: "Guardar", desde: vista) { [weak self] v in
            guard let self, let nombre = v.first, !nombre.trimmingCharacters(in: .whitespaces).isEmpty else { return }
            _ = try? nucleo.guardarFigura(editor: self.editor, nombre: nombre)
            self.objectWillChange.send()
        }
    }
}
