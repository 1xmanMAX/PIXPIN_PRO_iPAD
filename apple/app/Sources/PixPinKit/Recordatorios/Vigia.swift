// **El vigia de los recordatorios** (tanda A4): lo que en Windows es el
// hilo de `recordatorios.rs` (`Vigia`, `vigilar`, `releer`) y el
// `atender_recordatorios` del bucle principal.
//
// - **Que hay puesto** lo dice el nucleo (`Nucleo.recordatorios`): los
//   mensajes con `recuerdaEn` y las tareas pendientes con `⏰` de todos los
//   chats. No hay otro almacen: se relee al arrancar, tras sincronizar
//   (lo puesto en el movil suena aqui) y cuando cambia algo en este Mac
//   (`pixpinCambioLocal`, `pixpinTareasCambiadas`: el menu del mensaje, la
//   ventana de Tareas).
// - **Un solo temporizador**, al proximo, con la hora de la pared
//   (`wallDeadline`: el Mac dormido no lo retrasa). Al despertar o si
//   cambia la hora del sistema se revisa lo vencido y se vuelve a armar.
// - **Con la app cerrada**: cada recordatorio lleva ademas su notificacion
//   del sistema a la misma hora, con sus botones (`CentroAvisos`). Al
//   arrancar suena lo que vencio con la app cerrada, como en Windows
//   («llegar tarde es mejor que no llegar»), salvo lo que el sistema ya
//   enseno o se atendio desde su notificacion.
// - **Al sonar**: su aviso en la ventanita de la app (el pin de Windows)
//   con «Hecha» (si es una tarea), «+10 min» y «Abrir»; y se le quita la
//   hora en el cuaderno (`olvidarRecordatorio`), o sonaria otra vez en el
//   proximo arranque (Android hace lo mismo).
//
// Rendimiento: nada sondea. Leer todos los chats va fuera del hilo
// principal, y el nucleo salta sin leer como JSON las lineas que no
// pueden llevar hora.
//
// En el iPad (del Mac): con la app en segundo plano iOS no deja correr el
// temporizador, asi que lo que suena entonces es la notificacion del
// sistema (con sus botones); al volver a primer plano se revisa lo vencido.
// Las notas de voz con hora y las clases del horario suenan como una
// llamada (`LlamadaIPad.swift`): con la app delante, su pantalla de
// llamada; cerrada, su notificacion con sonido de llamada.

import Foundation
import PixPinCore
import UIKit

/// La hora del reloj de la pared de un instante, en ms (la de las tareas,
/// `⏰ AAAA-MM-DD HH:MM`): UTC mas el huso **de ese instante**.
func milisegundosLocales(_ fecha: Date) -> Int64 {
    milisegundos(fecha) + desfaseLocalMs(fecha)
}

/// El instante de una hora del reloj de la pared en ms. El huso se mira en
/// ese dia (un recordatorio para despues del cambio de hora no se corre
/// una hora).
func fechaDeLocales(_ local: Int64) -> Date {
    let aprox = fechaDe(ms: local - desfaseLocalMs())
    return fechaDe(ms: local - desfaseLocalMs(aprox))
}

/// Un recordatorio puesto, ya con su instante.
struct RecordatorioPuesto: Equatable, Sendable {
    let chat: String
    let nombreChat: String
    /// El id de agenda (el del mensaje, o `tarea:<lista>:<hora local>`).
    let id: String
    let cuando: Date
    let texto: String
    let tarea: TareaRecordada?
    /// Si suena como una llamada (una nota de voz con hora, una clase del
    /// horario): su notificacion lleva el timbre.
    var llamada = false

    /// Unico entre todos los chats: el id de un mensaje solo es unico en el
    /// suyo. Es tambien el id de su notificacion.
    var clave: String { AvisosRecordatorio.identificador(chat: chat, id: id) }

    init(chat: String, nombreChat: String = "", id: String, cuando: Date, texto: String,
         tarea: TareaRecordada? = nil) {
        self.chat = chat
        self.nombreChat = nombreChat
        self.id = id
        self.cuando = cuando
        self.texto = texto
        self.tarea = tarea
    }

    /// El del nucleo. La hora de una tarea es local: se pasa a su instante
    /// con el huso de su dia, no con el de hoy.
    init(_ r: RecordatorioMac, desfaseHoy: Int64) {
        let cuando = r.tarea == nil
            ? fechaDe(ms: r.cuandoUtcMs)
            : fechaDeLocales(r.cuandoUtcMs + desfaseHoy)
        self.init(chat: r.chat, nombreChat: r.nombreChat, id: r.id, cuando: cuando,
                  texto: r.texto, tarea: r.tarea)
    }
}

/// **Lo que hay puesto, ordenado por hora** (`Agenda` de Windows): una
/// lista y no un monton porque hacen falta las dos cosas que un monton no
/// da, ensenarla entera y cancelar por clave.
struct AgendaRecordatorios: Equatable {
    /// Siempre ordenada: el primero es el que vence antes.
    private(set) var pendientes: [RecordatorioPuesto] = []

    init(_ v: [RecordatorioPuesto] = []) {
        for r in v { programar(r) }
    }

    /// Pone uno, o **cambia** el que ya hubiera con esa clave: corregir la
    /// hora de algo no lo duplica.
    mutating func programar(_ r: RecordatorioPuesto) {
        pendientes.removeAll { $0.clave == r.clave }
        let sitio = pendientes.firstIndex { ($0.cuando, $0.clave) > (r.cuando, r.clave) } ?? pendientes.count
        pendientes.insert(r, at: sitio)
    }

    /// Lo quita. Devuelve si habia algo que quitar.
    @discardableResult
    mutating func cancelar(_ clave: String) -> Bool {
        let antes = pendientes.count
        pendientes.removeAll { $0.clave == clave }
        return pendientes.count != antes
    }

    /// El primero que vence.
    var proximo: RecordatorioPuesto? { pendientes.first }

    /// Saca **todo** lo que ya vencio a `ahora`, en orden: dos para el mismo
    /// instante suenan los dos, y lo que vencio con la app cerrada sale al
    /// arrancar en vez de quedarse ahi.
    mutating func vencidos(a ahora: Date) -> [RecordatorioPuesto] {
        let cuantos = pendientes.firstIndex { $0.cuando > ahora } ?? pendientes.count
        let salen = Array(pendientes.prefix(cuantos))
        pendientes.removeFirst(cuantos)
        return salen
    }
}

@MainActor
public final class Recordatorios {
    public static let compartido = Recordatorios()

    /// Cuantas notificaciones del sistema se dejan programadas como mucho
    /// (el sistema tiene un tope; las mini-apps tambien usan).
    nonisolated static let topeDelSistema = 40
    /// Lo que se espera tras un cambio local antes de releer (juntar).
    nonisolated static let calma: Duration = .milliseconds(600)

    private var nucleo: Nucleo?
    private(set) var agenda = AgendaRecordatorios()
    /// Las notificaciones del sistema que este vigia dejo puestas.
    private var enElSistema: Set<String> = []
    private var temporizador: DispatchSourceTimer?
    private var lectura: Task<Void, Never>?
    private var releerLuego: Task<Void, Never>?
    private var vigias: [NSObjectProtocol] = []
    /// Cuando empezo a vigilar: lo vencido antes sono con la app cerrada.
    private var arranque = Date()
    /// Lo que ya sono o se atendio desde su notificacion, por clave y
    /// hora ([`marca`]): no vuelve a salir si se lee antes de que el
    /// cuaderno pierda la hora. Otra hora en el mismo mensaje si sale.
    private var hechos: Set<String> = []
    /// Los avisos del horario pospuestos («Volver a llamar en»), hasta que
    /// suenan (en memoria, como `POSPUESTOS` de Windows).
    private var pospuestos: [RecordatorioPuesto] = []
    /// La huella del horario y el dia: si cambia, se relee la agenda.
    private var firmaHorario: String = ""
    private var vigiaHorario: Timer?
    private var vigiaCalendario: Timer?

    /// El chat de mentira de los avisos del horario (no son de ningun chat).
    nonisolated static let chatHorario = ""

    /// La clave de un recordatorio con su hora.
    nonisolated static func marca(_ clave: String, _ cuandoMs: Int64) -> String { "\(clave)@\(cuandoMs)" }

    /// Empieza a vigilar. Una vez, al arrancar la app.
    public func vigilar(nucleo: Nucleo) {
        guard self.nucleo == nil else { return }
        self.nucleo = nucleo
        arranque = Date()
        CentroAvisos.compartido.preparar()
        // El timbre de las llamadas, donde lo busca el sistema.
        Task.detached(priority: .utility) { TonoLlamada.preparar() }
        CentroAvisos.compartido.alResponder([.recordatorio, .recordatorioTarea, .llamada]) { accion, datos in
            Recordatorios.compartido.responder(accion, datos)
        }
        let local = NotificationCenter.default
        for nombre in [Notification.Name.pixpinCambioLocal, .pixpinTareasCambiadas] {
            vigias.append(local.addObserver(forName: nombre, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated { Recordatorios.compartido.releerConCalma() }
            })
        }
        vigias.append(local.addObserver(forName: .NSSystemClockDidChange, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { Recordatorios.compartido.armar() }
        })
        // Al volver a primer plano (iOS para el temporizador en el fondo) y
        // al cambiar el dia o el huso: se revisa lo vencido.
        for nombre in [UIApplication.willEnterForegroundNotification, UIApplication.significantTimeChangeNotification] {
            vigias.append(local.addObserver(forName: nombre, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated { Recordatorios.compartido.releer() }
            })
        }
        // El horario se edita como una nota: se mira cada pocos segundos si
        // cambio (o cambio el dia) y entonces se relee (`vigilar` de Windows).
        firmaHorario = Recordatorios.firma(nucleo)
        let t = Timer(timeInterval: 5, repeats: true) { _ in
            MainActor.assumeIsolated {
                let r = Recordatorios.compartido
                guard let n = r.nucleo else { return }
                let f = Recordatorios.firma(n)
                if f != r.firmaHorario {
                    r.firmaHorario = f
                    r.releer()
                }
            }
        }
        RunLoop.main.add(t, forMode: .common)
        vigiaHorario = t
        // Google Calendar (Windows, 18-oct): se lee cada 15 min, en su hilo;
        // si trajo algo nuevo, los avisos se releen.
        let c = Timer(timeInterval: 15 * 60, repeats: true) { _ in
            MainActor.assumeIsolated { Recordatorios.compartido.leerCalendario() }
        }
        RunLoop.main.add(c, forMode: .common)
        vigiaCalendario = c
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) { Recordatorios.compartido.leerCalendario() }
        releer()
    }

    /// Lee Google Calendar ya (al arrancar, cada 15 min y al conectarlo).
    public func leerCalendario() {
        guard let nucleo else { return }
        TransporteHttps.instalar()
        _ = LlaveroDelSistema.instalar()
        Task {
            let cambio = await Task.detached(priority: .utility) { (try? nucleo.calendarioLeer()) ?? false }.value
            if cambio { self.releer() }
        }
    }

    /// La fecha y el tamano de `horario.md` y el dia de hoy.
    nonisolated static func firma(_ nucleo: Nucleo) -> String {
        let ruta = nucleo.rutaDelHorario()
        let a = (try? FileManager.default.attributesOfItem(atPath: ruta)) ?? [:]
        let fecha = (a[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0
        let tam = (a[.size] as? Int) ?? -1
        return "\(fecha)/\(tam)/\(Calendar.current.startOfDay(for: Date()).timeIntervalSince1970)"
    }

    /// «Volver a llamar en» de un aviso del horario: el mismo, mas tarde.
    func posponerHorario(id: String, nombre: String, a cuando: Date) {
        let base = id.split(separator: "|", maxSplits: 1).first.map(String.init) ?? id
        let nuevo = "\(base)|\(milisegundos(cuando))"
        pospuestos.removeAll { $0.id == id }
        var r = RecordatorioPuesto(chat: Self.chatHorario, id: nuevo, cuando: cuando, texto: nombre)
        r.llamada = true
        pospuestos.append(r)
        releer()
    }

    /// **Vuelve a leer** lo puesto en todos los chats y deja eso (al
    /// arrancar y tras sincronizar: el `releer` de Windows).
    public func releer() {
        guard let nucleo else { return }
        let desfase = desfaseLocalMs()
        lectura?.cancel()
        lectura = Task { [weak self] in
            let (leidos, horario, llamadas) = await Task.detached(priority: .utility) {
                () -> ([RecordatorioMac], [AvisoHorario], Set<String>) in
                let leidos = (try? nucleo.recordatorios(desfaseLocalMs: desfase)) ?? []
                // Las notas de voz (las llamadas secretas): su notificacion
                // lleva el timbre. Solo las que aun no vencieron.
                let ahora = milisegundos()
                var llamadas = Set<String>()
                for r in leidos where r.tarea == nil && r.cuandoUtcMs > ahora {
                    if (try? nucleo.llamadaSecreta(chat: r.chat, mensaje: r.id)) != nil {
                        llamadas.insert(r.chat + "\u{0}" + r.id)
                    }
                }
                return (leidos, nucleo.avisosDelHorario(desfaseMs: desfase), llamadas)
            }.value
            guard !Task.isCancelled, let self else { return }
            let delHorario = horario.map {
                var r = RecordatorioPuesto(chat: Self.chatHorario, nombreChat: "Horario", id: $0.id,
                                           cuando: fechaDe(ms: $0.cuandoUtcMs), texto: $0.nombre)
                r.llamada = true
                return r
            }
            let deChats = leidos.map { l -> RecordatorioPuesto in
                var r = RecordatorioPuesto(l, desfaseHoy: desfase)
                r.llamada = llamadas.contains(l.chat + "\u{0}" + l.id)
                return r
            }
            self.aplicar(deChats + delHorario + self.pospuestos)
        }
    }

    /// Releer tras un rato sin cambios (varios cambios seguidos, una vez).
    func releerConCalma() {
        releerLuego?.cancel()
        releerLuego = Task { [weak self] in
            try? await Task.sleep(for: Recordatorios.calma)
            guard !Task.isCancelled else { return }
            self?.releer()
        }
    }

    /// Deja como agenda lo leido: quita del sistema lo que ya no esta, pone
    /// lo nuevo y arma el temporizador.
    func aplicar(_ leidos: [RecordatorioPuesto]) {
        let nueva = AgendaRecordatorios(leidos.filter {
            !hechos.contains(Self.marca($0.clave, milisegundos($0.cuando)))
        })
        let futuros = nueva.pendientes.filter { $0.cuando > Date() }.prefix(Self.topeDelSistema)
        let quedan = Set(futuros.map(\.clave))
        CentroAvisos.compartido.quitar(Array(enElSistema.subtracting(quedan)))
        let antes = Dictionary(agenda.pendientes.map { ($0.clave, $0) }, uniquingKeysWith: { a, _ in a })
        for r in futuros where antes[r.clave] != r || !enElSistema.contains(r.clave) {
            CentroAvisos.compartido.poner(AvisosRecordatorio.aviso(de: r))
        }
        enElSistema = quedan
        agenda = nueva
        armar()
    }

    /// Suena lo vencido y deja el temporizador al proximo.
    func armar() {
        temporizador?.cancel()
        temporizador = nil
        let ahora = Date()
        for r in agenda.vencidos(a: ahora) {
            vencer(r)
        }
        guard let proximo = agenda.proximo else { return }
        let fuente = DispatchSource.makeTimerSource(queue: .main)
        fuente.schedule(wallDeadline: .now() + max(0, proximo.cuando.timeIntervalSince(ahora)),
                        leeway: .milliseconds(250))
        fuente.setEventHandler {
            MainActor.assumeIsolated { Recordatorios.compartido.armar() }
        }
        fuente.resume()
        temporizador = fuente
    }

    /// **Suena uno**: su aviso en la app y fuera la hora del cuaderno.
    private func vencer(_ r: RecordatorioPuesto) {
        enElSistema.remove(r.clave)
        if r.id.hasPrefix("horario:") {
            pospuestos.removeAll { $0.id == r.id }
            guard hechos.insert(Self.marca(r.clave, milisegundos(r.cuando))).inserted else { return }
            sonarHorario(r)
            return
        }
        olvidar(r.chat, r.id, tarea: r.tarea != nil)
        guard hechos.insert(Self.marca(r.clave, milisegundos(r.cuando))).inserted else { return }
        // **Una nota de voz con hora es una llamada secreta**: suena como
        // una llamada. Lo que vencio hace rato con la app cerrada sale como
        // el aviso de siempre.
        if r.tarea == nil, let nucleo, Llamada.aunSuena(r.cuando, ahora: Date()) {
            Task {
                let l = await Task.detached(priority: .userInitiated) {
                    try? nucleo.llamadaSecreta(chat: r.chat, mensaje: r.id)
                }.value
                if let l {
                    Llamada.sonar(l, nucleo: nucleo)
                } else {
                    self.avisar(r)
                }
            }
            return
        }
        avisar(r)
    }

    /// **Suena un aviso del horario**: la pantalla de la llamada con la hora
    /// y el aula. Si el horario cambio y ya no se encuentra, sale igual con
    /// lo que se apunto al ponerlo. Lo que vencio hace rato (la app
    /// cerrada) es un aviso de siempre.
    private func sonarHorario(_ r: RecordatorioPuesto) {
        guard let nucleo else {
            avisar(r)
            return
        }
        if Llamada.aunSuena(r.cuando, ahora: Date()) {
            sonarClase(id: r.id, nombre: r.texto, nucleo: nucleo, contestada: false)
            return
        }
        let desfase = desfaseLocalMs()
        Task {
            let a = await Task.detached(priority: .userInitiated) {
                nucleo.avisoDelHorario(id: r.id, desfaseMs: desfase)
            }.value
            var aviso = AvisosRecordatorio.aviso(de: r)
            aviso.categoria = .mini
            aviso.titulo = a?.nombre ?? r.texto
            aviso.texto = [a?.detalle, a?.texto].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: "\n")
            if aviso.texto.isEmpty { aviso.texto = "Horario" }
            CentroAvisos.compartido.mostrarEnLaApp(aviso, sonar: true)
        }
    }

    /// **La llamada de una clase**: lee otra vez su aviso del horario (la
    /// hora, el aula, su nota de voz y lo apuntado) y la hace sonar.
    func sonarClase(id: String, nombre: String, nucleo: Nucleo, contestada: Bool) {
        let desfase = desfaseLocalMs()
        Task {
            let a = await Task.detached(priority: .userInitiated) {
                nucleo.avisoDelHorario(id: id, desfaseMs: desfase)
            }.value
            let l = LlamadaSecreta(audio: a?.audio ?? "", nombre: a?.nombre ?? nombre,
                                   chat: Self.chatHorario, mensaje: id)
            Llamada.sonar(l, nucleo: nucleo, detalle: a?.detalle ?? "", texto: a?.texto, contestada: contestada)
        }
    }

    /// Lo que hacen los botones de una llamada (su notificacion del
    /// sistema o la «Llamada perdida» de la ventanita): tocarla la abre ya
    /// contestada; «+10 min» vuelve a llamar.
    private func responderLlamada(_ accion: AccionAviso, chat: String, id: String, nombre: String, nucleo: Nucleo) {
        let horario = chat == Self.chatHorario
        switch accion {
        case .abrir:
            if horario {
                sonarClase(id: id, nombre: nombre, nucleo: nucleo, contestada: true)
            } else {
                olvidar(chat, id, tarea: false)
                Task {
                    let l = await Task.detached(priority: .userInitiated) {
                        try? nucleo.llamadaSecreta(chat: chat, mensaje: id)
                    }.value
                    if let l {
                        Llamada.sonar(l, nucleo: nucleo, contestada: true)
                    } else {
                        Recordatorios.abrir(chat: chat, mensaje: id, nucleo: nucleo)
                    }
                }
            }
        case .posponer:
            let cuando = Date().addingTimeInterval(TimeInterval(CentroAvisos.posponerMinutos * 60))
            if horario {
                posponerHorario(id: id, nombre: nombre, a: cuando)
            } else {
                Task.detached(priority: .userInitiated) {
                    let hecho = (try? nucleo.volverARecordar(chat: chat, mensaje: id,
                                                             cuandoUtcMs: milisegundos(cuando))) == true
                    await MainActor.run {
                        if hecho { Recordatorios.avisarCambio(chat: chat, tarea: false) }
                        Recordatorios.compartido.releer()
                    }
                }
            }
        case .hecha, .conservar, .galeria, .cerrar:
            if !horario { olvidar(chat, id, tarea: false) }
        }
    }

    /// El aviso de un recordatorio que sono, en la ventanita de la app.
    private func avisar(_ r: RecordatorioPuesto) {
        let aviso = AvisosRecordatorio.aviso(de: r)
        // Lo que vencio con la app cerrada y el sistema ya enseno no sale
        // otra vez: sigue en el centro de notificaciones con sus botones.
        if r.cuando < arranque.addingTimeInterval(-30), CentroAvisos.compartido.permiso == .concedido,
           let centro = CentroAvisos.centro
        {
            centro.getDeliveredNotifications { entregadas in
                let ya = entregadas.contains { $0.request.identifier == aviso.id }
                Task { @MainActor in
                    if !ya { CentroAvisos.compartido.mostrarEnLaApp(aviso, sonar: true) }
                }
            }
            return
        }
        CentroAvisos.compartido.mostrarEnLaApp(aviso, sonar: true)
    }

    /// Le quita la hora en el cuaderno (fuera del hilo principal) y lo dice
    /// a la sincronizacion: una tarea, al instante.
    private func olvidar(_ chat: String, _ id: String, tarea: Bool) {
        guard let nucleo else { return }
        Task.detached(priority: .utility) {
            guard (try? nucleo.olvidarRecordatorio(chat: chat, id: id)) == true else { return }
            await MainActor.run {
                Recordatorios.avisarCambio(chat: chat, tarea: tarea)
            }
        }
    }

    /// Lo que se cambio en un cuaderno, a quien escucha (la sincronizacion,
    /// el chat, la ventana de Tareas).
    static func avisarCambio(chat: String, tarea: Bool) {
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        if tarea {
            NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: chat)
        }
    }

    // MARK: Los botones

    /// Lo que hace cada boton del aviso, venga de la ventanita o de la
    /// notificacion del sistema (`atender_recordatorios` y los pedidos
    /// `posponer`, `marcar_tarea` y `abrir` de Windows).
    func responder(_ accion: AccionAviso, _ datos: [String: String]) {
        guard let chat = datos[AvisosRecordatorio.claveChat], let id = datos[AvisosRecordatorio.claveId] else { return }
        let tarea = AvisosRecordatorio.tarea(de: datos)
        let clave = AvisosRecordatorio.identificador(chat: chat, id: id)
        CentroAvisos.compartido.quitarDeLaApp(clave)
        // Atendido: no vuelve a salir aunque aun no se haya leido sin hora
        // (la app se abrio pulsando la notificacion).
        if let ms = datos[AvisosRecordatorio.claveCuando].flatMap(Int64.init) {
            hechos.insert(Self.marca(clave, ms))
        }
        agenda.cancelar(clave)
        guard let nucleo else { return }
        if datos[Llamada.claveNombre] != nil || chat == Self.chatHorario {
            responderLlamada(accion, chat: chat, id: id, nombre: datos[Llamada.claveNombre] ?? "", nucleo: nucleo)
            return
        }
        switch accion {
        case .posponer:
            let cuando = Date().addingTimeInterval(TimeInterval(CentroAvisos.posponerMinutos * 60))
            Task.detached(priority: .userInitiated) {
                let hecho: Bool
                if let tarea {
                    hecho = (try? nucleo.ponerHoraTarea(chat: chat, lista: tarea.lista, indice: tarea.indice,
                                                        crudo: nil, cuandoLocalMs: milisegundosLocales(cuando))) == true
                } else {
                    hecho = (try? nucleo.volverARecordar(chat: chat, mensaje: id,
                                                         cuandoUtcMs: milisegundos(cuando))) == true
                }
                await MainActor.run {
                    if hecho { Recordatorios.avisarCambio(chat: chat, tarea: tarea != nil) }
                    Recordatorios.compartido.releer()
                }
            }
        case .hecha:
            olvidar(chat, id, tarea: tarea != nil)
            guard let tarea else { return }
            Task.detached(priority: .userInitiated) {
                let hecho = (try? nucleo.tacharTareaRecordada(chat: chat, lista: tarea.lista, indice: tarea.indice)) == true
                if hecho {
                    await MainActor.run { Recordatorios.avisarCambio(chat: chat, tarea: true) }
                }
            }
        case .abrir:
            olvidar(chat, id, tarea: tarea != nil)
            Recordatorios.abrir(chat: chat, mensaje: tarea?.lista ?? id, nucleo: nucleo)
        case .conservar, .galeria, .cerrar:
            olvidar(chat, id, tarea: tarea != nil)
        }
    }

    /// **«Abrir»**: el chat en Proyectos, desplazado hasta el mensaje (o la
    /// lista de la tarea) y marcado, como «Abrir en Proyectos» del buscador.
    static func abrir(chat: String, mensaje: String, nucleo _: Nucleo) {
        guard chat != chatHorario else { return }
        Navegacion.compartida.irAlChat(chat, mensaje: mensaje)
    }
}
