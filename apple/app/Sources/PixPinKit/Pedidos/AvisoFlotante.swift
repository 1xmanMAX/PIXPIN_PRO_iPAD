// **El aviso flotante** (`ventana_chat/aviso_asistente.rs` de Windows): lo
// que contestan los pedidos, el asistente pedido desde fuera y las
// automatizaciones, sin abrir nada. Una tarjeta pequena **abajo a la
// derecha**, encima de todo lo de la app (tambien de un visor a pantalla
// completa), que no quita el foco ni tapa los toques fuera de ella:
//
// - Titulo y aspa arriba; el texto debajo, que crece hasta `altoMaximo` y lo
//   que no cabe se lee desplazandolo; los botones abajo («Abrir», «Sí»,
//   «No», «Deshacer»...).
// - Mientras trabaja (`acabado == false`, «💭 …») no se va; al acabar se va
//   sola a los `dura` segundos, salvo con el dedo, el lapiz o el puntero
//   encima (como el raton encima en Windows).
// - Varias a la vez: cada una nueva sube sobre la anterior; como mucho
//   `tope` (la mas vieja se va).
// - Quien la saca se queda la `TarjetaFlotante` para cambiar lo que dice
//   cuando tenga la respuesta (el `Mando` de Windows).
//
// En el iPad es una `UIWindow` propia encima de la escena de delante que
// solo se queda los toques de sus tarjetas; lo demas pasa a la app.

import SwiftUI
import UIKit

/// Un boton de la tarjeta.
public struct BotonAviso: Identifiable {
    public let id = UUID()
    public var titulo: String
    /// El azul lleno (el de «Abrir» o «Sí»).
    public var principal: Bool
    /// Rojo (lo que borra).
    public var peligro: Bool
    /// Si al pulsarlo se va la tarjeta (casi siempre).
    public var cierra: Bool
    public var accion: @MainActor () -> Void

    public init(_ titulo: String, principal: Bool = false, peligro: Bool = false, cierra: Bool = true,
                accion: @escaping @MainActor () -> Void) {
        self.titulo = titulo
        self.principal = principal
        self.peligro = peligro
        self.cierra = cierra
        self.accion = accion
    }
}

/// **Una tarjeta** ya en pantalla: para cambiar lo que dice.
@MainActor
public final class TarjetaFlotante: ObservableObject, Identifiable {
    public let id = UUID()
    @Published public private(set) var titulo: String
    @Published public private(set) var texto: String
    @Published public private(set) var botones: [BotonAviso]
    /// Ya esta la respuesta: cuenta para irse.
    @Published public private(set) var acabado: Bool
    /// Con el dedo o el puntero encima no se va.
    @Published var encima = false {
        didSet { if !encima { programarIrse() } }
    }

    /// Lo que se queda tras acabar.
    let dura: TimeInterval
    /// Lo que hace el aspa ademas de cerrar (una pregunta pendiente: «No»).
    var alCerrar: (@MainActor () -> Void)?
    fileprivate weak var capa: AvisoFlotante?
    private var tanda = 0
    public private(set) var cerrada = false

    init(titulo: String, texto: String, botones: [BotonAviso], acabado: Bool, dura: TimeInterval) {
        self.titulo = titulo
        self.texto = AvisoFlotante.recortar(texto)
        self.botones = botones
        self.acabado = acabado
        self.dura = dura
    }

    /// **Pone la respuesta** (y sus botones) y empieza a contar para irse.
    public func poner(_ texto: String, titulo: String? = nil, botones: [BotonAviso] = []) {
        guard !cerrada else { return }
        if let titulo { self.titulo = titulo }
        self.texto = AvisoFlotante.recortar(texto)
        self.botones = botones
        alCerrar = nil
        acabado = true
        programarIrse()
    }

    /// Cambia lo que dice **sin darla por acabada** (sigue trabajando, o
    /// espera un «Sí»/«No»: no se va sola).
    public func cambiar(_ texto: String, botones: [BotonAviso] = [], alCerrar: (@MainActor () -> Void)? = nil) {
        guard !cerrada else { return }
        self.texto = AvisoFlotante.recortar(texto)
        self.botones = botones
        self.alCerrar = alCerrar
        acabado = false
        tanda += 1
    }

    /// Se va ya (el aspa, un boton, o quien la saco).
    public func cerrar() {
        guard !cerrada else { return }
        cerrada = true
        tanda += 1
        capa?.quitar(self)
    }

    /// El aspa: lo suyo (una pregunta es un «No») y cerrar.
    func aspa() {
        let f = alCerrar
        alCerrar = nil
        f?()
        cerrar()
    }

    func pulsar(_ b: BotonAviso) {
        if b.cierra {
            alCerrar = nil
            cerrar()
        }
        b.accion()
    }

    private func programarIrse() {
        guard acabado, !cerrada, !encima else { return }
        tanda += 1
        let mia = tanda
        DispatchQueue.main.asyncAfter(deadline: .now() + dura) { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.tanda == mia, !self.encima else { return }
                self.cerrar()
            }
        }
    }

    fileprivate func empezar() { programarIrse() }

    /// Un toque o un desplazamiento: se queda mientras se lee (otro rato
    /// entero desde el ultimo toque).
    func tocar() {
        encima = true
        tanda += 1
        let mia = tanda
        DispatchQueue.main.asyncAfter(deadline: .now() + dura) { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.tanda == mia else { return }
                self.encima = false
            }
        }
    }
}

@MainActor
public final class AvisoFlotante: ObservableObject {
    public static let compartido = AvisoFlotante()

    /// Lo que se queda una respuesta del asistente tras acabar (`DURA`).
    public nonisolated static let dura: TimeInterval = 15
    /// Lo mas que se ensena de una respuesta (`LETRAS`): lo demas, en el chat.
    nonisolated static let letras = 2000
    /// Cuantas a la vez como mucho.
    nonisolated static let tope = 4
    nonisolated static let ancho: CGFloat = 340
    nonisolated static let altoMaximo: CGFloat = 420

    /// **Otro que diga los avisos** (las pruebas): titulo y texto. Con el,
    /// nada sale en pantalla, pero la tarjeta se devuelve igual.
    public static var alMostrar: ((String, String) -> Void)?

    @Published fileprivate(set) var tarjetas: [TarjetaFlotante] = []
    private var ventana: UIWindow?

    public init() {}

    /// Lo que se queda un aviso corto (el globo): mas cuanto mas largo.
    nonisolated static func duraDe(_ texto: String) -> TimeInterval {
        min(10, 4 + Double(texto.count) / 30)
    }

    /// Lo que cabe: `letras` como mucho, con «…» si se corta.
    nonisolated static func recortar(_ t: String) -> String {
        let limpio = t.trimmingCharacters(in: .whitespacesAndNewlines)
        guard limpio.count > letras else { return limpio }
        return String(limpio.prefix(letras)) + "…"
    }

    /// **Saca una tarjeta**. Sin `acabado`, se queda hasta que se le ponga
    /// la respuesta (`poner`) o se cierre. `dura`: lo que se queda tras
    /// acabar (sin el, segun lo largo del texto).
    @discardableResult
    public func mostrar(_ texto: String, titulo: String = "PixPin", acabado: Bool = true,
                        botones: [BotonAviso] = [], dura: TimeInterval? = nil,
                        alCerrar: (@MainActor () -> Void)? = nil) -> TarjetaFlotante {
        let t = TarjetaFlotante(titulo: titulo, texto: texto, botones: botones, acabado: acabado,
                                dura: dura ?? Self.duraDe(texto))
        t.alCerrar = alCerrar
        t.capa = self
        if let alMostrar = Self.alMostrar {
            alMostrar(titulo, texto)
            return t
        }
        tarjetas.append(t)
        while tarjetas.count > Self.tope {
            tarjetas.first?.cerrar()
        }
        ponerVentana()
        t.empezar()
        return t
    }

    fileprivate func quitar(_ t: TarjetaFlotante) {
        tarjetas.removeAll { $0 === t }
        if tarjetas.isEmpty { ventana?.isHidden = true }
    }

    /// La ventana de las tarjetas, en la escena de delante (con Stage
    /// Manager puede haber cambiado desde la ultima vez).
    private func ponerVentana() {
        let escenas = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        guard let escena = escenas.first(where: { $0.activationState == .foregroundActive }) ?? escenas.first else {
            return
        }
        if let v = ventana, v.windowScene === escena {
            v.isHidden = false
            return
        }
        ventana?.isHidden = true
        let v = VentanaDelAviso(windowScene: escena)
        v.windowLevel = UIWindow.Level(rawValue: UIWindow.Level.alert.rawValue + 2)
        v.backgroundColor = .clear
        let raiz = UIHostingController(rootView: CapaAvisoFlotante(capa: self))
        raiz.view.backgroundColor = .clear
        v.rootViewController = raiz
        v.isHidden = false
        ventana = v
    }
}

/// Una ventana que solo se queda los toques de sus tarjetas.
final class VentanaDelAviso: UIWindow {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let tocado = super.hitTest(point, with: event)
        return tocado === rootViewController?.view ? nil : tocado
    }
}

// MARK: - Las vistas

struct CapaAvisoFlotante: View {
    @ObservedObject var capa: AvisoFlotante

    var body: some View {
        VStack(alignment: .trailing, spacing: 10) {
            Spacer(minLength: 0)
            // La nueva sube sobre la anterior: la mas nueva, arriba.
            ForEach(capa.tarjetas.reversed()) { t in
                VistaTarjetaFlotante(t: t)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
        .padding(.trailing, 16)
        .padding(.bottom, 20)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        .animation(.easeOut(duration: 0.2), value: capa.tarjetas.map(\.id))
    }
}

struct VistaTarjetaFlotante: View {
    @ObservedObject var t: TarjetaFlotante
    @State private var altoTexto: CGFloat = 0

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                if !t.acabado, t.botones.isEmpty {
                    ProgressView().controlSize(.small)
                }
                Text(t.titulo)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(Paleta.proyectos.texto)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Button { t.aspa() } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 12, weight: .bold))
                        .frame(width: 28, height: 28)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(Paleta.proyectos.apagado)
                .accessibilityLabel("Cerrar")
            }
            ScrollView {
                Text(t.texto)
                    .font(.system(size: 13))
                    .foregroundStyle(Paleta.proyectos.texto)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(GeometryReader { g in
                        Color.clear.preference(key: AltoTextoAviso.self, value: g.size.height)
                    })
            }
            .frame(height: min(max(altoTexto, 18), AvisoFlotante.altoMaximo - 110))
            .onPreferenceChange(AltoTextoAviso.self) { altoTexto = $0 }
            .simultaneousGesture(DragGesture(minimumDistance: 2).onChanged { _ in t.tocar() })
            if !t.botones.isEmpty {
                HStack(spacing: 8) {
                    Spacer(minLength: 0)
                    ForEach(t.botones) { b in
                        Button { t.pulsar(b) } label: {
                            Text(b.titulo)
                                .font(.system(size: 13, weight: .semibold))
                                .padding(.horizontal, 12)
                                .padding(.vertical, 6)
                                .foregroundStyle(b.principal ? Color.white
                                    : b.peligro ? Paleta.v2.rojo : Paleta.proyectos.texto)
                                .background(b.principal ? Paleta.proyectos.contador : Paleta.proyectos.filaEncima,
                                            in: RoundedRectangle(cornerRadius: 8))
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .padding(.horizontal, 14)
        .padding(.top, 8)
        .padding(.bottom, 12)
        .frame(width: AvisoFlotante.ancho)
        .background(Paleta.proyectos.lista, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Paleta.proyectos.separador))
        .shadow(color: .black.opacity(0.22), radius: 12, y: 4)
        .onHover { t.encima = $0 }
        .onTapGesture { t.tocar() }
    }
}

private struct AltoTextoAviso: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}
