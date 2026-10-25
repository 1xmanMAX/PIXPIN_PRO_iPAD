# Guion de medida de docs/investigacion/2026-10-13-ia-local.md (seccion 4.2).
# Lanza llama-server con un modelo, mide carga, RAM y seis pruebas de chat.
# Ej.: .\medir.ps1 -Bin <carpeta llama.cpp> -Modelo x.gguf -Mmproj m.gguf -Imagen prueba.png -Extra "-t 4" -Salida .\q2b
param(
  [string]$Bin, [string]$Modelo, [string]$Mmproj = "", [string]$Imagen = "", [string]$Extra = "", [int]$Puerto = 8691, [string]$Salida
)
$ErrorActionPreference = 'Stop'
$dir = Split-Path $MyInvocation.MyCommand.Path
$args2 = @('-m', $Modelo, '--port', $Puerto, '--host', '127.0.0.1', '-c', '8192', '--jinja', '-np', '1', '--no-ui')
if ($Mmproj) { $args2 += @('--mmproj', $Mmproj) }
if ($Extra) { $args2 += $Extra.Split(' ') }
$t0 = Get-Date
$p = Start-Process -FilePath "$Bin\llama-server.exe" -ArgumentList $args2 -PassThru -WindowStyle Hidden -RedirectStandardError "$Salida.log" -RedirectStandardOutput "$Salida.out"
$listo = $false
for ($i = 0; $i -lt 600; $i++) {
  if ($p.HasExited) { break }
  try { $r = Invoke-WebRequest "http://127.0.0.1:$Puerto/health" -UseBasicParsing -TimeoutSec 2; if ($r.StatusCode -eq 200) { $listo = $true; break } } catch {}
  Start-Sleep -Milliseconds 100
}
$carga = ((Get-Date) - $t0).TotalSeconds
$p.Refresh(); $ramTrasCarga = $p.WorkingSet64
$res = [ordered]@{ modelo = (Split-Path $Modelo -Leaf); extra = $Extra; carga_s = [math]::Round($carga, 2); ram_tras_carga_mb = [math]::Round($ramTrasCarga / 1MB) }

$herramientas = @(
  @{ type = 'function'; function = @{ name = 'dibujar_en_lienzo'; description = 'Dibuja figuras en el lienzo de PixPin. Coordenadas en pixeles, origen arriba a la izquierda, lienzo de 800x600.'; parameters = @{ type = 'object'; required = @('elementos'); properties = @{ elementos = @{ type = 'array'; items = @{ type = 'object'; required = @('tipo', 'x', 'y'); properties = @{ tipo = @{ type = 'string'; enum = @('rectangulo', 'elipse', 'linea', 'flecha', 'texto') }; x = @{ type = 'number' }; y = @{ type = 'number' }; ancho = @{ type = 'number' }; alto = @{ type = 'number' }; color = @{ type = 'string'; description = 'color CSS, ej. #ff0000' }; relleno = @{ type = 'string' }; texto = @{ type = 'string' } } } } } } } },
  @{ type = 'function'; function = @{ name = 'grafica'; description = 'Dibuja una grafica de barras, lineas o pastel en el lienzo a partir de datos.'; parameters = @{ type = 'object'; required = @('tipo', 'etiquetas', 'valores'); properties = @{ tipo = @{ type = 'string'; enum = @('barras', 'lineas', 'pastel') }; titulo = @{ type = 'string' }; etiquetas = @{ type = 'array'; items = @{ type = 'string' } }; valores = @{ type = 'array'; items = @{ type = 'number' } } } } } },
  @{ type = 'function'; function = @{ name = 'ocr'; description = 'Reconoce el texto de un PDF o imagen que no tiene texto seleccionable, con el OCR de Windows.'; parameters = @{ type = 'object'; required = @('archivo'); properties = @{ archivo = @{ type = 'string' }; paginas = @{ type = 'string'; description = 'ej. 1-3' }; idioma = @{ type = 'string'; enum = @('es', 'en') } } } } }
)
$sistema = 'Eres el asistente de PixPin, una app de capturas y notas. Respondes en espanol, breve. Si el usuario pide dibujar, graficar o reconocer texto, llama a la herramienta adecuada.'
function Pedir($mensajes, $conHerr, $nombre) {
  $cuerpo = @{ messages = $mensajes; max_tokens = 400; temperature = 0.7; top_p = 0.8; top_k = 20; chat_template_kwargs = @{ enable_thinking = $false } }
  if ($conHerr) { $cuerpo.tools = $herramientas }
  $json = $cuerpo | ConvertTo-Json -Depth 30 -Compress
  $t = Get-Date
  $r = Invoke-RestMethod "http://127.0.0.1:$Puerto/v1/chat/completions" -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($json)) -TimeoutSec 900
  $seg = ((Get-Date) - $t).TotalSeconds
  $m = $r.choices[0].message
  $o = [ordered]@{ prueba = $nombre; total_s = [math]::Round($seg, 2); prompt_tokens = $r.usage.prompt_tokens; tokens_salida = $r.usage.completion_tokens
    prompt_tps = [math]::Round($r.timings.prompt_per_second, 1); gen_tps = [math]::Round($r.timings.predicted_per_second, 1)
    texto = $m.content; llamadas = ($m.tool_calls | ForEach-Object { $_.function.name + ' ' + $_.function.arguments }) -join ' | ' }
  return $o
}
if (-not $Imagen) { $Imagen = Join-Path $dir 'prueba.png' }
$img = [Convert]::ToBase64String([IO.File]::ReadAllBytes($Imagen))
$pruebas = @()
if ($listo) {
  $pruebas += Pedir @(@{ role = 'system'; content = $sistema }, @{ role = 'user'; content = 'Explica en dos frases para que sirve fijar una captura en pantalla.' }) $false 'texto'
  $pruebas += Pedir @(@{ role = 'system'; content = $sistema }, @{ role = 'user'; content = 'Dibuja en el lienzo un circulo rojo a la izquierda y un cuadrado azul a la derecha, con el titulo CASA arriba.' }) $true 'herramienta_dibujo'
  $pruebas += Pedir @(@{ role = 'system'; content = $sistema }, @{ role = 'user'; content = 'Hazme una grafica de barras de ventas: enero 120, febrero 95, marzo 140.' }) $true 'herramienta_grafica'
  $pruebas += Pedir @(@{ role = 'system'; content = $sistema }, @{ role = 'user'; content = 'Hola, que tal?' }) $true 'sin_herramienta'
  if ($Mmproj) {
    $pruebas += Pedir @(@{ role = 'user'; content = @(@{ type = 'image_url'; image_url = @{ url = "data:image/png;base64,$img" } }, @{ type = 'text'; text = 'Describe esta imagen: que figuras hay, de que color, donde estan y que texto se lee.' }) }) $false 'imagen'
    $pruebas += Pedir @(@{ role = 'system'; content = $sistema }, @{ role = 'user'; content = @(@{ type = 'image_url'; image_url = @{ url = "data:image/png;base64,$img" } }, @{ type = 'text'; text = 'Quiero esta imagen: dibujala dentro del lienzo.' }) }) $true 'imagen_a_lienzo'
  }
}
$p.Refresh(); $res.ram_pico_mb = [math]::Round($p.PeakWorkingSet64 / 1MB); $res.pruebas = $pruebas; $res.listo = $listo
Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
$res | ConvertTo-Json -Depth 10 | Out-File -Encoding utf8 "$Salida.json"
Get-Content "$Salida.json"
