# PixPin para iPad: plan y reglas (25-oct-2026)

La app de iPad es **la misma PixPin** que Windows, Mac y Android: el mismo formato de
proyectos, la misma sincronización por la Wi‑Fi y la misma cara. Se construye sobre el
**mismo núcleo de Rust** que el Mac (`apple/nucleo`, copiado de PIXPIN_PRO_MAC
`apple/nucleo` en e96241c) y reutiliza tal cual las piezas de Swift del Mac que no tocan
AppKit. Lo de AppKit se reescribe con **SwiftUI + UIKit** pensado para el tacto y el
**Apple Pencil**.

## Referencias

| Qué | Dónde |
|---|---|
| La versión de referencia (funciones, aspecto, textos) | PIXPIN_PRO_WINDOWS (`apps/pixpin`, `crates/`) y sus capturas en `docs/img/` |
| El port de Apple más cercano (Swift, mismo núcleo) | PIXPIN_PRO_MAC `apple/app/Sources/PixPinKit` |
| La API del núcleo para Swift (UniFFI) | `apple/PixPinCore/Sources/PixPinCore/pixpin_nucleo.swift` (generado; buscar con grep, son 42 000 líneas) |
| El plan de Apple (decisiones) | PIXPIN_PRO_MAC `docs/investigacion/2026-10-15-mac-ipad-iphone.md` §4.3 y Fase 4 |

## Estructura

```
apple/
  nucleo/           Rust: el núcleo (igual que el Mac; no se toca sin llevarlo al Mac y a Windows)
  PixPinCore/       Paquete Swift con los enlaces de UniFFI y el xcframework (no versionado)
  app/              Paquete Swift `PixPinIPad` → biblioteca `PixPinKit` (toda la app)
    Sources/PixPin/   @main (lo compila el proyecto de Xcode)
    Sources/PixPinKit/
      Raiz/           Navegación, entorno, barra común de visores, enrutador (comunes a todo)
      Almacen.swift   El estado: chats y mensajes por páginas (del Mac, con UIKit)
      Paleta.swift    Los colores de Windows
      <Zona>/         Una carpeta por zona (ver abajo)
    Tests/PixPinKitTests/
  xcode/project.yml XcodeGen: la app de iPad (iPadOS 18+) y las pruebas
  herramientas/     xcframework.sh (núcleo → xcframework), app.sh (simulador, pruebas, ipa)
```

## Cómo encaja cada zona (contratos)

`Raiz/RaizIPad.swift` monta una `NavigationSplitView` con las secciones de `Seccion`
(lo que en Windows son las ventanas de la bandeja). Cada zona da **su vista pública con
esta firma exacta** (hoy son huecos que se rellenan):

| Zona (carpeta) | Vista pública |
|---|---|
| `Proyectos/` | `VistaProyectos(almacen:)` — lista de proyectos + el chat |
| `Galeria/`, `Timeline/`, `Tareas/`, `Buscar/`, `Lecciones/` | `VistaGaleria(almacen:)`, `VistaTimeline(almacen:)`, `VistaTareas(almacen:)`, `VistaBuscar(almacen:)`, `VistaLeccionesIPad(almacen:)` |
| `Asistente/` | `VistaAsistenteIPad(almacen:)` |
| `Sincro/` | `VistaSincroIPad(almacen:sincro:)` |
| `Ajustes/` | `VistaAjustesIPad(almacen:sincro:)` |
| `Lienzo/` | `EditorLienzoIPad(almacen:chat:mensaje:foto:)` |
| `Documentos/` | `VisorDocumentoIPad(url:titulo:almacen:chat:mensaje:)`, `EditorNotaIPad(almacen:chat:mensaje:)`, `HojaCalculoIPad(almacen:chat:mensaje:)` |
| `Planos/` | `VisorPlanoIPad(url:titulo:nucleo:)`, `Visor3DIPad(url:titulo:nucleo:)` |
| `Pines/` | `CapaPines(almacen:)` — la capa de pines flotantes encima de todo |

Para abrir cosas se usa **`Navegacion.compartida`** (`Raiz/Navegacion.swift`):
`abrir(.lienzo(...))`, `abrirFichero(url, titulo:, chat:, mensaje:)` (elige visor por la
extensión), `irAlChat(chat, mensaje:)`, `cerrarVisor()`. Los visores van a pantalla
completa (el «pin sin bordes» de Windows) y **todos** llevan arriba
`BarraVisorIPad(tipo:icono:titulo:alFijar:) { botones propios }` (`Raiz/BarraVisorIPad.swift`),
con el modo noche común `ModoNoche.compartido`.

Lo que una zona tiene que poner en marcha al arrancar se apunta desde su propio fichero con
`Arranque.apuntar { entorno in ... }`, sin tocar `Raiz/`. El `Almacen` avisa por
`GanchosAlmacen.alEnviar` (mensajes nuevos, para el asistente) y `GanchosAlmacen.alEntrarPdf`.
Un enlace `pixpinmax://` llega a `AbrirDesdeFuera.alEnlace`.

## Reglas

1. **El formato y la sincronización son los de Windows y Android.** Todo lo que se guarda
   pasa por el núcleo (`Nucleo`, `EditorLienzo`, …). Nada de formatos propios del iPad.
2. **Mismo aspecto que Windows**: plano, los colores de `Paleta`, los mismos textos en
   español, las mismas herramientas en el mismo orden.
3. **Solo iPadOS 18** y APIs de iOS: nada de AppKit (`NS*`), `Host`, `NSWorkspace`,
   ScreenCaptureKit, Carbon… Lo de iPadOS 26 solo con `if #available(iOS 26, *)` **y** dentro
   de `#if compiler(>=6.2)` (la CI compila con Xcode 16.4, SDK de iOS 18).
4. Swift 5 (modo de lenguaje `.v5`), `@MainActor` para lo de interfaz, el disco fuera del hilo
   principal (`Task.detached`), como en el Mac.
5. Cada zona escribe **solo en su carpeta**. Lo común (`Raiz/`, `Almacen.swift`,
   `Paleta.swift`, `project.yml`) se cambia a propósito y avisando.
6. Los ficheros copiados del Mac se dejan tal cual siempre que se pueda (para poder traer
   sus arreglos); si hay que tocarlos para iOS, lo mínimo.

## Lo que el iPad hace distinto (y por qué)

| En Windows/Mac | En el iPad |
|---|---|
| Ventanas de la bandeja | Secciones de la barra lateral; con Stage Manager, varias ventanas |
| Pines siempre encima de todo | **Capa de pines dentro de la app** (arrastrar, pellizcar, opacidad), **Picture in Picture** para vídeo y el pin en vivo; iOS no deja dibujar encima de otras apps |
| Capturar una zona de la pantalla | iOS no deja capturar otras apps: **fotos, cámara, escáner de documentos (VisionKit), Fotos y la extensión de Compartir**, y capturar la propia app; el editor de capturas anota encima igual |
| Atajos globales | Atajos de teclado dentro de la app (teclado del iPad) y App Intents (Siri, Atajos) |
| Sincronizar siempre | **Mientras la app está abierta** (iOS cierra la red al pasar a segundo plano); al volver se sincroniza con quien esté. Un PC o Mac encendido hace de punto de encuentro |
| Ratón | Tacto, **Apple Pencil** (presión, inclinación, doble toque/apretar para cambiar herramienta, la palma no pinta) y trackpad |
