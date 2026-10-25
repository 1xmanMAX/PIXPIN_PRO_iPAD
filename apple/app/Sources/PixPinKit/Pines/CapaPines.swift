// La capa de pines flotantes, encima de toda la app (y de los visores). iOS
// no deja dibujar encima de otras apps, asi que lo que en Windows flota
// encima de todo flota aqui dentro de PixPin.
//
// La raiz la pone dos veces: encima de las secciones y encima del visor a
// pantalla completa (que tapa lo de debajo). Las dos pintan el mismo estado
// (`GestorPinesIPad.compartido`), y para no pintar dos veces el mismo pin la
// de debajo se esconde mientras hay un visor abierto; la del visor se sabe
// presentada (`isPresented`).
//
// **No roba toques fuera de los pines**: la capa no tiene fondo, solo los
// pines, sus barras, las burbujas del borde y el panel reciben toques; lo
// demas pasa a la app de debajo.
//
// Tambien pinta las guias azules de alinear pines, la pila de capturas
// (abajo a la izquierda) y la cuenta atras de «Capturar PixPin en 3 s».
// Al arrancar enchufa los ganchos del chat: «Pinear» un fichero o una lista
// de tareas viva desde el menu de un mensaje.
//
// Firma intacta: `CapaPines(almacen:)` (la usa `Raiz/RaizIPad.swift`).

import PixPinCore
import SwiftUI

/// La capa de pines flotantes, encima de toda la app (y de los visores).
public struct CapaPines: View {
    let almacen: Almacen
    @ObservedObject private var gestor = GestorPinesIPad.compartido
    @ObservedObject private var nav = Navegacion.compartida
    @ObservedObject private var avisos = AvisosMini.compartido
    @ObservedObject private var pila = PilaCapturasIPad.compartida
    @ObservedObject private var capturador = CapturadorIPad.compartido
    @ObservedObject private var grabador = GrabadorIPad.compartido
    /// Dentro del visor (una presentacion a pantalla completa).
    @Environment(\.isPresented) private var presentada

    @MainActor
    public init(almacen: Almacen) {
        self.almacen = almacen
        CapaPines.apuntarArranque()
    }

    /// Si esta capa es la que pinta ahora.
    private var activa: Bool { presentada || nav.visor == nil }

    public var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .topLeading) {
                if activa {
                    ForEach(gestor.pines.filter { !$0.oculto && !$0.minimizado }) { pin in
                        VistaPinIPad(pin: pin, gestor: gestor)
                            .transition(.scale(scale: 0.85).combined(with: .opacity))
                    }
                    ForEach(Array(gestor.guias.enumerated()), id: \.offset) { _, g in
                        let m = ReglasGuias.marco(de: g, grosor: 2)
                        Rectangle()
                            .fill(Paleta.v2.acento.opacity(0.92))
                            .frame(width: m.width, height: m.height)
                            .position(x: m.midX, y: m.midY)
                            .allowsHitTesting(false)
                    }
                    VistaPilaCapturas(pila: pila)
                        .padding(.leading, 16)
                        .padding(.bottom, 16)
                        .frame(width: geo.size.width, height: geo.size.height, alignment: .bottomLeading)
                    if let n = capturador.cuentaAtras {
                        Text("\(n)")
                            .font(.system(size: 96, weight: .bold, design: .rounded).monospacedDigit())
                            .foregroundStyle(.white)
                            .frame(width: 180, height: 180)
                            .background(Color.black.opacity(0.55), in: Circle())
                            .frame(width: geo.size.width, height: geo.size.height)
                            .allowsHitTesting(false)
                            .transition(.opacity)
                    }
                    BurbujasPines(gestor: gestor)
                        .frame(width: geo.size.width, height: geo.size.height, alignment: .trailing)
                    if gestor.panelAbierto {
                        VistaPinesAbiertos(gestor: gestor)
                            .padding(.trailing, 64)
                            .padding(.vertical, 16)
                            .frame(width: geo.size.width, height: geo.size.height, alignment: .trailing)
                            .transition(.move(edge: .trailing).combined(with: .opacity))
                    }
                    BotonPararGrabacion(grabador: grabador)
                        .padding(.top, 12)
                        .frame(width: geo.size.width, alignment: .top)
                    VistaAvisoSonando(avisos: avisos)
                        .frame(width: geo.size.width, alignment: .top)
                }
            }
            .frame(width: geo.size.width, height: geo.size.height, alignment: .topLeading)
            .opacity(gestor.sinCapa ? 0 : 1)
            .animation(.snappy, value: gestor.panelAbierto)
            .animation(.snappy, value: pila.visible)
            .onAppear {
                gestor.arrancar(almacen: almacen)
                if activa { gestor.area = geo.size }
            }
            .onChange(of: geo.size) { _, nuevo in
                if activa { gestor.area = nuevo }
            }
        }
    }

    @MainActor private static var apuntado = false

    /// Lo que se pone en marcha al arrancar la app, una vez: el almacen de
    /// pines (vuelven los que estaban abiertos) y los avisos de las
    /// mini-apps.
    @MainActor
    private static func apuntarArranque() {
        guard !apuntado else { return }
        apuntado = true
        Arranque.apuntar { entorno in
            // «Pinear» desde el menu de un mensaje del chat.
            GanchosChat.pinearFichero = { url, _ in
                if !GestorPinesIPad.compartido.pinear(url: url) { avisarFalloPin() }
            }
            GanchosChat.pinearTareas = { chat, mensaje, documento in
                GestorPinesIPad.compartido.pinearMiniapp(chat: chat, mensaje: mensaje, miniapp: "tareas",
                                                         documento: documento)
            }
            guard let almacen = entorno.almacen else { return }
            GestorPinesIPad.compartido.arrancar(almacen: almacen)
        }
    }
}
