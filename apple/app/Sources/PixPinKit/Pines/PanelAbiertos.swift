// El panel «Pines abiertos» (rediseno v2 de Windows, `panel_abiertos.rs`):
// la lista de los pines que hay, agrupados por su color, para encontrar los
// que quedaron tapados, minimizados u ocultos. Y las burbujas del borde: los
// pines minimizados, mas el boton que abre el panel.
//
// Toque en una fila: trae el pin a la vista. El ojo lo oculta o lo vuelve a
// ensenar; la cruz lo cierra. Abajo, mostrar, ocultar y cerrar todos (cerrar
// se puede deshacer). Arriba, «+» pinea una nota o una mini-app nueva, o lo
// del portapapeles. La agrupacion y la busqueda son puras (`ReglasPanel`, las
// del Mac, probadas en `PinesIPadTests`).

import SwiftUI
import UIKit

/// Que es cada pin, para su icono.
enum TipoFilaPin: Equatable {
    case foto, nota, pdf, video, audio, vivo, herramienta

    var simbolo: String {
        switch self {
        case .foto: "photo"
        case .nota: "note.text"
        case .pdf: "doc.richtext"
        case .video: "film"
        case .audio: "waveform"
        case .vivo: "dot.radiowaves.left.and.right"
        case .herramienta: "checklist"
        }
    }

    /// La etiqueta de la miniatura del panel de Windows (TXT, IMG, VID...).
    var etiqueta: String {
        switch self {
        case .foto: "IMG"
        case .nota: "TXT"
        case .pdf: "PDF"
        case .video: "VID"
        case .audio: "AUD"
        case .vivo: "VIVO"
        case .herramienta: "APP"
        }
    }
}

/// Una fila del panel.
struct FilaPin: Identifiable, Equatable {
    let id: Int
    var nombre: String
    var detalle: String
    var tipo: TipoFilaPin
    /// El color de su grupo (0-7), o sin grupo.
    var grupo: Int?
    var oculto: Bool
}

/// Un grupo del panel: su color y sus filas.
struct GrupoFilas: Identifiable, Equatable {
    var grupo: Int?
    var filas: [FilaPin]
    var id: Int { grupo ?? -1 }
}

/// Los ocho colores de grupo de Windows, en el orden de la paleta.
enum ColoresGrupo {
    static let rgb: [(r: Int, g: Int, b: Int)] = [
        (219, 51, 46), (242, 115, 26), (242, 173, 26), (51, 168, 84),
        (26, 168, 173), (41, 112, 219), (122, 71, 199), (222, 71, 153),
    ]
    static let nombres = ["Rojo", "Naranja", "Ámbar", "Verde", "Cian", "Azul", "Violeta", "Rosa"]

    static func color(_ i: Int) -> Color {
        let c = rgb[((i % rgb.count) + rgb.count) % rgb.count]
        return Color(.sRGB, red: Double(c.r) / 255, green: Double(c.g) / 255, blue: Double(c.b) / 255, opacity: 1)
    }

    static func nombre(_ i: Int) -> String {
        nombres[((i % nombres.count) + nombres.count) % nombres.count]
    }
}

enum ReglasPanel {
    /// Las filas que casan con la busqueda (sin distinguir mayusculas ni
    /// tildes), en su orden. Vacia, todas.
    static func filtrar(_ filas: [FilaPin], busqueda: String) -> [FilaPin] {
        let b = busqueda.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !b.isEmpty else { return filas }
        let opciones: String.CompareOptions = [.caseInsensitive, .diacriticInsensitive]
        return filas.filter {
            $0.nombre.range(of: b, options: opciones) != nil || $0.detalle.range(of: b, options: opciones) != nil
        }
    }

    /// Las filas por grupo: los colores en el orden de la paleta y «sin
    /// grupo» al final, cada grupo con sus filas en el orden de llegada.
    static func agrupar(_ filas: [FilaPin]) -> [GrupoFilas] {
        var claves: [Int?] = []
        for f in filas where !claves.contains(where: { $0 == f.grupo }) { claves.append(f.grupo) }
        claves.sort { ($0 ?? Int.max) < ($1 ?? Int.max) }
        return claves.map { g in GrupoFilas(grupo: g, filas: filas.filter { $0.grupo == g }) }
    }
}

/// Las mini-apps que se pueden pinear, en el orden de Windows.
enum MiniAppsPin {
    static let todas: [(miniapp: String, icono: String)] = [
        ("tareas", "checklist"), ("contador", "plus.forwardslash.minus"), ("ruleta", "circle.dashed"),
        ("gastos", "eurosign.circle"), ("cronometro", "stopwatch"), ("temporizador", "timer"), ("alarma", "alarm"),
    ]
}

/// El menu «Nuevo pin»: nota, mini-apps y lo del portapapeles.
struct MenuNuevoPin<Etiqueta: View>: View {
    @ViewBuilder var etiqueta: () -> Etiqueta

    var body: some View {
        Menu {
            Button { GestorPinesIPad.compartido.pinearNota() } label: { Label("Nota", systemImage: "note.text") }
            Section("Mini-apps") {
                ForEach(MiniAppsPin.todas, id: \.miniapp) { m in
                    Button { GestorPinesIPad.compartido.pinearMiniapp(m.miniapp) } label: {
                        Label(DocumentoMini.nombre(m.miniapp), systemImage: m.icono)
                    }
                }
            }
            Button {
                if !GestorPinesIPad.compartido.pinearDesdePortapapeles() { avisarFalloPin() }
            } label: {
                Label("Pegar como pin", systemImage: "doc.on.clipboard")
            }
            if GestorPinesIPad.compartido.hayUltimoCerrado {
                Button { GestorPinesIPad.compartido.devolverUltimoCerrado() } label: {
                    Label("Devolver el último pin cerrado", systemImage: "arrow.uturn.backward")
                }
            }
        } label: {
            etiqueta()
        }
    }
}

/// El panel, flotando a la derecha de la capa (oscuro, como el de Windows).
struct VistaPinesAbiertos: View {
    @ObservedObject var gestor: GestorPinesIPad

    var body: some View {
        let filas = gestor.filas
        let grupos = ReglasPanel.agrupar(ReglasPanel.filtrar(filas, busqueda: gestor.busqueda))
        VStack(spacing: 0) {
            HStack {
                Text("Pines abiertos").font(.title3.weight(.semibold))
                Spacer()
                Text("\(filas.count)").foregroundStyle(Paleta.v2.gris)
                MenuNuevoPin {
                    Image(systemName: "plus").frame(width: 32, height: 32)
                }
                .help("Nuevo pin")
                Button { gestor.panelAbierto = false } label: {
                    Image(systemName: "xmark").frame(width: 32, height: 32)
                }
                .help("Cerrar")
            }
            .padding(.horizontal, 16)
            .padding(.top, 14)
            HStack {
                Image(systemName: "magnifyingglass").foregroundStyle(Paleta.v2.gris)
                TextField("Buscar un pin", text: $gestor.busqueda)
            }
            .padding(10)
            .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 10))
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            if filas.isEmpty {
                Spacer()
                VStack(spacing: 8) {
                    Image(systemName: "pin.slash").font(.largeTitle).foregroundStyle(Paleta.v2.gris)
                    Text("No hay pines abiertos").foregroundStyle(Paleta.v2.gris)
                    Text("Captura algo y pulsa «Pinear», o crea una nota o una mini-app con +.")
                        .font(.caption).foregroundStyle(Paleta.v2.gris).multilineTextAlignment(.center)
                }
                .padding(.horizontal, 24)
                Spacer()
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 4) {
                        ForEach(grupos) { g in
                            HStack(spacing: 6) {
                                Circle().fill(g.grupo.map(ColoresGrupo.color) ?? Paleta.v2.gris).frame(width: 9, height: 9)
                                Text((g.grupo.map(ColoresGrupo.nombre) ?? "Sin grupo").uppercased())
                                Spacer()
                                Text("\(g.filas.count)")
                            }
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(Paleta.v2.gris)
                            .padding(.horizontal, 16)
                            .padding(.top, 8)
                            ForEach(g.filas) { f in fila(f) }
                        }
                    }
                }
            }
            VStack(spacing: 8) {
                Button { gestor.ocultarTodos(false) } label: {
                    Text("Mostrar todos").frame(maxWidth: .infinity).padding(.vertical, 10)
                        .background(Paleta.v2.azulLleno, in: RoundedRectangle(cornerRadius: 10))
                }
                Button { gestor.ocultarTodos(true) } label: {
                    Text("Ocultar todos").frame(maxWidth: .infinity).padding(.vertical, 10)
                        .background(Paleta.v2.cajaElegida, in: RoundedRectangle(cornerRadius: 10))
                }
                if gestor.ultimoCerrado != nil, !gestor.puedeDeshacer {
                    Button { gestor.devolverUltimoCerrado() } label: {
                        Label("Devolver el último pin cerrado", systemImage: "arrow.uturn.backward")
                            .frame(maxWidth: .infinity).padding(.vertical, 10)
                            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Paleta.v2.linea))
                    }
                    .keyboardShortcut("t", modifiers: [.command, .shift])
                }
                if gestor.puedeDeshacer {
                    Button { gestor.deshacerCerrarTodos() } label: {
                        Text("Deshacer «Cerrar todos»").frame(maxWidth: .infinity).padding(.vertical, 10)
                            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Paleta.v2.acento))
                    }
                    .keyboardShortcut("z", modifiers: .command)
                } else {
                    Button { gestor.cerrarTodosConDeshacer() } label: {
                        Text("Cerrar todos · se puede deshacer").frame(maxWidth: .infinity).padding(.vertical, 10)
                            .foregroundStyle(Paleta.v2.rojo)
                            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Paleta.v2.rojo.opacity(0.6)))
                    }
                    .disabled(filas.isEmpty)
                }
            }
            .buttonStyle(.plain)
            .foregroundStyle(Paleta.v2.texto)
            .padding(16)
        }
        .foregroundStyle(Paleta.v2.texto)
        .buttonStyle(.borderless)
        .frame(width: 340)
        .frame(maxHeight: 620)
        .background(Paleta.v2.fondo, in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Paleta.v2.linea))
        .shadow(color: .black.opacity(0.35), radius: 18)
        .environment(\.colorScheme, .dark)
    }

    private func fila(_ f: FilaPin) -> some View {
        HStack(spacing: 12) {
            Text(f.tipo.etiqueta)
                .font(.caption.weight(.semibold))
                .frame(width: 52, height: 40)
                .background(Paleta.v2.tarjetaEncima, in: RoundedRectangle(cornerRadius: 8))
            VStack(alignment: .leading, spacing: 2) {
                Text(f.nombre).lineLimit(1).foregroundStyle(f.oculto ? Paleta.v2.gris : Paleta.v2.texto)
                Text(f.oculto ? "Oculto" : f.detalle).font(.caption).lineLimit(1)
                    .foregroundStyle(f.oculto ? Paleta.v2.naranja : Paleta.v2.gris)
            }
            Spacer(minLength: 4)
            Button { gestor.alternarOculto(f.id) } label: {
                Image(systemName: f.oculto ? "eye.slash" : "eye").frame(width: 36, height: 36)
            }
            .help(f.oculto ? "Mostrar" : "Ocultar")
            Button {
                if let p = gestor.pin(f.id) { gestor.cerrar(p) }
            } label: {
                Image(systemName: "xmark").frame(width: 36, height: 36)
            }
            .help("Cerrar")
        }
        .foregroundStyle(Paleta.v2.gris)
        .padding(.horizontal, 16)
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onTapGesture { gestor.traer(f.id) }
    }
}

/// Las burbujas del borde derecho: el boton del panel (con cuantos hay) y
/// un circulo por pin minimizado. Solo con algun pin abierto.
struct BurbujasPines: View {
    @ObservedObject var gestor: GestorPinesIPad

    var body: some View {
        let minimizados = gestor.pines.filter { $0.minimizado && !$0.oculto }
        if !gestor.pines.isEmpty || gestor.puedeDeshacer || gestor.ultimoCerrado != nil {
            VStack(spacing: 10) {
                Button { gestor.panelAbierto.toggle() } label: {
                    ZStack(alignment: .topTrailing) {
                        Image(systemName: "pin.fill")
                            .font(.system(size: 18, weight: .semibold))
                            .frame(width: 44, height: 44)
                            .background(Paleta.v2.fondo.opacity(0.92), in: Circle())
                        Text("\(gestor.pines.count)")
                            .font(.caption2.weight(.bold))
                            .padding(.horizontal, 5)
                            .background(Paleta.v2.acento, in: Capsule())
                            .offset(x: 4, y: -4)
                    }
                }
                .foregroundStyle(Paleta.v2.texto)
                .help("Pines abiertos")
                ForEach(minimizados) { p in
                    BurbujaPin(pin: p, gestor: gestor)
                }
            }
            .padding(.trailing, 8)
            .shadow(color: .black.opacity(0.3), radius: 6)
        }
    }
}

/// Un pin minimizado: su foto (o su icono) en un circulo. Toque: vuelve;
/// mantener: cerrar.
struct BurbujaPin: View {
    @ObservedObject var pin: PinIPad
    let gestor: GestorPinesIPad

    var body: some View {
        Button { gestor.traer(pin.id) } label: {
            Group {
                if let vista = pin.vista {
                    Image(decorative: vista, scale: 1).resizable().scaledToFill()
                } else {
                    Image(systemName: pin.fila.tipo.simbolo)
                        .font(.system(size: 20))
                        .foregroundStyle(Paleta.v2.texto)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(Paleta.v2.tarjeta)
                }
            }
            .frame(width: ReglasCapa.ladoBurbuja, height: ReglasCapa.ladoBurbuja)
            .clipShape(Circle())
            .overlay(Circle().strokeBorder(pin.grupo.map(ColoresGrupo.color) ?? Color.white.opacity(0.6), lineWidth: 2))
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button { gestor.traer(pin.id) } label: { Label("Mostrar", systemImage: "arrow.up.left.and.arrow.down.right") }
            Button(role: .destructive) { gestor.cerrar(pin) } label: { Label("Cerrar", systemImage: "xmark") }
        }
        .help(pin.fila.nombre)
    }
}

/// El boton de la barra de herramientas para la raiz: abre «Pines
/// abiertos»; mantenerlo pulsado da el menu de pines nuevos.
public struct BotonPinesAbiertos: View {
    @ObservedObject private var gestor = GestorPinesIPad.compartido

    public init() {}

    public var body: some View {
        Button { gestor.panelAbierto.toggle() } label: {
            Label("Pines", systemImage: gestor.pines.isEmpty ? "pin" : "pin.fill")
        }
        .contextMenu {
            Button { GestorPinesIPad.compartido.pinearNota() } label: { Label("Nota", systemImage: "note.text") }
            ForEach(MiniAppsPin.todas, id: \.miniapp) { m in
                Button { GestorPinesIPad.compartido.pinearMiniapp(m.miniapp) } label: {
                    Label(DocumentoMini.nombre(m.miniapp), systemImage: m.icono)
                }
            }
            Button {
                if !GestorPinesIPad.compartido.pinearDesdePortapapeles() { avisarFalloPin() }
            } label: {
                Label("Pegar como pin", systemImage: "doc.on.clipboard")
            }
            if gestor.hayUltimoCerrado {
                Button { gestor.devolverUltimoCerrado() } label: {
                    Label("Devolver el último pin cerrado", systemImage: "arrow.uturn.backward")
                }
            }
        }
        .help("Pines abiertos")
    }
}
