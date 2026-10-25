// **La barra de arriba comun** de Galeria, Tareas, Lecciones, Timeline y
// Buscar, como `cabecera.rs` de Windows y `Cabecera.swift` del Mac: 64 de
// alto en #1C1C1E con una linea blanca al 7 % debajo; el titulo (17,
// negrita) y su subtitulo (12, gris) a la izquierda, el buscador justo
// despues (hasta 520) y los botones propios de cada seccion a la derecha
// (40 de alto, radio 10, #2C2C2E).
//
// En el iPad no hay ventanas que cerrar ni arrastrar: es la cabecera de la
// seccion (la barra lateral hace de bandeja). Mismos nombres que en el Mac
// (`Cabecera`, `BuscadorCabecera`, `BotonCabecera`, `SegmentosCabecera`,
// `ventanaConCabecera()`) para que las vistas copiadas del Mac (Lecciones)
// compilen sin tocarlas.

import SwiftUI

/// El buscador de la barra: el texto lo lleva cada seccion.
public struct BuscadorCabecera {
    let texto: Binding<String>
    let pista: String
    /// Con el foco nada mas aparecer («Buscar en PixPin»: se abre para
    /// escribir).
    let enfocar: Bool

    public init(_ texto: Binding<String>, pista: String, enfocar: Bool = false) {
        self.texto = texto
        self.pista = pista
        self.enfocar = enfocar
    }
}

public struct Cabecera<Botones: View>: View {
    public static var alto: CGFloat { 64 }

    let titulo: String
    let subtitulo: String
    let buscador: BuscadorCabecera?
    let botones: Botones
    @Environment(\.horizontalSizeClass) private var clase

    public init(_ titulo: String, subtitulo: String = "", buscador: BuscadorCabecera? = nil,
                @ViewBuilder botones: () -> Botones) {
        self.titulo = titulo
        self.subtitulo = subtitulo
        self.buscador = buscador
        self.botones = botones()
    }

    /// En una ventana estrecha (Split View, Slide Over) el titulo encoge.
    private var anchoTitulo: CGFloat { clase == .compact ? 120 : 200 }

    public var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 3) {
                Text(titulo)
                    .font(.system(size: 17, weight: .bold))
                    .foregroundStyle(Paleta.v2.texto)
                if !subtitulo.isEmpty {
                    Text(subtitulo)
                        .font(.system(size: 12))
                        .foregroundStyle(Paleta.v2.gris)
                }
            }
            .lineLimit(1)
            .truncationMode(.tail)
            .frame(width: anchoTitulo, alignment: .leading)
            .padding(.leading, 20)
            if let buscador {
                CajaBuscador(buscador: buscador)
                    .frame(minWidth: 120, maxWidth: 520)
                    .padding(.leading, 12)
                    .layoutPriority(-1)
            }
            Spacer(minLength: 12)
            HStack(spacing: 10) {
                botones
            }
            .padding(.trailing, 16)
        }
        .frame(height: Self.alto)
        .frame(maxWidth: .infinity)
        .background(Paleta.v2.fondo)
        .overlay(alignment: .bottom) {
            Paleta.v2.linea.frame(height: 1)
        }
        .environment(\.colorScheme, .dark)
    }
}

public extension Cabecera where Botones == EmptyView {
    init(_ titulo: String, subtitulo: String = "", buscador: BuscadorCabecera? = nil) {
        self.init(titulo, subtitulo: subtitulo, buscador: buscador) { EmptyView() }
    }
}

/// La caja de buscar: lupa, el texto o su pista y, con algo escrito, la ✕
/// de vaciarlo. ⌘F (teclado del iPad) le da el foco.
struct CajaBuscador: View {
    let buscador: BuscadorCabecera
    @FocusState private var foco: Bool

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 13))
                .foregroundStyle(foco ? Paleta.v2.acento : Paleta.v2.gris)
            TextField("", text: buscador.texto,
                      prompt: Text(buscador.pista).foregroundStyle(Paleta.v2.gris))
                .textFieldStyle(.plain)
                .font(.system(size: 15))
                .foregroundStyle(Paleta.v2.texto)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .submitLabel(.search)
                .focused($foco)
            if !buscador.texto.wrappedValue.isEmpty {
                Button { buscador.texto.wrappedValue = "" } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 15))
                        .foregroundStyle(Paleta.v2.gris)
                        .frame(width: 30, height: 30)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Vaciar")
            }
        }
        .padding(.leading, 12)
        .padding(.trailing, 6)
        .frame(height: 40)
        .background(Color(hex: 0x2A2A2D), in: RoundedRectangle(cornerRadius: 10))
        .overlay {
            RoundedRectangle(cornerRadius: 10)
                .strokeBorder(foco ? Paleta.v2.acento : Color.white.opacity(0.1), lineWidth: 1)
        }
        .onAppear { if buscador.enfocar { foco = true } }
        .background {
            // ⌘F con el teclado del iPad: al buscador.
            Button("") { foco = true }
                .keyboardShortcut("f")
                .opacity(0)
                .allowsHitTesting(false)
        }
    }
}

/// La chapita de un atajo («⌘Z»), como `ui::chapa` de Windows.
struct Chapa: View {
    let texto: String

    var body: some View {
        Text(texto)
            .font(.system(size: 11.5, weight: .medium))
            .foregroundStyle(Paleta.v2.cuerpo)
            .padding(.horizontal, 5)
            .frame(height: 20)
            .background(Color(hex: 0x2E2E31), in: RoundedRectangle(cornerRadius: 5))
            .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(Color.white.opacity(0.12), lineWidth: 1))
    }
}

/// Un boton propio de la seccion en la barra: icono y rotulo (o solo
/// icono, cuadrado), 40 de alto, #2C2C2E; encendido, azul lleno. `tenue`:
/// sin fondo.
public struct BotonCabecera: View {
    let icono: String?
    let rotulo: String
    let ayuda: String
    var activo = false
    var tenue = false
    let accion: () -> Void
    @Environment(\.isEnabled) private var habilitado

    public init(icono: String? = nil, rotulo: String = "", ayuda: String = "", activo: Bool = false,
                tenue: Bool = false, accion: @escaping () -> Void) {
        self.icono = icono
        self.rotulo = rotulo
        self.ayuda = ayuda.isEmpty ? rotulo : ayuda
        self.activo = activo
        self.tenue = tenue
        self.accion = accion
    }

    private var fondo: Color {
        if activo { return Paleta.v2.azulLleno }
        if tenue { return .clear }
        return Paleta.v2.caja
    }

    public var body: some View {
        Button(action: accion) {
            HStack(spacing: 8) {
                if let icono {
                    Image(systemName: icono)
                        .font(.system(size: tenue ? 14 : 15, weight: .regular))
                        .frame(width: 20)
                }
                if !rotulo.isEmpty {
                    Text(rotulo).font(.system(size: 13.5, weight: .medium)).lineLimit(1)
                }
            }
            .foregroundStyle(activo ? Color.white : tenue ? Paleta.v2.gris : Paleta.v2.texto)
            .padding(.horizontal, rotulo.isEmpty ? 0 : 14)
            .frame(minWidth: 40, minHeight: 40)
            .background(fondo, in: RoundedRectangle(cornerRadius: 10))
            .opacity(habilitado ? 1 : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 10))
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
        .help(ayuda)
        .accessibilityLabel(ayuda)
    }
}

/// El **control segmentado** de la barra (las pestanas del timeline): un
/// contenedor #2C2C2E con radio 12 y el elegido en una pildora azul.
public struct SegmentosCabecera<Valor: Hashable>: View {
    let opciones: [(Valor, String)]
    @Binding var eleccion: Valor

    public init(_ opciones: [(Valor, String)], eleccion: Binding<Valor>) {
        self.opciones = opciones
        _eleccion = eleccion
    }

    public var body: some View {
        HStack(spacing: 0) {
            ForEach(opciones.indices, id: \.self) { i in
                let (valor, rotulo) = opciones[i]
                Button { eleccion = valor } label: {
                    Text(rotulo)
                        .font(.system(size: 13.5))
                        .lineLimit(1)
                        .foregroundStyle(valor == eleccion ? Color.white : Paleta.v2.texto)
                        .padding(.horizontal, 12)
                        .frame(maxHeight: .infinity)
                        .background { Capsule().fill(valor == eleccion ? Paleta.v2.acento : .clear) }
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(3)
        .frame(height: 40)
        .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 12))
    }
}

public extension View {
    /// **Una seccion de la barra comun**, como las ventanas de Windows:
    /// oscura siempre (aunque el iPad vaya en claro), con el fondo #1C1C1E y
    /// sin la barra de navegacion del sistema (la `Cabecera` hace de titulo).
    func ventanaConCabecera() -> some View {
        background(Paleta.v2.fondo)
            .toolbar(.hidden, for: .navigationBar)
            .environment(\.colorScheme, .dark)
    }
}

extension String {
    /// La primera letra en mayuscula («hoy · jueves» → «Hoy · jueves»).
    var capitalizedPrimera: String {
        guard let p = first else { return self }
        return p.uppercased() + dropFirst()
    }
}

/// El aviso de abajo de las secciones («Llevada a la papelera», con
/// «Deshacer» si se puede), como el de Windows.
struct AvisoAbajo: View {
    let texto: String
    var esError = false
    var deshacer: (() -> Void)?

    var body: some View {
        HStack(spacing: 12) {
            Text(texto).font(.system(size: 13.5)).foregroundStyle(Paleta.v2.texto).lineLimit(2)
            if let deshacer {
                Button("Deshacer", action: deshacer)
                    .buttonStyle(.plain)
                    .foregroundStyle(Paleta.v2.acento)
                    .keyboardShortcut("z")
            }
        }
        .padding(.horizontal, 16)
        .frame(minHeight: 44)
        .background(esError ? Paleta.v2.rojo.opacity(0.35) : Paleta.v2.tarjetaEncima, in: Capsule())
        .overlay(Capsule().strokeBorder(Color.white.opacity(0.08)))
        .shadow(color: .black.opacity(0.35), radius: 12, y: 4)
        .padding(.bottom, 16)
        .transition(.move(edge: .bottom).combined(with: .opacity))
    }
}
