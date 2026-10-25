// Un pin en la capa: lo que se ve, los gestos (arrastrar, pellizcar, girar
// con dos dedos) y la barra comun del pin (la de Windows: zoom, copiar,
// anotar, sacar texto, al chat, mas, minimizar y cerrar).
//
// - Una foto se arrastra por cualquier sitio, se pellizca para cambiar su
//   tamano y se gira con dos dedos (al soltar se pega al cuarto de vuelta
//   mas cercano, que es lo que guarda Windows).
// - Una nota o una mini-app tiene dentro botones y cajas de texto, asi que
//   se arrastra por su asa de arriba y cambia de tamano por la esquina.
// - Un toque lo trae delante y ensena su barra.
// - Al arrastrar salen las guias azules de los demas pines y al soltar se
//   pega a ellas (el iman de Windows).
// - Un PDF pasa de pagina desde su barra (◀ 3 / 12 ▶).
// - El asa de la mano de la barra lo arrastra a otra app (Split View o
//   Slide Over): la foto, el fichero o el texto.

import AVFoundation
import SwiftUI
import UIKit

struct VistaPinIPad: View {
    @ObservedObject var pin: PinIPad
    @ObservedObject var gestor: GestorPinesIPad

    @GestureState private var arrastre: CGSize = .zero
    @GestureState private var pellizco: CGFloat = 1
    @GestureState private var giroGesto: Angle = .zero
    /// Lo que crece por la esquina (solo herramientas).
    @GestureState private var esquina: CGSize = .zero

    private var elegido: Bool { gestor.elegido == pin.id }

    /// El tamano mientras dura un gesto.
    private var tamanoVivo: CGSize {
        if pin.esHerramienta {
            return CGSize(width: max(220, pin.tamano.width + esquina.width),
                          height: max(180, pin.tamano.height + esquina.height))
        }
        let escala = ReglasPin.escalaLimitada(pin.escala * pellizco, base: pin.tamanoBase)
        return CGSize(width: pin.tamanoBase.width * escala, height: pin.tamanoBase.height * escala)
    }

    var body: some View {
        let tam = tamanoVivo
        let barraAbajo = pin.centro.y - tam.height / 2 < 64
        contenido(tam)
            .frame(width: tam.width, height: tam.height)
            .clipShape(RoundedRectangle(cornerRadius: pin.esImagen ? 2 : 12))
            .overlay {
                RoundedRectangle(cornerRadius: pin.esImagen ? 2 : 12)
                    .strokeBorder(borde, lineWidth: elegido || pin.grupo != nil ? 2 : 0.5)
            }
            .shadow(color: .black.opacity(0.35), radius: elegido ? 14 : 8, y: 3)
            .rotationEffect(giroGesto)
            .opacity(pin.opacidad)
            .overlay(alignment: barraAbajo ? .bottom : .top) {
                if elegido {
                    BarraPinIPad(pin: pin, gestor: gestor)
                        .fixedSize()
                        .alignmentGuide(barraAbajo ? .bottom : .top) { d in
                            barraAbajo ? d[.top] - 10 : d[.bottom] + 10
                        }
                }
            }
            .position(x: pin.centro.x + arrastre.width, y: pin.centro.y + arrastre.height)
            // Un arrastre cancelado (sin `onEnded`) no deja guias colgadas.
            .onChange(of: arrastre == .zero) { _, quieto in
                if quieto { gestor.quitarGuias() }
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel(pin.fila.nombre)
    }

    private var borde: Color {
        if let g = pin.grupo { return ColoresGrupo.color(g) }
        return elegido ? Paleta.v2.acento : Color.white.opacity(0.25)
    }

    @ViewBuilder
    private func contenido(_ tam: CGSize) -> some View {
        switch pin.contenido {
        case .imagen:
            Group {
                if let vista = pin.vista {
                    Image(decorative: vista, scale: 1)
                        .resizable()
                        .interpolation(.high)
                } else {
                    Color.gray
                }
            }
            .contentShape(Rectangle())
            .onTapGesture(count: 2) { tamanoReal() }
            .onTapGesture { gestor.tocar(pin) }
            .gesture(arrastrar.simultaneously(with: pellizcar.simultaneously(with: girar)))
        case let .video(reproductor, _, audio):
            VistaVideoPin(reproductor: reproductor, soloAudio: audio, pin: pin)
                .overlay(alignment: .top) { asa }
                .simultaneousGesture(TapGesture().onEnded { gestor.elegir(pin) })
                .gesture(pellizcar)
        case let .herramienta(h):
            VStack(spacing: 0) {
                asa
                h.vista()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .background(Paleta.v2.tarjeta)
            .environment(\.colorScheme, .dark)
            .overlay(alignment: .bottomTrailing) { asaEsquina }
            .simultaneousGesture(TapGesture().onEnded { gestor.elegir(pin) })
        }
    }

    /// El asa de arriba de una herramienta o un video: se arrastra por aqui.
    private var asa: some View {
        HStack(spacing: 6) {
            Image(systemName: "line.3.horizontal")
                .font(.caption.weight(.bold))
            Text(pin.fila.nombre).font(.caption.weight(.semibold)).lineLimit(1)
            Spacer(minLength: 0)
        }
        .foregroundStyle(Paleta.v2.gris)
        .padding(.horizontal, 10)
        .frame(height: 30)
        .frame(maxWidth: .infinity)
        .background(pin.esHerramienta ? Paleta.v2.tarjetaEncima : Color.black.opacity(0.35))
        .contentShape(Rectangle())
        .onTapGesture { gestor.tocar(pin) }
        .gesture(arrastrar)
    }

    /// La esquina de abajo a la derecha: cambia el tamano de la herramienta.
    private var asaEsquina: some View {
        Image(systemName: "arrow.up.left.and.arrow.down.right")
            .font(.caption2.weight(.bold))
            .foregroundStyle(Paleta.v2.gris)
            .frame(width: 28, height: 28)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 2, coordinateSpace: .global)
                    .updating($esquina) { v, estado, _ in estado = v.translation }
                    .onEnded { v in
                        let nuevo = CGSize(width: max(220, pin.tamano.width + v.translation.width),
                                           height: max(180, pin.tamano.height + v.translation.height))
                        let delta = CGSize(width: nuevo.width - pin.tamano.width, height: nuevo.height - pin.tamano.height)
                        pin.tamanoBase = CGSize(width: nuevo.width / pin.escala, height: nuevo.height / pin.escala)
                        // Crece hacia abajo a la derecha: el centro se mueve la mitad.
                        pin.centro = CGPoint(x: pin.centro.x + delta.width / 2, y: pin.centro.y + delta.height / 2)
                        gestor.cambio(pin)
                    }
            )
    }

    // MARK: Gestos

    private var arrastrar: some Gesture {
        DragGesture(minimumDistance: 4, coordinateSpace: .global)
            .updating($arrastre) { v, estado, _ in estado = v.translation }
            .onChanged { v in
                if gestor.elegido != pin.id { gestor.elegir(pin) }
                gestor.guiar(pin, centro: CGPoint(x: pin.centro.x + v.translation.width,
                                                  y: pin.centro.y + v.translation.height))
            }
            .onEnded { v in
                let suelto = CGPoint(x: pin.centro.x + v.translation.width, y: pin.centro.y + v.translation.height)
                let pegado = gestor.iman(pin, centro: suelto)
                if pegado != suelto { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
                pin.centro = pegado
                gestor.cambio(pin)
            }
    }

    private var pellizcar: some Gesture {
        MagnifyGesture()
            .updating($pellizco) { v, estado, _ in estado = v.magnification }
            .onEnded { v in
                if pin.esHerramienta {
                    let base = pin.tamanoBase
                    pin.tamanoBase = CGSize(width: max(220, base.width * v.magnification),
                                            height: max(180, base.height * v.magnification))
                } else {
                    pin.escala = ReglasPin.escalaLimitada(pin.escala * v.magnification, base: pin.tamanoBase)
                }
                gestor.cambio(pin)
            }
    }

    private var girar: some Gesture {
        RotateGesture()
            .updating($giroGesto) { v, estado, _ in estado = v.rotation }
            .onEnded { v in
                let cuartos = ReglasCapa.cuartosDeGiro(grados: v.rotation.degrees)
                if cuartos != 0 {
                    pin.girar(cuartos)
                    gestor.cambio(pin)
                }
            }
    }

    /// Doble toque: a su tamano de nacer (o al doble, si ya estaba).
    private func tamanoReal() {
        withAnimation(.snappy) {
            pin.escala = abs(pin.escala - 1) < 0.01 ? ReglasPin.escalaLimitada(2, base: pin.tamanoBase) : 1
        }
        gestor.cambio(pin)
    }
}

/// La barra comun del pin (`pin-barra.png` de Windows): oscura y plana.
struct BarraPinIPad: View {
    @ObservedObject var pin: PinIPad
    @ObservedObject var gestor: GestorPinesIPad

    var body: some View {
        HStack(spacing: 4) {
            if let paginas = pin.paginas {
                boton("chevron.left", "Página anterior") { gestor.pasarPagina(pin, -1) }
                    .disabled(!paginas.hayAnterior)
                Text(paginas.rotulo)
                    .font(.callout.monospacedDigit())
                    .frame(minWidth: 56)
                boton("chevron.right", "Página siguiente") { gestor.pasarPagina(pin, 1) }
                    .disabled(!paginas.haySiguiente)
                separador
            }
            if pin.esImagen {
                boton("minus", "Más pequeño") { zoom(1 / 1.25) }
                Text("\(Int((pin.escala * 100).rounded())) %")
                    .font(.callout.monospacedDigit())
                    .frame(minWidth: 52)
                boton("plus", "Más grande") { zoom(1.25) }
                separador
                boton("doc.on.doc", "Copiar") { gestor.copiar(pin) }
                boton("pencil", "Anotar") { gestor.anotar(pin) }
                boton("text.viewfinder", "Sacar texto") { gestor.sacarTextoDe(pin) }
                boton("bubble.left", "Al chat") { gestor.alChat(pin) }
            } else if case let .herramienta(h) = pin.contenido {
                boton("doc.on.doc", "Copiar") { gestor.copiar(pin) }
                if case .nota = h.clase {
                    boton("bubble.left", "Guardar en «Mensajes guardados»") { gestor.alChat(pin) }
                }
            }
            mas
            asaArrastrarFuera
            separador
            boton("arrow.down.right.and.arrow.up.left", "Minimizar") { withAnimation(.snappy) { gestor.minimizar(pin) } }
            boton("xmark", "Cerrar") { withAnimation(.snappy) { gestor.cerrar(pin) } }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .foregroundStyle(Paleta.v2.texto)
        .background(Paleta.v2.fondo.opacity(0.96), in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Paleta.v2.linea))
        .shadow(color: .black.opacity(0.35), radius: 10, y: 2)
        .environment(\.colorScheme, .dark)
    }

    private var separador: some View {
        Rectangle().fill(Paleta.v2.linea).frame(width: 1, height: 24).padding(.horizontal, 4)
    }

    private func boton(_ icono: String, _ ayuda: String, _ accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            Image(systemName: icono)
                .font(.system(size: 17, weight: .medium))
                .frame(width: 40, height: 36)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(ayuda)
        .accessibilityLabel(ayuda)
    }

    /// La mano: mantener y arrastrar lleva el pin a otra app.
    private var asaArrastrarFuera: some View {
        Image(systemName: "hand.draw")
            .font(.system(size: 17, weight: .medium))
            .frame(width: 40, height: 36)
            .contentShape(Rectangle())
            .onDrag { gestor.proveedorArrastre(pin) }
            .help("Arrastrar a otra app")
            .accessibilityLabel("Arrastrar a otra app")
    }

    /// «…»: lo que no cabe en la barra.
    private var mas: some View {
        Menu {
            if pin.paginas != nil {
                Section("Páginas") {
                    Button { gestor.pinearPagina(de: pin) } label: {
                        Label("Pinear esta página", systemImage: "doc.on.doc")
                    }
                    Button { gestor.pinearTodasLasPaginas(de: pin) } label: {
                        Label("Pinear todas las páginas", systemImage: "square.stack")
                    }
                }
            }
            if pin.esImagen {
                Button { gestor.guardarEnFotos(pin) } label: { Label("Guardar en Fotos", systemImage: "square.and.arrow.down") }
                Section("Girar") {
                    Button { cambiar { pin.girar(1) } } label: { Label("Girar a la derecha", systemImage: "rotate.right") }
                    Button { cambiar { pin.girar(-1) } } label: { Label("Girar a la izquierda", systemImage: "rotate.left") }
                    Button { cambiar { pin.voltear(horizontal: true) } } label: {
                        Label("Voltear en horizontal", systemImage: "arrow.left.and.right.righttriangle.left.righttriangle.right")
                    }
                    Button { cambiar { pin.voltear(horizontal: false) } } label: {
                        Label("Voltear en vertical", systemImage: "arrow.up.and.down.righttriangle.up.righttriangle.down")
                    }
                }
                Menu {
                    Button { cambiar { pin.ponerFiltros(conGris()) } } label: {
                        Label("Gris", systemImage: pin.filtros.gris ? "checkmark" : "circle.lefthalf.filled")
                    }
                    Button { cambiar { pin.ponerFiltros(conInvertido()) } } label: {
                        Label("Invertir", systemImage: pin.filtros.invertido ? "checkmark" : "circle.righthalf.filled")
                    }
                    Button { cambiar { pin.ponerFiltros(pin.filtros.conBrillo(1)) } } label: {
                        Label("Más brillo", systemImage: "sun.max")
                    }
                    Button { cambiar { pin.ponerFiltros(pin.filtros.conBrillo(-1)) } } label: {
                        Label("Menos brillo", systemImage: "sun.min")
                    }
                    Button { cambiar { pin.restaurarVista() } } label: { Label("Restaurar", systemImage: "arrow.uturn.backward") }
                } label: {
                    Label("Filtros", systemImage: "camera.filters")
                }
                Button { cambiar { pin.escala = 1 } } label: { Label("Tamaño real", systemImage: "1.magnifyingglass") }
            }
            Menu {
                ForEach(ReglasPin.opacidades, id: \.self) { p in
                    Button { cambiar { pin.opacidad = Double(p) / 100 } } label: {
                        if Int((pin.opacidad * 100).rounded()) == p {
                            Label("\(p) %", systemImage: "checkmark")
                        } else {
                            Text("\(p) %")
                        }
                    }
                }
            } label: {
                Label("Opacidad", systemImage: "circle.lefthalf.striped.horizontal")
            }
            Menu {
                ForEach(0 ..< ColoresGrupo.nombres.count, id: \.self) { i in
                    Button { cambiar { pin.grupo = i } } label: {
                        Label(ColoresGrupo.nombre(i), systemImage: pin.grupo == i ? "checkmark.circle.fill" : "circle.fill")
                    }
                }
                Button { cambiar { pin.grupo = nil } } label: { Label("Sin grupo", systemImage: "circle.slash") }
            } label: {
                Label("Grupo", systemImage: "circle.grid.2x2")
            }
        } label: {
            Image(systemName: "ellipsis")
                .font(.system(size: 17, weight: .medium))
                .frame(width: 40, height: 36)
                .contentShape(Rectangle())
        }
        .help("Más")
    }

    private func conGris() -> FiltrosPin {
        var f = pin.filtros
        f.gris.toggle()
        return f
    }

    private func conInvertido() -> FiltrosPin {
        var f = pin.filtros
        f.invertido.toggle()
        return f
    }

    private func cambiar(_ hacer: () -> Void) {
        hacer()
        gestor.cambio(pin)
    }

    private func zoom(_ factor: CGFloat) {
        withAnimation(.snappy) {
            pin.escala = ReglasPin.escalaLimitada(pin.escala * factor, base: pin.tamanoBase)
        }
        gestor.cambio(pin)
    }
}
