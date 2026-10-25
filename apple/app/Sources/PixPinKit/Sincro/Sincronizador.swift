// Sincronizar este Mac con los demas aparatos del grupo (PC, movil, otro
// Mac), como «Sincronizar» de Windows y su sincronizacion sola
// (`sincronizar/automatica.rs`).
//
// - **Presencia** (`presencia.rs` de Windows): mientras la app vive y el Mac
//   esta en un grupo, escucha (`Puerta`), se anuncia (`Anunciador`) y mira
//   quien del grupo hay en la Wi-Fi (`Vecinos`). Con `[sincro] presencia =
//   false` no se abre puerta ni se anuncia (y no hay automatica): se puede
//   sincronizar a mano, pero nadie encuentra a este Mac.
// - **Cuando** (el `RelojSincro` del nucleo, igual que Windows): a mano; al
//   aparecer un aparato del grupo; 30 s despues del ultimo cambio de aqui;
//   cada 10 min mientras haya alguno; y **las tareas al instante** (600 ms
//   despues de tachar, solo esos chats). Lo automatico se apaga con el modo
//   de bajo consumo.
// - **Como**: a mano con un aparato, **preguntando** («¿Qué sincronizar?»,
//   con los proyectos borrados); «con todos» y lo automatico, sin preguntar
//   y en **dos rondas** con tres aparatos o mas. Cada vuelta es una llamada
//   al nucleo en una `Task.detached` con prioridad `.utility` (§4bis.3).
// - **Sin ruido**: lo automatico solo avisa de un fallo al tercero seguido.
// - **Las personas de los chats compartidos** (iPad, como Windows y el
//   movil; el Mac aun no): con su propio reloj, sondeo cada minuto y una
//   vuelta con cada una que conteste, solo con los chats comunes. Sin grupo
//   pero con chats compartidos tambien se escucha y se anuncia
//   (`anuncioDePresencia`).
//
// - **Donde esta cada uno** (A3, `al_dia.rs` de Windows): lo que da la
//   sonda de un aparato visto en Bonjour se apunta en
//   `sincro/direcciones.txt` (el fichero del movil); los miembros del grupo
//   que no se anuncian se prueban en su direccion recordada al refrescar y
//   al abrir la ventana; y antes de una vuelta a mano se mira primero la
//   direccion de siempre y, si no contesta, el anuncio (`dondeEstaAparato`).
//   La lista sale con **todos los miembros**, tambien los que no estan, con
//   «sincronizado hace X».
//
// Nada sondea (§4bis.6): Bonjour avisa de quien aparece, quien escribe avisa
// con `pixpinCambioLocal` (y `pixpinTareasCambiadas`), el cambio de modo de
// bajo consumo llega por notificacion y entre medias se duerme de una vez lo
// que diga el reloj (`proximoMs`).

import Foundation
import PixPinCore

public extension Notification.Name {
    /// Algo cambio en los chats de este aparato (una nota, un chat nuevo):
    /// la sincronizacion sola cuenta desde aqui sus 30 s de calma.
    static let pixpinCambioLocal = Notification.Name("PixPinCambioLocal")
    /// Cambiaron las tareas de un chat (se tacho una casilla, se apunto una
    /// tarea): salen **al instante** a los aparatos que contestan.
    /// `object` es la ficha del chat (`FichaChat.id`, un `String`).
    static let pixpinTareasCambiadas = Notification.Name("PixPinTareasCambiadas")
    /// Llego de otro aparato una foto para el lienzo que esta delante
    /// (`FrenteMac.lienzo`). `object` es el `ArchivoLlegado` (su `ruta`).
    static let pixpinFotoAlLienzo = Notification.Name("PixPinFotoAlLienzo")
}

@MainActor
public final class Sincronizador: ObservableObject {
    /// Como esta cada aparato del grupo visto en la red.
    public enum Estado: Equatable, Sendable {
        case mirando
        case contesta
        case noContesta
        /// Del grupo, pero ni se anuncia ni contesta en su direccion
        /// recordada: apagado o fuera de esta Wi-Fi.
        case noEncontrado
        case sincronizando
        case hecho(String)
        case fallo(String)
    }

    public struct Fila: Identifiable, Equatable, Sendable {
        public var id: String
        public var nombre: String
        public var letra: String
        public var direccion: Direccion?
        public var estado: Estado
        /// La ultima vez que se sincronizo con el (ms UTC), 0 si nunca.
        public var ultimaVez: Int64 = 0
    }

    /// Que chats entran en una vuelta (`Alcance` de Windows).
    enum Alcance: Sendable {
        /// Lo elegido la ultima vez con ese aparato («con todos», lo automatico).
        case comoSiempre
        /// Se pregunta en «¿Qué sincronizar?».
        case preguntar
        /// Solo los chats de estas fichas: las tareas al instante.
        case tareas([String])
    }

    /// Como acabo una vuelta.
    enum FinVuelta {
        case hecha(ResumenSincro)
        /// El otro (o este) ya estaba en otra vuelta: no es un fallo, la
        /// proxima.
        case ocupado
        case fallo(String)
    }

    @Published public private(set) var aparato: InfoAparato?
    /// El codigo del grupo como se lee (`K7Q2M-9XMPA`), si hay grupo.
    @Published public private(set) var codigoLegible: String?
    @Published public private(set) var filas: [Fila] = []
    /// Una vuelta (o una union) en marcha.
    @Published public private(set) var ocupado = false
    /// Lo que se esta haciendo, en palabras («Buscando…»).
    @Published public private(set) var fase: String?
    /// La frase de la ultima vuelta (`ResumenSincro.texto`).
    @Published public private(set) var ultimoResultado: String?
    @Published public private(set) var error: String?
    @Published public private(set) var bajoConsumo: Bool
    /// El puerto donde escucha este Mac (para escribirlo a mano en otro).
    @Published public private(set) var puerto: UInt16?
    /// `[sincro] presencia` de `pixpinmax.toml`: apagada, ni se escucha ni
    /// se anuncia, y no hay sincronizacion sola.
    @Published public private(set) var presencia = true
    /// **«¿Qué sincronizar?» esperando respuesta**: la ventana de
    /// Sincronizar la ensena y contesta.
    @Published public var pregunta: PreguntaPendiente?
    /// La sincronizacion sola (al aparecer, tras los cambios, cada 10 min y
    /// las tareas al instante).
    @Published public var automatica = true {
        didSet {
            latido()
            latidoPersonas()
        }
    }

    /// Como va «Enviar a…» con cada aparato (por su id).
    @Published public private(set) var envios: [String: EstadoEnvioSuelto] = [:]
    /// Lo ultimo que llego suelto de otro aparato (para ensenarlo).
    @Published public private(set) var ultimasLlegadas: [ArchivoLlegado] = []

    /// Si este aparato esta en algun chat compartido con personas
    /// (`hayChatsCompartidos`): sin grupo, tambien se escucha y se anuncia
    /// (`anuncioDePresencia`), y se sincroniza solo con ellas.
    @Published public private(set) var hayCompartidos = false
    /// **Con quien va la sincronizacion sola** ahora (los que contestan a la
    /// sonda, `RelojSincro.conQuien`), por nombre: los del grupo.
    @Published public private(set) var conQuienAuto: [String] = []
    /// Las personas de los chats compartidos que contestan (por nombre).
    @Published public private(set) var personasQueContestan: [String] = []
    /// Fallos seguidos de la sincronizacion sola (`RelojSincro.fallosSeguidos`).
    @Published public private(set) var fallosAuto: UInt32 = 0
    /// Las direcciones recordadas (`sincro/direcciones.txt`), con el nombre
    /// de quien es si se sabe: se releen al refrescar.
    @Published public private(set) var recordadas: [DireccionConNombre] = []

    /// Se llama cuando una vuelta trajo algo (para releer la lista de chats).
    public var alTraerCambios: (@MainActor () -> Void)?

    public let nucleo: Nucleo
    public let vecinos = Vecinos()
    private let anunciador = Anunciador()
    private var puerta: Puerta?
    private var etiqueta: String?
    /// La decision de cuando sincronizar solo (`automatica.rs` de Windows).
    private let relojAuto = RelojSincro()
    /// Cuantos avisos de cambio van: la huella de los chats para el reloj.
    private var cambios: UInt64 = 0
    /// El unico sueno hasta lo proximo que toque.
    private var espera: Task<Void, Never>?
    /// Una vuelta automatica (o de tareas) en marcha.
    private var corriendo = false
    private var ultimaVuelta: [String: Date] = [:]
    /// Los miembros del grupo segun la identidad (menos este), con su
    /// direccion recordada y la ultima vez (`miembros_del_grupo`).
    private var miembros: [MiembroGrupo] = []
    /// Los que se encontraron en su direccion recordada y no por Bonjour:
    /// esos no se pierden porque Bonjour no los anuncie.
    private var porRecordada: Set<String> = []
    /// Los que se estan probando en su direccion recordada ahora mismo.
    private var probando: Set<String> = []
    private var observadores: [NSObjectProtocol] = []
    /// Si ya se leyo `[sincro] automatica` del TOML (iPad: al arrancar).
    private var automaticaLeida = false
    /// **Las personas de los chats compartidos llevan su propio reloj**
    /// (`automatica.rs` de Windows, `conPersonas` del movil): aparecer, un
    /// cambio aqui o el periodo, por cada una.
    private let relojPersonas = RelojSincro()
    /// Donde contesto cada persona en la ultima sonda.
    private var dondePersonas: [String: Direccion] = [:]
    private var nombresPersonas: [String: String] = [:]
    private var ultimoSondeoPersonas: Int64 = 0
    private var sondeandoPersonas = false
    /// Una vuelta con las personas en marcha.
    private var corriendoPersonas = false
    private var esperaPersonas: Task<Void, Never>?
    private let reloj: () -> Date

    public init(nucleo: Nucleo, reloj: @escaping () -> Date = { Date() }) {
        self.nucleo = nucleo
        self.reloj = reloj
        bajoConsumo = ProcessInfo.processInfo.isLowPowerModeEnabled
    }

    /// Arranca con la app: lee quien es este aparato y, si esta en un grupo,
    /// abre la puerta, se anuncia y busca a los suyos.
    public func arrancar() {
        guard observadores.isEmpty else { return }
        let centro = NotificationCenter.default
        observadores.append(centro.addObserver(forName: .NSProcessInfoPowerStateDidChange, object: nil,
                                               queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.cambioDeEnergia() }
        })
        observadores.append(centro.addObserver(forName: .pixpinCambioLocal, object: nil,
                                               queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.cambioLocal() }
        })
        observadores.append(centro.addObserver(forName: .pixpinTareasCambiadas, object: nil,
                                               queue: .main) { [weak self] aviso in
            let ficha = aviso.object as? String
            MainActor.assumeIsolated { if let ficha { self?.cambiaronTareas(ficha) } }
        })
        vecinos.alCambiar = { [weak self] _ in self?.vecinosCambiaron() }
        relojAuto.verCambios(cambios: cambios, ahora: ahoraMs())
        relojPersonas.verCambios(cambios: cambios, ahora: ahoraMs())
        Task { await refrescar() }
    }

    /// Lo deja todo: deja de anunciarse, de escuchar y de buscar.
    ///
    /// iPad: se para al irse la app al fondo. Lo que el reloj sabia de quien
    /// contesta ya no vale: al volver, cada uno que conteste cuenta como
    /// «aparecio» (la vuelta al volver a la app), salvo que se sincronizara
    /// con el hace nada (`tocaPorAparecer`, como el Bonjour que va y viene);
    /// y los encontrados en su direccion recordada se vuelven a probar (la IP
    /// puede haber cambiado).
    public func parar() {
        espera?.cancel()
        espera = nil
        esperaPersonas?.cancel()
        esperaPersonas = nil
        let ahora = reloj()
        for id in relojAuto.conQuien() where tocaPorAparecer(ultimaVuelta: ultimaVuelta[id], ahora: ahora) {
            relojAuto.olvidar(id: id)
        }
        for id in relojPersonas.conQuien() { relojPersonas.olvidar(id: id) }
        porRecordada = []
        dondePersonas = [:]
        ultimoSondeoPersonas = 0
        conQuienAuto = []
        personasQueContestan = []
        pregunta?.contestar(nil)
        anunciador.retirar()
        puerta?.cerrar()
        puerta = nil
        puerto = nil
        vecinos.parar()
        for o in observadores { NotificationCenter.default.removeObserver(o) }
        observadores = []
    }

    // MARK: - El grupo

    /// Crea un grupo nuevo con este Mac dentro y ensena su codigo.
    public func crearGrupo() async {
        await trabajar("Creando el grupo…") { nucleo in
            _ = try nucleo.crearGrupo()
        }
        await refrescar()
    }

    /// **Se une al grupo** del codigo `codigo`. Sin `direccion`, busca en la
    /// Wi-Fi (25 s, como Windows) un aparato que anuncie la etiqueta de ese
    /// codigo; con ella (`192.168.1.20:47474`), va directo. Despues hace la
    /// primera vuelta con el, que es la que trae los chats.
    public func unirse(codigo: String, direccion: Direccion? = nil) async {
        error = nil
        guard codigoValido(codigo: codigo) else {
            error = "El código de grupo son diez signos (como K7Q2M-9XMPA)."
            return
        }
        guard !ocupado else { return }
        ocupado = true
        defer { ocupado = false; fase = nil; latido() }
        var destino = direccion
        if destino == nil {
            fase = "Buscando un aparato del grupo en esta Wi-Fi…\nEn el otro aparato, ten PixPin abierto."
            let yo = aparato?.id ?? ""
            let suya = await Task.detached(priority: .utility) { etiquetaDeGrupo(codigo: codigo) }.value
            if let v = await vecinos.esperar(tope: 25, { $0.esDelGrupo(etiqueta: suya, yo: yo) }) {
                destino = await Vecinos.sondear(v.punto)
            }
        }
        guard let destino else {
            error = "No apareció ningún aparato con ese código. Comprueba el código, que los dos "
                + "estáis en la misma Wi-Fi y que el otro tiene PixPin abierto. También puedes "
                + "escribir su dirección."
            return
        }
        fase = "Uniéndose al grupo…"
        let nucleo = self.nucleo
        do {
            _ = try await Task.detached(priority: .utility) {
                try nucleo.unirseAGrupo(host: destino.host, puerto: destino.puerto, codigo: codigo)
            }.value
        } catch {
            self.error = textoDeError(error)
            return
        }
        await refrescar()
        fase = "Trayendo los chats…"
        if case let .hecha(r) = await vuelta(con: destino, id: nil, alcance: .comoSiempre) {
            ultimoResultado = textoDe(r)
            error = nil
            if r.trajo { alTraerCambios?() }
        }
    }

    /// **Renombra este aparato** (`Disco.renombrar`): se anuncia ya con el
    /// nombre nuevo, y los demas lo ven en la siguiente vuelta.
    public func renombrar(_ nombre: String) async {
        await trabajar("Cambiando el nombre…") { nucleo in
            let info = try nucleo.renombrarAparato(nombre: nombre)
            // iPad: el sistema solo da «iPad»; el nombre elegido se guarda
            // tambien aqui (`nombreDeEsteAparato()` lo lee al abrir).
            UserDefaults.standard.set(info.nombre, forKey: claveNombreDelAparato)
        }
        await refrescar()
    }

    /// **Sale del grupo** (`Disco.salirDelGrupo`): deja de sincronizarse
    /// con los demas. No se borra nada.
    public func salirDelGrupo() async {
        await trabajar("Saliendo del grupo…") { nucleo in
            _ = try nucleo.salirDelGrupo()
        }
        porRecordada = []
        miembros = []
        filas = []
        await refrescar()
    }

    // MARK: - Sincronizar

    /// «Sincronizar» con un aparato (`id`), **preguntando que** («¿Qué
    /// sincronizar?», como el boton de cada aparato en Windows), o «con
    /// todos» los que contestan (`nil`), sin preguntar y en dos rondas si son
    /// dos o mas.
    public func sincronizarAhora(id: String? = nil) async {
        if let id {
            guard let fila = filas.first(where: { $0.id == id }) else { return }
            await conUno(fila)
        } else {
            _ = await conTodos(motivo: .manual)
        }
    }

    /// Una vuelta preguntando con un aparato.
    private func conUno(_ fila: Fila) async {
        guard !ocupado else { return }
        // La direccion puede haber cambiado desde que se vio (la IP del movil
        // cambia al reconectarse): la de siempre si contesta, si no la del
        // anuncio.
        ocupado = true
        fase = "Buscando a \(fila.nombre)…"
        // Cuenta como vuelta tambien si no llega a conectar: el boton de la
        // lista gira y, si no se encuentra, se pone en rojo.
        let giro = EstadoDeLaVuelta.compartido
        giro.empieza()
        guard let direccion = await llegar(fila) else {
            giro.acaba(bien: false)
            ocupado = false
            fase = nil
            poner(fila.id, .noEncontrado, direccion: nil)
            error = "\(fila.nombre): no se encuentra. Que tenga PixPin abierto y esté en esta Wi-Fi."
            return
        }
        fase = "Conectando con \(fila.nombre)…"
        let fin = await vuelta(con: direccion, id: fila.id, alcance: .preguntar)
        // Como acabo ya lo apunto la vuelta de dentro.
        giro.acaba(bien: nil)
        ocupado = false
        fase = nil
        switch fin {
        case let .hecha(r):
            if !r.cancelada { ultimoResultado = "Al día con \(r.otroNombre)\n" + textoDe(r) }
            error = nil
            if r.trajo { alTraerCambios?() }
        case .ocupado:
            error = "\(fila.nombre): está sincronizando con otro aparato. Prueba otra vez en un momento."
        case let .fallo(texto):
            error = "\(fila.nombre): \(texto)"
        }
        latido()
    }

    /// **«Conectar con una dirección»** (`QueCampo::Direccion` de Windows):
    /// una vuelta preguntando con el aparato que escucha en `direccion`,
    /// escrita a mano (la que ensena el otro en «La de este aparato»). Si
    /// sale bien, la direccion queda apuntada para la proxima vez.
    public func sincronizarCon(direccion: Direccion) async {
        guard !ocupado else { return }
        ocupado = true
        error = nil
        fase = "Conectando con \(direccion.host)…"
        let fin = await vuelta(con: direccion, id: nil, alcance: .preguntar)
        ocupado = false
        fase = nil
        switch fin {
        case let .hecha(r):
            if !r.cancelada { ultimoResultado = "Al día con \(r.otroNombre)\n" + textoDe(r) }
            if r.trajo { alTraerCambios?() }
        case .ocupado:
            error = "\(direccion.host): está sincronizando con otro aparato. Prueba otra vez en un momento."
        case let .fallo(texto):
            error = "\(direccion.host): \(texto)"
        }
        // Puede ser un miembro nuevo para esta lista.
        await refrescar()
    }

    /// **Todos de una vez** (`lanzar_con_todos` de Windows): uno detras de
    /// otro y sin preguntar, con lo elegido la ultima vez con cada uno. Con
    /// dos o mas, **dos rondas**: en la primera este queda con todo, pero los
    /// primeros se quedaron con lo de antes de hablar con los ultimos; la
    /// segunda lo reparte. Uno que falla no se reintenta en la segunda.
    /// Devuelve si fue bien (con alguno al dia, o con nadie a quien llamar).
    private func conTodos(motivo: MotivoSincro) async -> Bool {
        guard sePuedeSincronizar(motivo, automatica: automatica, bajoConsumo: bajoConsumo,
                                 presencia: presencia) else { return true }
        let lista = filas.filter { $0.direccion != nil }
        guard !lista.isEmpty, !ocupado else { return true }
        let manual = motivo == .manual
        ocupado = true
        // Toda la ronda cuenta como una vuelta (`lanzar_con_todos` de
        // Windows): sale si quedo al dia con alguno; que uno estuviera
        // apagado no pone el boton en rojo.
        let giro = EstadoDeLaVuelta.compartido
        giro.empieza()
        var alDia: [String] = []
        var fallaron: [(String, String)] = []
        defer {
            ocupado = false
            fase = nil
            giro.acaba(bien: !alDia.isEmpty ? true : fallaron.isEmpty ? nil : false)
        }
        let rondas = rondasConTodos(aparatos: UInt32(lista.count), soloTareas: false)
        var textos: [String] = []
        var avisos: [String] = []
        var trajo = false
        for ronda in 1 ... rondas {
            for fila in lista where !fallaron.contains(where: { $0.0 == fila.nombre }) {
                guard let direccion = filas.first(where: { $0.id == fila.id })?.direccion else { continue }
                let rotulo = rondas > 1 ? "Ronda \(ronda) de \(rondas) · " : ""
                fase = "\(rotulo)Sincronizando con \(fila.nombre)…"
                switch await vuelta(con: direccion, id: fila.id, alcance: .comoSiempre) {
                case let .hecha(r):
                    trajo = trajo || r.trajo
                    if !alDia.contains(r.otroNombre) { alDia.append(r.otroNombre) }
                    // De la segunda ronda solo se cuenta lo que hizo algo.
                    if ronda == 1 || r.traidos + r.enviados + r.fusionados + r.borrados > 0 {
                        textos.append("\(r.otroNombre): \(r.texto.split(separator: "\n").first ?? "")")
                    }
                    for a in r.avisos where !avisos.contains(a) { avisos.append(a) }
                case .ocupado:
                    continue
                case let .fallo(texto):
                    fallaron.append((fila.nombre, texto))
                }
            }
        }
        if trajo { alTraerCambios?() }
        let rotulo = manual ? "" : "Automática: "
        if alDia.isEmpty {
            guard let primero = fallaron.first else { return true }
            let t = "\(primero.0): \(primero.1)"
            if manual { error = t } else { ultimoResultado = rotulo + "no se pudo (\(t))" }
            return false
        }
        let titulo = alDia.count == 1 ? "Al día con \(alDia[0])" : "Al día con \(alDia.count) aparatos"
        if let primero = fallaron.first {
            avisos.append("Quedó pendiente \(fallaron.map { "«\($0.0)»" }.joined(separator: ", ")): \(primero.1)")
        }
        ultimoResultado = ([rotulo + titulo] + textos + avisos).joined(separator: "\n")
        if manual { error = nil }
        return true
    }

    /// **Las tareas al instante** con todos los que contestan: solo esos
    /// chats y una ronda. Sin avisos: si no llego a alguno, la vuelta entera
    /// de detras lo arregla.
    private func tareasAlInstante(_ fichas: [String]) async {
        let lista = filas.filter { $0.direccion != nil }
        guard !lista.isEmpty, !ocupado else { return }
        ocupado = true
        let giro = EstadoDeLaVuelta.compartido
        giro.empieza()
        var trajo = false
        var alDia: [String] = []
        defer {
            ocupado = false
            giro.acaba(bien: alDia.isEmpty ? nil : true)
        }
        for fila in lista {
            guard let direccion = fila.direccion else { continue }
            if case let .hecha(r) = await vuelta(con: direccion, id: fila.id, alcance: .tareas(fichas)) {
                trajo = trajo || r.trajo
                alDia.append(r.otroNombre)
            }
        }
        if !alDia.isEmpty { ultimoResultado = "Tareas al instante: al día con \(alDia.joined(separator: ", "))" }
        if trajo { alTraerCambios?() }
    }

    /// Una vuelta con `direccion`, fuera del hilo principal. No toca
    /// `ocupado`: eso es de quien llama.
    private func vuelta(con direccion: Direccion, id: String?, alcance: Alcance) async -> FinVuelta {
        if let id { poner(id, .sincronizando, direccion: direccion) }
        // El boton de la lista gira mientras dura (`una_vuelta` de Windows)
        // y apunta como acabo: cancelada u ocupado no cuentan.
        let giro = EstadoDeLaVuelta.compartido
        giro.empieza()
        let nucleo = self.nucleo
        let puente = PuenteElegir { [weak self] pregunta, responder in
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self else { return responder(nil) }
                    self.pregunta?.contestar(nil)
                    self.pregunta = PreguntaPendiente(pregunta, responder: responder)
                }
            }
        }
        do {
            let r = try await Task.detached(priority: .utility) { () throws -> ResumenSincro in
                switch alcance {
                case .comoSiempre:
                    try nucleo.sincronizarCon(host: direccion.host, puerto: direccion.puerto)
                case .preguntar:
                    try nucleo.sincronizarPreguntando(host: direccion.host, puerto: direccion.puerto,
                                                      elegir: puente)
                case let .tareas(fichas):
                    try nucleo.sincronizarTareas(host: direccion.host, puerto: direccion.puerto,
                                                 fichas: fichas)
                }
            }.value
            pregunta = nil
            ultimaVuelta[r.otroId] = reloj()
            poner(r.otroId, r.cancelada ? .contesta : .hecho(r.texto), direccion: direccion)
            if !r.cancelada, let i = filas.firstIndex(where: { $0.id == r.otroId }) {
                filas[i].ultimaVez = ahoraMs()
            }
            apuntar(r.otroId, direccion)
            giro.acaba(bien: r.cancelada ? nil : true)
            return .hecha(r)
        } catch {
            pregunta = nil
            let texto = textoDeError(error)
            if esOcupado(texto) {
                giro.acaba(bien: nil)
                if let id { poner(id, .contesta, direccion: direccion) }
                return .ocupado
            }
            giro.acaba(bien: false)
            if let id { poner(id, .fallo(texto), direccion: direccion) }
            return .fallo(texto)
        }
    }

    // MARK: - Enviar a…

    /// **«Enviar a…» un aparato del grupo**: manda `urls` a lo que ese
    /// aparato tenga delante (su chat abierto, su lienzo si son fotos, o la
    /// conversacion general), fuera del hilo principal.
    public func enviar(_ urls: [URL], a id: String) async {
        guard !urls.isEmpty, let fila = filas.first(where: { $0.id == id }) else { return }
        guard let direccion = await llegar(fila) else {
            envios[id] = .fallo("\(fila.nombre) no contesta.")
            return
        }
        envios[id] = .enviando(urls.count)
        let nucleo = self.nucleo
        let rutas = urls.map(\.path)
        do {
            let r = try await Task.detached(priority: .utility) {
                try nucleo.enviarAAparato(host: direccion.host, puerto: direccion.puerto, rutas: rutas)
            }.value
            envios[id] = estadoDeEnvio(r, a: fila.nombre)
        } catch {
            envios[id] = .fallo(textoDeError(error))
        }
    }

    // MARK: - Presencia

    /// Relee la identidad, el anuncio y `[sincro] presencia` (fuera del hilo
    /// principal: la etiqueta cuesta PBKDF2 la primera vez) y enciende o
    /// apaga la presencia segun haya grupo y este permitida.
    public func refrescar() async {
        let nucleo = self.nucleo
        do {
            let (info, anuncio, deja, ajustes, grupo) = try await Task.detached(priority: .utility) {
                try (nucleo.aparato(), nucleo.anuncio(), nucleo.anuncioDePresencia(), try? nucleo.ajustes(),
                     (try? nucleo.miembrosDelGrupo()) ?? [])
            }.value
            let (compartidos, direcciones) = await Task.detached(priority: .utility) {
                ((try? nucleo.hayChatsCompartidos()) ?? false, nucleo.direccionesRecordadas())
            }.value
            aparato = info
            miembros = grupo
            hayCompartidos = compartidos
            recordadas = direccionesConNombre(direcciones, miembros: grupo, personas: nombresPersonas)
            codigoLegible = info.grupo.map { PixPinCore.codigoLegible(codigo: $0) }
            etiqueta = anuncio?.etiqueta
            presencia = ajustes?.sincroPresencia ?? true
            // iPad: la sincronizacion sola de `pixpinmax.toml`, la primera
            // vez (despues la cambian los Ajustes, que guardan el TOML).
            if !automaticaLeida, let ajustes {
                automaticaLeida = true
                automatica = ajustes.sincroAutomatica
            }
            // Con grupo, o sin el pero dentro de un chat compartido con
            // personas (que llaman aqui): se escucha y se anuncia, salvo que
            // la presencia este apagada en los ajustes.
            if let deja, presencia {
                try encender(deja)
            } else {
                cerrarPuerta()
                // Sin presencia se sigue buscando a los del grupo: a mano se
                // puede sincronizar igual (como Windows).
                if etiqueta != nil { vecinos.empezar() } else { vecinos.parar(); filas = [] }
            }
            if etiqueta == nil { porRecordada = [] }
            vecinosCambiaron()
            mirarRecordadas()
        } catch {
            self.error = textoDeError(error)
        }
        latido()
        // Las personas se vuelven a mirar ya (un chat nuevo, la vuelta a la app).
        ultimoSondeoPersonas = 0
        latidoPersonas()
    }

    private func encender(_ anuncio: AnuncioBonjour) throws {
        if puerta == nil {
            let p = try Puerta(nucleo: nucleo)
            p.arrancar { [weak self] resultado in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.atendido(resultado) }
                }
            }
            puerta = p
            puerto = p.puerto
        }
        if let puerto { anunciador.anunciar(anuncio, puerto: puerto) }
        vecinos.empezar()
    }

    private func cerrarPuerta() {
        anunciador.retirar()
        puerta?.cerrar()
        puerta = nil
        puerto = nil
    }

    /// Alguien llamo a la puerta de este Mac.
    private func atendido(_ resultado: Result<Atendido, Error>) {
        switch resultado {
        case let .success(.aparato(id, nombre)):
            // Sincronizo o se unio: puede haber chats nuevos y miembros
            // nuevos en el grupo.
            ultimaVuelta[id] = reloj()
            ultimoResultado = "\(nombre) sincronizó con este iPad."
            alTraerCambios?()
            Task { await refrescar() }
        case let .success(.persona(_, nombre)):
            // Una persona de un chat compartido se junto con este Mac.
            ultimoResultado = "\(nombre) sincronizó un chat compartido con este iPad."
            alTraerCambios?()
        case let .success(.archivos(_, nombre, llegadas)):
            ultimasLlegadas = llegadas
            ultimoResultado = textoDeLlegadas(llegadas, de: nombre)
            // **El aviso al recibirlo** (Windows, 17-oct): PixPin puede estar
            // en la barra de menus sin nada a la vista, y lo que llega de otro
            // aparato del grupo sin escanear nada tiene que notarse.
            // iPad: el aviso lo pone quien tenga los avisos (`GanchosSincro`).
            if !llegadas.isEmpty {
                let titulo = "Llegó de \(nombre)", texto = textoDeLlegadas(llegadas, de: nombre)
                if let gancho = GanchosSincro.alLlegar {
                    gancho(titulo, texto)
                } else {
                    avisarDeLlegada(titulo: titulo, texto: texto)
                }
            }
            for l in llegadas where l.donde == .lienzo {
                NotificationCenter.default.post(name: .pixpinFotoAlLienzo, object: l)
            }
            alTraerCambios?()
        case .success:
            break
        case let .failure(e):
            NSLog("PixPin: una llamada de otro aparato fallo: %@", textoDeError(e))
        }
    }

    /// La lista de Bonjour cambio (o los miembros): rehace las filas de mi
    /// grupo y sondea a los nuevos (la sonda da su direccion, que se apunta
    /// en `direcciones.txt`, y le dice al reloj que contesta). Los miembros
    /// que no se anuncian salen tambien, como «No encontrado», salvo que
    /// contestaran en su direccion recordada.
    private func vecinosCambiaron() {
        guard let etiqueta, let yo = aparato?.id else {
            filas = []
            return
        }
        let antes = Dictionary(filas.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        let vez = Dictionary(miembros.map { ($0.id, $0.ultimaVez) }, uniquingKeysWith: { a, _ in a })
        var nuevas: [Fila] = []
        var aSondear: [Vecino] = []
        for v in vecinos.delGrupo(etiqueta: etiqueta, yo: yo) {
            if let f = antes[v.id] {
                nuevas.append(Fila(id: v.id, nombre: v.nombre, letra: v.datos.letra,
                                   direccion: f.direccion,
                                   estado: f.estado == .noEncontrado ? .mirando : f.estado,
                                   ultimaVez: max(f.ultimaVez, vez[v.id] ?? 0)))
                if f.direccion == nil, f.estado == .noEncontrado { aSondear.append(v) }
            } else {
                nuevas.append(Fila(id: v.id, nombre: v.nombre, letra: v.datos.letra,
                                   direccion: nil, estado: .mirando, ultimaVez: vez[v.id] ?? 0))
                aSondear.append(v)
            }
        }
        for m in miembros where !nuevas.contains(where: { $0.id == m.id }) {
            let f = antes[m.id]
            let sigue = porRecordada.contains(m.id)
            nuevas.append(Fila(id: m.id, nombre: f?.nombre ?? m.nombre, letra: m.letra ?? "",
                               direccion: sigue ? f?.direccion : nil,
                               estado: sigue ? (f?.estado ?? .contesta) : Self.sinRed(f?.estado),
                               ultimaVez: max(f?.ultimaVez ?? 0, m.ultimaVez)))
        }
        filas = ordenadas(nuevas)
        for v in aSondear {
            Task {
                let d = await Vecinos.sondear(v.punto)
                poner(v.id, d == nil ? .noContesta : .contesta, direccion: d)
                if let d { apuntar(v.id, d) }
                // Un anuncio de Bonjour que va y viene (la Wi-Fi que se
                // duerme) no es «aparecer» si se sincronizo con el hace poco:
                // el reloj solo ve aparecer a quien pasa de no a si.
                if d != nil, tocaPorAparecer(ultimaVuelta: ultimaVuelta[v.id], ahora: reloj()) {
                    relojAuto.responde(id: v.id, si: false)
                }
                relojAuto.responde(id: v.id, si: d != nil)
                latido()
            }
        }
    }

    /// El estado de un miembro que no se anuncia: lo que estaba haciendo se
    /// conserva (una vuelta en marcha, su resultado); lo demas es que no
    /// esta.
    private static func sinRed(_ antes: Estado?) -> Estado {
        switch antes {
        case .sincronizando?, .hecho?, .fallo?: return antes!
        default: return .noEncontrado
        }
    }

    /// Los disponibles primero y luego por nombre, como en el movil.
    private func ordenadas(_ v: [Fila]) -> [Fila] {
        v.sorted {
            let (a, b) = ($0.direccion != nil, $1.direccion != nil)
            if a != b { return a }
            return $0.nombre.localizedStandardCompare($1.nombre) == .orderedAscending
        }
    }

    /// **Prueba en su direccion recordada** a los miembros que Bonjour no
    /// ensena (`sondear` de Windows, pero una vez y no cada 4 s): al
    /// refrescar y al abrir la ventana de Sincronizar. Quien contesta queda
    /// disponible y cuenta para el reloj como «aparecio».
    public func mirarRecordadas() {
        guard etiqueta != nil else { return }
        let nucleo = self.nucleo
        for m in miembros {
            guard let d = m.direccion, !probando.contains(m.id),
                  filas.first(where: { $0.id == m.id })?.direccion == nil else { continue }
            probando.insert(m.id)
            Task {
                let si = await Task.detached(priority: .utility) {
                    nucleo.sondearDireccion(host: d.host, puerto: d.puerto)
                }.value
                probando.remove(m.id)
                guard si, filas.first(where: { $0.id == m.id })?.direccion == nil else { return }
                porRecordada.insert(m.id)
                poner(m.id, .contesta, direccion: Direccion(host: d.host, puerto: d.puerto))
                filas = ordenadas(filas)
                if tocaPorAparecer(ultimaVuelta: ultimaVuelta[m.id], ahora: reloj()) {
                    relojAuto.responde(id: m.id, si: false)
                }
                relojAuto.responde(id: m.id, si: true)
                latido()
            }
        }
    }

    /// **Donde llamar ahora a `fila`** (`llegar` de Windows): la direccion
    /// de siempre si contesta a la sonda; si no, la que de su anuncio de
    /// Bonjour (que se apunta). `nil` si ni contesta ni se anuncia.
    func llegar(_ fila: Fila) async -> Direccion? {
        let nucleo = self.nucleo
        let recordada = miembros.first(where: { $0.id == fila.id })?.direccion
            .map { Direccion(host: $0.host, puerto: $0.puerto) }
        let base = fila.direccion ?? recordada
        if let base {
            let si = await Task.detached(priority: .utility) {
                nucleo.sondearDireccion(host: base.host, puerto: base.puerto)
            }.value
            if si { return base }
        }
        // Solo se resuelven los candidatos: el mismo id, o el mismo nombre si
        // su anuncio viene sin id.
        var vistos: [VistoEnRed] = []
        if let etiqueta, let yo = aparato?.id {
            for v in vecinos.delGrupo(etiqueta: etiqueta, yo: yo)
                where v.datos.id == fila.id || (v.datos.id.isEmpty && v.nombre == fila.nombre) {
                if let d = await Vecinos.sondear(v.punto) {
                    vistos.append(VistoEnRed(id: v.datos.id, nombre: v.nombre, host: d.host, puerto: d.puerto))
                }
            }
        }
        guard let base else {
            guard let v = vistos.first else { return nil }
            let d = Direccion(host: v.host, puerto: v.puerto)
            apuntar(fila.id, d)
            return d
        }
        switch dondeEstaAparato(id: fila.id, nombre: fila.nombre, host: base.host, puerto: base.puerto,
                                contesta: false, vistos: vistos) {
        case .laDeSiempre:
            return base
        case let .nueva(host, puerto):
            NSLog("PixPin: %@ cambió de dirección", fila.nombre)
            let d = Direccion(host: host, puerto: puerto)
            apuntar(fila.id, d)
            return d
        case .noEsta:
            return nil
        }
    }

    /// Apunta en `sincro/direcciones.txt` donde contesto `id` (fuera del
    /// hilo principal; lo mismo otra vez no reescribe nada).
    private func apuntar(_ id: String, _ d: Direccion) {
        let nucleo = self.nucleo
        Task.detached(priority: .utility) {
            _ = try? nucleo.apuntarDireccion(id: id, host: d.host, puerto: d.puerto)
        }
    }

    private func poner(_ id: String, _ estado: Estado, direccion: Direccion?) {
        guard let i = filas.firstIndex(where: { $0.id == id }) else { return }
        filas[i].estado = estado
        if let direccion { filas[i].direccion = direccion }
    }

    // MARK: - Lo automatico (el reloj)

    /// Algo cambio aqui: la vuelta con todos sale tras 30 s sin mas cambios.
    public func cambioLocal() {
        cambios &+= 1
        relojAuto.verCambios(cambios: cambios, ahora: ahoraMs())
        relojPersonas.verCambios(cambios: cambios, ahora: ahoraMs())
        latido()
        latidoPersonas()
    }

    /// Cambiaron las tareas del chat de `ficha`: salen al instante.
    public func cambiaronTareas(_ ficha: String) {
        relojAuto.avisanTareas(ficha: ficha, ahora: ahoraMs())
        latido()
    }

    private func cambioDeEnergia() {
        bajoConsumo = ProcessInfo.processInfo.isLowPowerModeEnabled
        latido()
        latidoPersonas()
    }

    /// Si lo automatico puede andar ahora.
    private var autoEncendida: Bool {
        etiqueta != nil && sePuedeSincronizar(.periodo, automatica: automatica, bajoConsumo: bajoConsumo,
                                              presencia: presencia)
    }

    private func ahoraMs() -> Int64 { Int64((reloj().timeIntervalSince1970 * 1000).rounded()) }

    /// **Mira que toca** (`Reloj::siguiente`) y, si no toca nada, duerme de
    /// una vez hasta lo proximo (`proximoMs`). Cualquier aviso (un cambio,
    /// una tarea, un aparato que aparece, el final de una vuelta) lo vuelve
    /// a llamar y rearma el sueno.
    private func latido() {
        espera?.cancel()
        espera = nil
        let ids = relojAuto.conQuien()
        let nombres = ids.map { id in filas.first { $0.id == id }?.nombre ?? id }
        if nombres != conQuienAuto { conQuienAuto = nombres }
        let fallos = relojAuto.fallosSeguidos()
        if fallos != fallosAuto { fallosAuto = fallos }
        guard autoEncendida else { return }
        let ahora = ahoraMs()
        let enMarcha = ocupado || corriendo || corriendoPersonas
        switch relojAuto.siguiente(ahora: ahora, ocupado: enMarcha) {
        case .nada:
            break
        case .olvidar:
            _ = relojAuto.tomarTareas()
        case let .tareas(fichas):
            _ = relojAuto.tomarTareas()
            corriendo = true
            Task {
                await tareasAlInstante(fichas)
                corriendo = false
                latido()
                latidoPersonas()
            }
            return
        case let .entera(motivo):
            // Si no sale (otra vuelta a mano a la vez), se cuenta como
            // hecha: se vuelve a mirar en el siguiente periodo.
            relojAuto.empezo(ahora: ahora)
            corriendo = true
            Task {
                let bien = await conTodos(motivo: motivo == .aparecio ? .aparecio
                    : motivo == .cambio ? .quieto : .periodo)
                corriendo = false
                if relojAuto.acabo(bien: bien, cambios: cambios, ahora: ahoraMs()) {
                    // El tercer fallo seguido, una vez.
                    error = "La sincronización automática falla desde hace un rato"
                        + (ultimoResultado.map { ": \($0)" } ?? ".")
                }
                latido()
                latidoPersonas()
            }
            return
        }
        guard let ms = relojAuto.proximoMs(ahora: ahora, corriendo: enMarcha) else { return }
        espera = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(max(ms, 20)))
            guard !Task.isCancelled else { return }
            self?.latido()
        }
    }

    // MARK: - Las personas de los chats compartidos (lo automatico)

    /// Cada cuanto se mira si las personas de los chats compartidos estan en
    /// la red (`SONDEO_PERSONAS` de Windows): un minuto, sin prisa.
    static let sondeoPersonasMs: Int64 = 60_000

    /// Si lo automatico con las personas puede andar: hay chats compartidos
    /// y la sincronizacion sola esta permitida (la misma regla que el grupo).
    private var personasEncendida: Bool {
        hayCompartidos && sePuedeSincronizar(.periodo, automatica: automatica, bajoConsumo: bajoConsumo,
                                             presencia: presencia)
    }

    /// **El reloj de las personas** (el bucle de `automatica.rs` de Windows):
    /// cada minuto se mira quien de ellas contesta y, cuando toca (aparecer,
    /// un cambio aqui tras 30 s, cada 10 min), una vuelta con cada una que
    /// contesto, solo con los chats que compartimos. Nunca a la par que una
    /// vuelta del grupo.
    private func latidoPersonas() {
        esperaPersonas?.cancel()
        esperaPersonas = nil
        guard personasEncendida else { return }
        let ahora = ahoraMs()
        if !sondeandoPersonas, ahora - ultimoSondeoPersonas >= Self.sondeoPersonasMs {
            sondearPersonas()
        }
        let enMarcha = ocupado || corriendo || corriendoPersonas
        if case .entera = relojPersonas.siguiente(ahora: ahora, ocupado: enMarcha) {
            relojPersonas.empezo(ahora: ahora)
            corriendoPersonas = true
            let ids = relojPersonas.conQuien()
            Task {
                let bien = await conPersonas(ids)
                corriendoPersonas = false
                if relojPersonas.acabo(bien: bien, cambios: cambios, ahora: ahoraMs()) {
                    error = "La sincronización automática con los chats compartidos falla desde hace un rato"
                        + (ultimoResultado.map { ": \($0)" } ?? ".")
                }
                latidoPersonas()
                latido()
            }
            return
        }
        // Las tareas al instante son del grupo: aqui no se apuntan nunca.
        var ms = Self.sondeoPersonasMs - (ahora - ultimoSondeoPersonas)
        if let p = relojPersonas.proximoMs(ahora: ahora, corriendo: corriendoPersonas) { ms = min(ms, p) }
        // Con otra vuelta en marcha se vuelve a mirar al acabar (y, por si
        // acaso, cada segundo: nunca girando en vacio).
        if enMarcha { ms = max(ms, 1_000) }
        let dormir = max(ms, 20)
        esperaPersonas = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(dormir))
            guard !Task.isCancelled else { return }
            self?.latidoPersonas()
        }
    }

    /// **Quien de las personas contesta**: en su direccion recordada y, si
    /// no, en su anuncio de Bonjour (por el `id`; su presencia sin grupo se
    /// anuncia asi), que se apunta. Lo dice al reloj de las personas.
    private func sondearPersonas() {
        sondeandoPersonas = true
        ultimoSondeoPersonas = ahoraMs()
        let nucleo = self.nucleo
        Task {
            let lista = await Task.detached(priority: .utility) { personasDeLosChats(nucleo) }.value
            var siguen = Set<String>()
            for p in lista {
                siguen.insert(p.id)
                nombresPersonas[p.id] = p.nombre
                var donde: Direccion?
                if let r = p.recordada {
                    let si = await Task.detached(priority: .utility) {
                        nucleo.sondearDireccion(host: r.host, puerto: r.puerto)
                    }.value
                    if si { donde = r }
                }
                if donde == nil, let v = vecinos.todos.first(where: { $0.id == p.id }),
                   let d = await Vecinos.sondear(v.punto) {
                    donde = d
                    apuntar(p.id, d)
                }
                dondePersonas[p.id] = donde
                relojPersonas.responde(id: p.id, si: donde != nil)
            }
            // Quien ya no esta en ningun chat compartido conmigo, fuera.
            for id in relojPersonas.conQuien() where !siguen.contains(id) {
                relojPersonas.olvidar(id: id)
                dondePersonas[id] = nil
            }
            personasQueContestan = relojPersonas.conQuien().map { nombresPersonas[$0] ?? $0 }
            sondeandoPersonas = false
            latidoPersonas()
        }
    }

    /// **Una vuelta con cada persona que contesto** (`con_personas` de
    /// Windows): solo los chats que compartimos (`chats` vacio), con la
    /// direccion de la sonda. Ocupado no es un fallo: la proxima. Devuelve
    /// si fue bien.
    private func conPersonas(_ ids: [String]) async -> Bool {
        let nucleo = self.nucleo
        var bien = true
        var trajo: [String] = []
        var fallo: String?
        for id in ids {
            guard let d = dondePersonas[id] else { continue }
            let nombre = nombresPersonas[id] ?? id
            do {
                let r = try await Task.detached(priority: .utility) {
                    try nucleo.sincronizarConPersona(host: d.host, puerto: d.puerto, id: id, chats: [])
                }.value
                if r.puertoDelOtro > 0 { apuntar(id, Direccion(host: d.host, puerto: r.puertoDelOtro)) }
                if r.trajoAlgo {
                    trajo.append("Chat compartido con \(r.otroNombre): \(resumenDePersona(r))")
                }
            } catch {
                let texto = textoDeError(error)
                if esOcupado(texto) { continue }
                bien = false
                fallo = "No se pudo sincronizar solo con \(nombre) (\(texto))"
            }
        }
        if !trajo.isEmpty {
            ultimoResultado = "Automática: " + trajo.joined(separator: "\n")
            alTraerCambios?()
        } else if let fallo {
            ultimoResultado = "Automática: \(fallo)"
        }
        return bien
    }

    /// Hace `trabajo` con el nucleo fuera del hilo principal, con su fase.
    private func trabajar(_ texto: String, _ trabajo: @escaping @Sendable (Nucleo) throws -> Void) async {
        guard !ocupado else { return }
        ocupado = true
        fase = texto
        error = nil
        let nucleo = self.nucleo
        do {
            try await Task.detached(priority: .utility) { try trabajo(nucleo) }.value
        } catch {
            self.error = textoDeError(error)
        }
        ocupado = false
        fase = nil
    }
}

/// La frase de una vuelta con sus avisos debajo.
func textoDe(_ r: ResumenSincro) -> String {
    r.avisos.isEmpty ? r.texto : ([r.texto] + r.avisos).joined(separator: "\n\n")
}

/// Si el error es el «ocupado» del protocolo (`mensajes::OCUPADO`): otra
/// vuelta en marcha aqui o alli. No es un fallo: se intenta la proxima vez.
func esOcupado(_ texto: String) -> Bool {
    texto.contains("Está sincronizando con otro aparato")
}

/// **«¿Qué sincronizar?» esperando**: lo que el nucleo pregunta y como
/// contestarle. Contestar una vez basta; las demas no hacen nada (cerrar la
/// ventana contesta «cancelar»: un hilo esperando para siempre dejaria el
/// aparato ocupado).
public final class PreguntaPendiente: Identifiable, @unchecked Sendable {
    public let id = UUID()
    public let pregunta: PreguntaSincro
    private let responder: @Sendable (EleccionSincro?) -> Void
    private let cerrojo = NSLock()
    private var contestada = false

    init(_ pregunta: PreguntaSincro, responder: @escaping @Sendable (EleccionSincro?) -> Void) {
        self.pregunta = pregunta
        self.responder = responder
    }

    public func contestar(_ eleccion: EleccionSincro?) {
        cerrojo.lock()
        let ya = contestada
        contestada = true
        cerrojo.unlock()
        if !ya { responder(eleccion) }
    }
}

/// El `ElegirSincro` del nucleo: el hilo de la vuelta se queda aqui hasta que
/// la ventana contesta.
final class PuenteElegir: ElegirSincro, @unchecked Sendable {
    typealias Mostrar = @Sendable (PreguntaSincro, @escaping @Sendable (EleccionSincro?) -> Void) -> Void
    private let mostrar: Mostrar

    init(_ mostrar: @escaping Mostrar) {
        self.mostrar = mostrar
    }

    func elegir(pregunta: PreguntaSincro) -> EleccionSincro? {
        final class Caja: @unchecked Sendable { var valor: EleccionSincro? }
        let caja = Caja()
        let listo = DispatchSemaphore(value: 0)
        mostrar(pregunta) { e in
            caja.valor = e
            listo.signal()
        }
        listo.wait()
        return caja.valor
    }
}

/// La clave de `UserDefaults` del nombre de este iPad (la que lee
/// `nombreDeEsteAparato()` en `Almacen.swift`).
public let claveNombreDelAparato = "pixpin.nombreDelAparato"

/// Lo que la sincronizacion avisa a otras partes de la app sin depender de
/// ellas (iPad: los avisos los lleva otra zona). Sin gancho, no pasa nada.
@MainActor
public enum GanchosSincro {
    /// Llego algo suelto de otro aparato («Enviar a…»): titulo y texto del
    /// aviso para ensenarlo en la app.
    public static var alLlegar: ((String, String) -> Void)?
}

// MARK: - Las personas de los chats compartidos (lo puro)

/// Una persona de un chat compartido a la que llamar solo.
public struct PersonaDelChat: Equatable, Sendable {
    public var id: String
    public var nombre: String
    /// Su ultima direccion conocida (`sincro/direcciones.txt`), si hay.
    public var recordada: Direccion?

    public init(id: String, nombre: String, recordada: Direccion? = nil) {
        self.id = id
        self.nombre = nombre
        self.recordada = recordada
    }
}

/// **Con quien de los chats compartidos se sincroniza solo**
/// (`personas_a_llamar` de Windows): las personas que estan dentro de un chat
/// donde tambien estoy yo dentro, sin mis propios aparatos (con esos ya va
/// la vuelta del grupo). Una vez cada una, en el orden de los chats.
public func personasALlamar(_ chats: [ChatCompartidoMac], yo: String, mios: Set<String>) -> [PersonaDelChat] {
    var salida: [PersonaDelChat] = []
    for c in chats where c.estoyDentro {
        for p in c.personas where p.dentro && !p.soyYo && p.id != yo && !mios.contains(p.id) {
            if !salida.contains(where: { $0.id == p.id }) {
                salida.append(PersonaDelChat(id: p.id, nombre: p.nombre))
            }
        }
    }
    return salida
}

/// Las personas a llamar con su direccion recordada, leidas del nucleo
/// (bloqueante: fuera del hilo principal). Sin chats compartidos, nada.
func personasDeLosChats(_ nucleo: Nucleo) -> [PersonaDelChat] {
    guard let chats = try? nucleo.chatsCompartidos(), !chats.isEmpty,
          let yo = try? nucleo.aparato().id else { return [] }
    var mios = Set(((try? nucleo.miembrosDelGrupo()) ?? []).map(\.id))
    mios.insert(yo)
    let dirs = nucleo.direccionesRecordadas()
    return personasALlamar(chats, yo: yo, mios: mios).map { p in
        var p = p
        if let d = dirs.first(where: { $0.id == p.id }) { p.recordada = Direccion(host: d.host, puerto: d.puerto) }
        return p
    }
}

/// Lo hecho con una persona, en una linea (`resumen` de Windows y el movil).
public func resumenDePersona(_ r: ResumenPersona) -> String {
    var v: [String] = []
    if r.traidos > 0 { v.append("\(r.traidos) traídos") }
    if r.enviados > 0 { v.append("\(r.enviados) enviados") }
    return v.isEmpty ? "al día" : v.joined(separator: " · ")
}

/// Una direccion recordada con el nombre de quien es (un aparato del grupo o
/// una persona de un chat compartido), si se sabe.
public struct DireccionConNombre: Identifiable, Equatable, Sendable {
    public var id: String
    public var nombre: String?
    public var direccion: Direccion
    /// Si es de un aparato de mi grupo (si no, de una persona o de alguien
    /// que ya no esta).
    public var esDelGrupo: Bool
}

/// Junta `sincro/direcciones.txt` con los nombres: los del grupo primero y
/// luego por nombre (los sin nombre al final).
public func direccionesConNombre(_ dirs: [DireccionRecordada], miembros: [MiembroGrupo],
                                 personas: [String: String]) -> [DireccionConNombre] {
    let v = dirs.map { d -> DireccionConNombre in
        let m = miembros.first { $0.id == d.id }
        return DireccionConNombre(id: d.id, nombre: m?.nombre ?? personas[d.id],
                                  direccion: Direccion(host: d.host, puerto: d.puerto), esDelGrupo: m != nil)
    }
    return v.sorted {
        if $0.esDelGrupo != $1.esDelGrupo { return $0.esDelGrupo }
        switch ($0.nombre, $1.nombre) {
        case let (a?, b?): return a.localizedStandardCompare(b) == .orderedAscending
        case (_?, nil): return true
        case (nil, _?): return false
        case (nil, nil): return $0.id < $1.id
        }
    }
}
