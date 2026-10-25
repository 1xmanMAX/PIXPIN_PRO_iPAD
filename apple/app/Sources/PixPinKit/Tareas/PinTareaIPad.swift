// **El pin de una tarea** (Windows `pines.rs` + «📌 Fijar como pin» de
// Tareas, 17-oct; Mac `Tareas/PinTarea.swift`): una tarea sola fijada
// encima de todo como recordatorio, con su titulo, su prioridad, su hora,
// sus #etiquetas y la nota de por que no se esta haciendo («💬 ¿Por qué no
// ahora?»). La nota va DENTRO de la tarea (tras `💬`, `ponerNotaTarea`),
// asi que viaja al movil y se ve en Tareas.
//
// La tarea se reconoce por su titulo y no por su numero: el chat o el movil
// pueden reordenar la lista mientras el pin esta fuera. Se relee al cambiar
// algo (`pixpinCambioLocal`, `pixpinTareasCambiadas` y tras sincronizar).
//
// En el iPad flota **dentro de PixPin** (iOS no deja dibujar encima de
// otras apps): una capa propia por encima de las secciones y de los
// visores, que deja pasar los toques fuera de las tarjetas (la de los
// avisos). Se arrastra por su barra y vuelve al abrir la app.

import PixPinCore
import SwiftUI
import UIKit

// MARK: - El modelo

@MainActor
final class ModeloPinTarea: ObservableObject, Identifiable {
    let chat: String
    let lista: String
    let titulo: String
    let nucleo: Nucleo
    @Published private(set) var marcas: MarcasTarea?
    @Published private(set) var hecha = false
    @Published private(set) var seFue = false
    /// Donde esta la tarjeta, en puntos de la capa (su esquina de arriba a
    /// la izquierda).
    @Published var sitio: CGPoint

    nonisolated static func clave(chat: String, lista: String, titulo: String) -> String {
        "\(chat)/\(lista)/\(titulo)"
    }

    let id: String

    init(chat: String, lista: String, titulo: String, nucleo: Nucleo, sitio: CGPoint) {
        (self.chat, self.lista, self.titulo, self.nucleo, self.sitio) = (chat, lista, titulo, nucleo, sitio)
        id = Self.clave(chat: chat, lista: lista, titulo: titulo)
    }

    func releer() {
        let (nucleo, chat, lista, titulo) = (self.nucleo, self.chat, self.lista, self.titulo)
        let d = desfaseLocalMs()
        Task {
            let t = await Task.detached(priority: .utility) {
                try? nucleo.tareaPorTitulo(chat: chat, lista: lista, titulo: titulo)
            }.value
            guard let t else {
                self.seFue = true
                return
            }
            self.seFue = false
            self.marcas = marcasDeTarea(texto: t.crudo, desfaseMs: d)
            self.hecha = t.hecha
        }
    }

    func ponerNota(_ nota: String) {
        let (nucleo, chat, lista, titulo) = (self.nucleo, self.chat, self.lista, self.titulo)
        let limpia = nota.trimmingCharacters(in: .whitespacesAndNewlines)
        Task {
            _ = await Task.detached(priority: .userInitiated) {
                try? nucleo.ponerNotaTarea(chat: chat, lista: lista, titulo: titulo, nota: limpia)
            }.value
            self.avisar()
            self.releer()
        }
    }

    func marcar(_ h: Bool) {
        let (nucleo, chat, lista, titulo) = (self.nucleo, self.chat, self.lista, self.titulo)
        hecha = h
        Task {
            _ = await Task.detached(priority: .userInitiated) {
                guard let t = try? nucleo.tareaPorTitulo(chat: chat, lista: lista, titulo: titulo) else { return false }
                return (try? nucleo.marcarTarea(chat: chat, mensaje: lista, indice: t.indice, crudo: t.crudo, hecha: h)) == true
            }.value
            self.avisar()
            self.releer()
        }
    }

    private func avisar() {
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: chat)
    }
}

// MARK: - La tarjeta

struct VistaPinTarea: View {
    @ObservedObject var modelo: ModeloPinTarea
    let cerrar: () -> Void
    let teclado: (Bool) -> Void
    @State private var escribiendo = false
    @State private var nota = ""
    @State private var arrastre: CGSize = .zero
    @FocusState private var foco: Bool

    static let ancho: CGFloat = 300

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 8) {
                Button { modelo.marcar(!modelo.hecha) } label: {
                    Image(systemName: modelo.hecha ? "checkmark.circle.fill" : "circle")
                        .font(.system(size: 22))
                        .foregroundStyle(modelo.hecha ? Paleta.v2.verde : Paleta.v2.gris)
                        .frame(width: 30, height: 30).contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(modelo.hecha ? "Desmarcar" : "Marcar como hecha")
                if let u = modelo.marcas?.marcaUrgencia { Text(u).padding(.top, 4) }
                Text(modelo.seFue ? "\(modelo.titulo) (ya no está)" : modelo.titulo)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(modelo.hecha || modelo.seFue ? Paleta.v2.gris : Paleta.v2.texto)
                    .strikethrough(modelo.hecha)
                    .lineLimit(3)
                    .padding(.top, 4)
                Spacer(minLength: 0)
                Button(action: cerrar) {
                    Image(systemName: "xmark").font(.system(size: 12, weight: .bold))
                        .frame(width: 30, height: 30).contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(Paleta.v2.gris)
                .accessibilityLabel("Quitar el pin")
            }
            if let m = modelo.marcas, m.horaMs != nil || !m.etiquetas.isEmpty {
                HStack(spacing: 6) {
                    if let h = m.horaMs {
                        ChapitaTarea(texto: "⏰ " + AtajosDeHora.legible(fechaDeLocales(h), ahora: Date()),
                                     color: Paleta.v2.amarillo)
                    }
                    ForEach(m.etiquetas, id: \.self) { ChapitaTarea(texto: "#\($0)", color: Paleta.v2.acento) }
                }
            }
            if escribiendo {
                TextField("", text: $nota, prompt: Text("Por qué no la haces ahora · Intro guarda")
                    .foregroundStyle(Paleta.v2.gris), axis: .vertical)
                    .lineLimit(1 ... 4)
                    .textFieldStyle(.plain)
                    .foregroundStyle(Paleta.v2.texto)
                    .padding(8)
                    .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 8))
                    .focused($foco)
                    .submitLabel(.done)
                    .onSubmit(guardar)
                    .onChange(of: foco) { _, f in if !f, escribiendo { guardar() } }
            } else if !modelo.seFue {
                Button {
                    nota = modelo.marcas?.nota ?? ""
                    escribiendo = true
                    teclado(true)
                    foco = true
                } label: {
                    Text(modelo.marcas?.nota.map { "💬 \($0)" } ?? "💬 ¿Por qué no ahora?")
                        .font(.system(size: 13))
                        .foregroundStyle(modelo.marcas?.nota == nil ? Paleta.v2.gris : Paleta.v2.cuerpo)
                        .lineLimit(3)
                        .multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(12)
        .frame(width: Self.ancho)
        .background(Paleta.v2.tarjeta, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Paleta.v2.acento.opacity(0.55)))
        .shadow(color: .black.opacity(0.35), radius: 10, y: 4)
        .environment(\.colorScheme, .dark)
        .offset(arrastre)
        // Se arrastra cogiendola por cualquier sitio que no sea un boton.
        .gesture(
            DragGesture(minimumDistance: 6, coordinateSpace: .global)
                .onChanged { arrastre = $0.translation }
                .onEnded { g in
                    modelo.sitio.x += g.translation.width
                    modelo.sitio.y += g.translation.height
                    arrastre = .zero
                    PinesDeTareasIPad.compartidos.recolocar(modelo)
                }
        )
    }

    private func guardar() {
        guard escribiendo else { return }
        escribiendo = false
        foco = false
        teclado(false)
        if nota != (modelo.marcas?.nota ?? "") { modelo.ponerNota(nota) }
    }
}

/// La capa de los pines de tarea.
struct CapaPinesTareas: View {
    @ObservedObject var pines: PinesDeTareasIPad

    var body: some View {
        ZStack(alignment: .topLeading) {
            Color.clear
            ForEach(pines.abiertos) { m in
                VistaPinTarea(modelo: m, cerrar: { pines.cerrar(m.id) }, teclado: { pines.teclado($0) })
                    .fixedSize(horizontal: false, vertical: true)
                    .offset(x: m.sitio.x, y: m.sitio.y)
                    .transition(.opacity.combined(with: .scale(scale: 0.95)))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .animation(.easeOut(duration: 0.15), value: pines.abiertos.map(\.id))
    }
}

// MARK: - Los pines abiertos

/// **Los pines de tarea abiertos**: uno por tarea, y vuelven al abrir la
/// app (se guardan en las preferencias del aparato, como los pines).
@MainActor
public final class PinesDeTareasIPad: ObservableObject {
    public static let compartidos = PinesDeTareasIPad()
    @Published private(set) var abiertos: [ModeloPinTarea] = []
    private var ventana: UIWindow?
    private weak var anterior: UIWindow?
    private var vigias: [NSObjectProtocol] = []
    private var nucleo: Nucleo?
    nonisolated static let claveGuardados = "PixPinPinesDeTareas"

    /// Lo que se guarda de un pin.
    struct Guardado: Codable, Equatable {
        var chat: String
        var lista: String
        var titulo: String
        var x: Double
        var y: Double
    }

    /// Al arrancar: vuelven los que estaban.
    func restaurar(nucleo: Nucleo) {
        self.nucleo = nucleo
        guard abiertos.isEmpty,
              let datos = UserDefaults.standard.data(forKey: Self.claveGuardados),
              let v = try? JSONDecoder().decode([Guardado].self, from: datos)
        else { return }
        // Al arrancar aun puede no haber escena donde ponerlos: se espera.
        Task {
            for _ in 0 ..< 30 where ArribaUIKit.escena() == nil {
                try? await Task.sleep(for: .milliseconds(300))
            }
            for g in v {
                self.abrir(chat: g.chat, lista: g.lista, titulo: g.titulo, nucleo: nucleo, sitio: CGPoint(x: g.x, y: g.y))
            }
        }
    }

    /// Pone el pin de la tarea `titulo` (sin marcas) de la lista `lista` del
    /// chat `chat`. Si ya estaba, lo relee.
    public func abrir(chat: String, lista: String, titulo: String, nucleo: Nucleo) {
        abrir(chat: chat, lista: lista, titulo: titulo, nucleo: nucleo, sitio: nil)
    }

    /// El de una tarea de la ventana de Tareas.
    func abrir(_ t: TareaConLista, nucleo: Nucleo) {
        let titulo = marcasDeTarea(texto: t.tarea.texto, desfaseMs: desfaseLocalMs()).titulo
        abrir(chat: t.lista.chat, lista: t.lista.mensaje, titulo: titulo, nucleo: nucleo)
    }

    private func abrir(chat: String, lista: String, titulo: String, nucleo: Nucleo, sitio: CGPoint?) {
        self.nucleo = nucleo
        let clave = ModeloPinTarea.clave(chat: chat, lista: lista, titulo: titulo)
        if let m = abiertos.first(where: { $0.id == clave }) {
            m.releer()
            return
        }
        let m = ModeloPinTarea(chat: chat, lista: lista, titulo: titulo, nucleo: nucleo,
                               sitio: sitio ?? sitioNuevo())
        m.releer()
        abiertos.append(m)
        mostrar()
        vigilar()
        guardar()
    }

    /// Abajo a la derecha, uno encima de otro (que no se tapen).
    private func sitioNuevo() -> CGPoint {
        let area = ArribaUIKit.ventana()?.bounds.size ?? CGSize(width: 1024, height: 768)
        let n = CGFloat(abiertos.count)
        return CGPoint(x: max(16, area.width - VistaPinTarea.ancho - 24),
                       y: max(80, area.height - 160 - n * 130))
    }

    func cerrar(_ id: String) {
        abiertos.removeAll { $0.id == id }
        if abiertos.isEmpty {
            teclado(false)
            ventana?.isHidden = true
        }
        guardar()
    }

    /// Tras arrastrarlo: que no se salga y se apunta donde quedo.
    func recolocar(_ m: ModeloPinTarea) {
        let area = ventana?.bounds.size ?? CGSize(width: 1024, height: 768)
        m.sitio.x = min(max(m.sitio.x, 8 - VistaPinTarea.ancho / 2), area.width - VistaPinTarea.ancho / 2)
        m.sitio.y = min(max(m.sitio.y, 8), area.height - 60)
        guardar()
    }

    /// Escribir en la nota: la capa toma el teclado mientras, y lo
    /// devuelve al acabar.
    func teclado(_ tomar: Bool) {
        if tomar {
            if ventana?.isKeyWindow != true {
                anterior = ArribaUIKit.escena()?.windows.first { $0.isKeyWindow }
                ventana?.makeKey()
            }
        } else if ventana?.isKeyWindow == true {
            anterior?.makeKey()
        }
    }

    private func guardar() {
        let v = abiertos.map { Guardado(chat: $0.chat, lista: $0.lista, titulo: $0.titulo,
                                        x: Double($0.sitio.x), y: Double($0.sitio.y)) }
        if let d = try? JSONEncoder().encode(v) {
            UserDefaults.standard.set(d, forKey: Self.claveGuardados)
        }
    }

    private func mostrar() {
        if ventana == nil, let escena = ArribaUIKit.escena() {
            let v = VentanaQueDejaPasar(windowScene: escena)
            // Encima de la app y de los visores; debajo de los avisos y de
            // las llamadas.
            v.windowLevel = UIWindow.Level(rawValue: UIWindow.Level.normal.rawValue + 5)
            v.backgroundColor = .clear
            let raiz = UIHostingController(rootView: CapaPinesTareas(pines: self))
            raiz.view.backgroundColor = .clear
            v.rootViewController = raiz
            ventana = v
        }
        ventana?.isHidden = false
    }

    private func vigilar() {
        guard vigias.isEmpty else { return }
        let local = NotificationCenter.default
        for nombre in [Notification.Name.pixpinCambioLocal, .pixpinTareasCambiadas, .pixpinTrajoCambios] {
            vigias.append(local.addObserver(forName: nombre, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated {
                    for m in PinesDeTareasIPad.compartidos.abiertos { m.releer() }
                }
            })
        }
    }

    /// Cuantos hay fuera (para las pruebas).
    var cuantos: Int { abiertos.count }
}
