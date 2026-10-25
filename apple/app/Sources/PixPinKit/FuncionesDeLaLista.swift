// **La pestana Funciones** de la seccion Proyectos (Windows 2e0da60,
// 768b73f, a274ac0 y 9620ac8, 25-oct-2026; el Mac `FuncionesDeLaLista.swift`;
// el usuario: «que Figuras sea como Funciones, donde solo aparezcan unas
// pocas funciones fuertes … y quitarlas de los chats»).
//
// La segunda parte del interruptor de la lista (`InterruptorChatFunciones`,
// en `Proyectos/VistaProyectos.swift`). Elegida, la columna de la lista de
// chats (estrecha, la pantalla entera) ensena una rejilla de mosaicos
// grandes con su icono, su titulo y una linea de descripcion: Grabar
// reunion, Nuevo lienzo, Tareas, Timeline, Horario, Capturas y Figuras. Las
// cuentas son de Rust (`funciones.rs` del nucleo, sobre `pixpin_ui::funciones`,
// el mismo crate que Windows y el Mac): el orden, los textos del catalogo,
// donde cae cada mosaico y lo que hace cada tecla. Aqui se pinta, se sigue
// el dedo o el puntero y se hace lo tocado.
//
// En el iPad cada funcion es su seccion de la barra lateral (Tareas,
// Timeline, Galeria) o su hoja encima (Grabar clase, Figuras); el lienzo
// nuevo se abre a pantalla completa. Con teclado: las flechas, Inicio y
// Fin recorren, Tab da la vuelta, Intro o Espacio hacen el elegido y Escape
// vuelve al chat (`docs/ui-funciones-y-chats.md` de Windows).

import PixPinCore
import SwiftUI

/// Los textos de Funciones y del interruptor, del catalogo de Windows en el
/// idioma de la app (los mismos que `main.ftl`).
enum TextosFunciones {
    static let actuales = textosDeFunciones(locale: Locale.preferredLanguages.first ?? "es")
}

/// Las vistas de Tareas a las que se va desde fuera: el mosaico Horario abre
/// Tareas en su vista Horario (Windows 9620ac8: el horario vive dentro de
/// Tareas). La seccion Tareas lee `VistaTareas.vistaPedida` al aparecer y
/// escucha `.pixpinAbrirVistaTareas` si ya estaba a la vista.
public enum VistaPedidaDeTareas: String {
    case horario
}

extension VistaTareas {
    /// La vista que se pidio antes de que la seccion existiera.
    @MainActor static var vistaPedida: VistaPedidaDeTareas?

    /// **Abre Tareas en `vista`** (pasa a la seccion y cambia a ella).
    @MainActor
    static func abrir(vista: VistaPedidaDeTareas) {
        vistaPedida = vista
        Navegacion.compartida.cerrarVisor()
        Navegacion.compartida.seccion = .tareas
        NotificationCenter.default.post(name: .pixpinAbrirVistaTareas, object: vista.rawValue)
    }
}

extension Notification.Name {
    /// Abrir Tareas en una vista (`VistaPedidaDeTareas.rawValue` en `object`).
    static let pixpinAbrirVistaTareas = Notification.Name("PixPinAbrirVistaTareas")
}

/// La rejilla de Funciones, en el sitio de la lista de chats.
struct FuncionesDeLaLista: View {
    @ObservedObject var almacen: Almacen
    /// El chat abierto: «Nuevo lienzo» y «Grabar» van a el (sin ninguno, a
    /// «Mensajes guardados»).
    let abierto: String?
    /// Escape: de vuelta a Chat.
    let volver: () -> Void
    /// El mosaico bajo el puntero.
    @State private var sobre: Int?
    /// El elegido con el teclado.
    @State private var elegida: UInt32?
    /// El teclado es de la rejilla.
    @FocusState private var foco: Bool
    private let textos = TextosFunciones.actuales
    private let medidas = medidasDeFunciones()

    var body: some View {
        GeometryReader { geo in
            let g = rejillaDeFunciones(ancho: UInt32(max(0, geo.size.width)),
                                       alto: UInt32(max(0, geo.size.height)))
            ZStack(alignment: .topLeading) {
                Paleta.proyectos.lista
                if g.titulo.ancho > 0 {
                    Text(textos.titulo)
                        .font(.system(size: CGFloat(medidas.tamTituloRejilla)))
                        .foregroundStyle(Paleta.proyectos.texto)
                        .lineLimit(1)
                        .frame(width: CGFloat(g.titulo.ancho), height: CGFloat(g.titulo.alto), alignment: .leading)
                        .offset(x: CGFloat(g.titulo.x), y: CGFloat(g.titulo.y))
                }
                ForEach(textos.mosaicos.indices, id: \.self) { i in
                    if i < g.mosaicos.count {
                        let caja = g.mosaicos[i]
                        MosaicoFuncion(mosaico: textos.mosaicos[i], medidas: medidas,
                                       soloIconos: g.soloIconos, conDetalle: g.conDetalle,
                                       elegido: foco && elegida == UInt32(i), sobre: sobre == i)
                            .frame(width: CGFloat(caja.ancho), height: CGFloat(caja.alto))
                            .offset(x: CGFloat(caja.x), y: CGFloat(caja.y))
                            .onHover { dentro in
                                if dentro { sobre = i } else if sobre == i { sobre = nil }
                            }
                            .onTapGesture {
                                elegida = UInt32(i)
                                hacer(textos.mosaicos[i].funcion)
                            }
                    }
                }
            }
            .frame(width: geo.size.width, height: geo.size.height, alignment: .topLeading)
            .clipped()
            .contentShape(Rectangle())
            .focusable()
            .focusEffectDisabled()
            .focused($foco)
            .onKeyPress(keys: [.leftArrow, .rightArrow, .upArrow, .downArrow, .home, .end,
                               .tab, .return, .space, .escape]) { k in
                tecla(k, columnas: g.columnas)
            }
        }
        // Al elegir la pestana el teclado (si hay uno) es suyo.
        .onAppear { foco = true }
    }

    private func tecla(_ k: KeyPress, columnas: UInt32) -> KeyPress.Result {
        guard k.modifiers.isDisjoint(with: [.command, .control, .option]) else { return .ignored }
        let t: TeclaDeFunciones
        switch k.key {
        case .leftArrow: t = .izquierda
        case .rightArrow: t = .derecha
        case .upArrow: t = .arriba
        case .downArrow: t = .abajo
        case .home: t = .inicio
        case .end: t = .fin
        case .tab: t = .tab
        case .return, .space: t = .hacer
        case .escape: t = .escape
        default: return .ignored
        }
        let r = teclaEnFunciones(elegida: elegida, tecla: t, columnas: columnas)
        elegida = r.elegida
        if let f = r.hacer { hacer(f) }
        if r.volver {
            foco = false
            volver()
        }
        return .handled
    }

    /// **Hace la funcion**: cada una a su seccion o a su hoja, como hacian
    /// los botones de la cabecera de la lista; el chat sigue a lo suyo.
    private func hacer(_ f: FuncionDeLaLista) {
        let nav = Navegacion.compartida
        switch f {
        case .grabar:
            _ = Task<Void, Never> {
                guard let chat = await destino() else { return }
                GrabarClase.abrir(chat: chat, almacen: almacen)
            }
        case .lienzo: _ = Task<Void, Never> { await nuevoLienzo() }
        case .tareas: nav.seccion = .tareas
        // Con las lecciones en su seccion.
        case .timeline: nav.seccion = .timeline
        case .horario: VistaTareas.abrir(vista: .horario)
        case .galeria: nav.seccion = .galeria
        case .figuras: VistaFiguras.abrir(almacen: almacen, chat: abierto)
        }
    }

    /// El chat abierto o, sin ninguno, «Mensajes guardados».
    private func destino() async -> String? {
        if let abierto { return abierto }
        if let g = almacen.chats.first(where: \.esGuardados) { return g.id }
        let nucleo = almacen.nucleo
        return await Task.detached(priority: .userInitiated) { try? nucleo.chatGuardados().id }.value
    }

    /// **Nuevo lienzo**: uno en blanco en el chat abierto (sale en su
    /// conversacion, como el «Lienzo» del clip) o, sin chat abierto, en
    /// «Mensajes guardados»; y se abre al momento a pantalla completa.
    private func nuevoLienzo() async {
        guard let chat = await destino() else {
            return almacen.ponerError("No se pudo crear el lienzo")
        }
        guard let m = await almacen.crearLienzo(en: chat) else { return }
        // Sube en la lista, como en Windows (`subir_en_la_lista`).
        await almacen.cargarChats()
        Navegacion.compartida.abrir(.lienzo(chat: chat, mensaje: m, foto: nil))
    }
}

/// Un mosaico: rectangulo redondeado del color del buscador con el circulo
/// de color y su icono en blanco, el titulo y debajo su linea. Bajito, el
/// icono a la izquierda del titulo y sin la linea; muy estrecho, solo el
/// circulo. Con el puntero encima, el color de fila resaltada; elegido con
/// el teclado, el de fila elegida con el texto en blanco.
struct MosaicoFuncion: View {
    let mosaico: MosaicoDeFunciones
    let medidas: MedidasDeFunciones
    let soloIconos: Bool
    let conDetalle: Bool
    let elegido: Bool
    let sobre: Bool

    private var lado: CGFloat { CGFloat(medidas.iconoLado) }
    private var relleno: CGFloat { CGFloat(medidas.relleno) }

    var body: some View {
        let forma = RoundedRectangle(cornerRadius: CGFloat(medidas.radio))
        let fondo = elegido ? Paleta.proyectos.filaElegida : sobre ? Paleta.proyectos.filaEncima : Paleta.proyectos.buscador
        let colorTexto = elegido ? Color.white : Paleta.proyectos.texto
        let colorDetalle = elegido ? Color.white.opacity(0.8) : Paleta.proyectos.apagado
        ZStack(alignment: .topLeading) {
            forma.fill(fondo)
            if soloIconos {
                circulo.frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if conDetalle {
                // De arriba abajo: el icono, el titulo y su linea.
                VStack(alignment: .leading, spacing: 0) {
                    circulo
                    Text(mosaico.titulo)
                        .font(.system(size: CGFloat(medidas.tamTitulo)))
                        .foregroundStyle(colorTexto)
                        .lineLimit(1)
                        .padding(.top, 8)
                    Text(mosaico.detalle)
                        .font(.system(size: CGFloat(medidas.tamDetalle)))
                        .foregroundStyle(colorDetalle)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .padding(.top, 2)
                }
                .padding(relleno)
            } else {
                HStack(spacing: 10) {
                    circulo
                    Text(mosaico.titulo)
                        .font(.system(size: CGFloat(medidas.tamTitulo)))
                        .foregroundStyle(colorTexto)
                        .lineLimit(1)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, relleno)
                .frame(maxHeight: .infinity)
            }
        }
        .contentShape(forma)
        .hoverEffect(.highlight)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(mosaico.titulo)
        .accessibilityHint(mosaico.detalle)
        .accessibilityAddTraits(.isButton)
    }

    /// El circulo de color con el icono en blanco: el mismo de dia y de noche.
    private var circulo: some View {
        IconoMaterial(nombre: mosaico.icono, lado: lado * 0.58)
            .foregroundStyle(Color.white)
            .frame(width: lado, height: lado)
            .background(Color(hex: mosaico.color), in: Circle())
    }
}

/// **El interruptor Chat · Funciones** (`ventana_chat/interruptor.rs` y
/// `pixpin_ui::interruptor` de Windows): dos partes de 76 en una capsula,
/// una puesta. «Funciones» se queda puesta como una pestana: la columna de
/// la lista se cambia por la rejilla de Funciones; «Chat» devuelve la lista.
struct InterruptorChatFunciones: View {
    @Binding var funciones: Bool

    var body: some View {
        HStack(spacing: 0) {
            ParteInterruptor(titulo: TextosFunciones.actuales.interruptorChat, puesta: !funciones) {
                funciones = false
            }
            ParteInterruptor(titulo: TextosFunciones.actuales.interruptorFunciones, puesta: funciones) {
                funciones = true
            }
        }
        .padding(3)
        .background(Paleta.proyectos.buscador, in: Capsule())
    }
}

/// Una parte del interruptor: puesta, en la capsula de fila elegida con el
/// texto en blanco; si no, plana, gris con el puntero encima.
struct ParteInterruptor: View {
    let titulo: String
    let puesta: Bool
    let accion: () -> Void
    @State private var encima = false

    var body: some View {
        Button(action: accion) {
            Text(titulo)
                .font(.system(size: 13, weight: puesta ? .medium : .regular))
                .foregroundStyle(puesta ? Color.white : Paleta.proyectos.texto)
                .frame(width: 84, height: 28)
                .background(puesta ? Paleta.proyectos.filaElegida
                    : encima ? Paleta.proyectos.botonEncima : .clear, in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { encima = $0 }
        .accessibilityAddTraits(puesta ? .isSelected : [])
    }
}
