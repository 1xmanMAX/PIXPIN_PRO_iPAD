#!/usr/bin/env python3
"""**Trae al iPad el codigo del Mac** (fusion a tres bandas), a imagen de
`portar-de-windows.py` del Mac.

    apple/herramientas/traer-del-mac.py [/ruta/a/PIXPIN_PRO_MAC] [commit]

- base:  el Mac en el ultimo commit traido (`apple/docs/mac-traido.txt`)
- ipad:  lo que hay aqui
- nuevo: el Mac en `commit`. Por omision, la punta mas reciente (por fecha del
         commit) entre `origin/main`, `origin/mac-desarrollo` y las
         `origin/claude/*` menos `claude/vibrant-gates-0lx4dc` (dice cual).

El repo del Mac por omision es `$PIXPIN_MAC` o `../PIXPIN_PRO_MAC` al lado de
este. Hay que haber hecho `git fetch` en el antes.

Zonas:
- `apple/nucleo/**`, `apple/Cargo.toml`, `apple/rust-toolchain.toml`: si el iPad
  no lo toco desde la base, se toma el del Mac; si el Mac no lo toco, se queda
  el del iPad; si cambiaron los dos, `git merge-file` (los .rs pasan antes por
  `rustfmt` —o el de `$RUSTFMT`— los tres, para que el formato no salga como
  conflicto). Lo borrado en el iPad que el Mac cambio se lista y no vuelve.
- `apple/por-adaptar/**`: no se compila; ahi manda el Mac (se copia tal cual,
  tambien los crates que el iPad no tenia).
- `apple/Cargo.lock`: se toma el del Mac y luego `cargo update --workspace`
  (sin red, `--offline`) para que encaje con los crates del iPad.
- `apple/PixPinCore/Sources/**`: no se fusiona; se regenera con UniFFI al final
  si no quedan conflictos en Rust (`--sin-enlaces` para no hacerlo).
- Swift (`apple/app/Sources/PixPinKit/**`): lo que cambio en el Mac y en el
  iPad existe con la misma ruta (los copiados sin AppKit) se fusiona a tres
  bandas igual. Lo nuevo o cambiado que el iPad no tiene: si no importa nada
  solo del Mac (AppKit, Carbon, ScreenCaptureKit...) y no choca con un nombre
  de fichero o de tipo del iPad ni usa tipos del Mac que el iPad no tiene, se
  copia tal cual; si no, queda en el informe
  para portarlo a mano. Las pruebas y `Sources/PixPin` solo se listan.

Al acabar deja el informe en `apple/target/traer-del-mac.json`, apunta el
commit en `mac-traido.txt` y dice que ficheros quedaron con conflicto
(`<<<<<<< ipad`). Despues: resolverlos, `cargo test -p pixpin-nucleo`,
regenerar los enlaces si hizo falta y portar el Swift de la lista.
"""
# `shutil.copy` y no `copy2`: el fichero copiado lleva la hora de ahora y no
# la del commit del Mac, que puede ser anterior a lo ya compilado (cargo lo
# daria por al dia y no lo recompilaria).
import filecmp
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

APPLE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
IPAD = os.path.dirname(APPLE)
APUNTE = os.path.join(APPLE, 'docs', 'mac-traido.txt')
KIT = 'apple/app/Sources/PixPinKit'
FUSION = ('apple/nucleo', 'apple/Cargo.toml', 'apple/rust-toolchain.toml')
MANDA_MAC = ('apple/por-adaptar',)
SOLO_LISTAR = ('apple/app/Tests', 'apple/app/Sources/PixPin')
SOLO_MAC = re.compile(r'^\s*(@preconcurrency\s+)?import\s+(AppKit|Cocoa|Carbon|ScreenCaptureKit|IOKit|ServiceManagement|QuickLookUI|CoreWLAN|SystemConfiguration|OSAKit|ApplicationServices)\b', re.M)
TIPO = re.compile(r'^(?:@\w+(?:\([^)]*\))?\s+)*(?:(?:public|internal|private|fileprivate|final|open|nonisolated)\s+)*(?:struct|class|enum|protocol|actor|typealias)\s+(\w+)', re.M)
FUERA = 'claude/vibrant-gates-0lx4dc'


def git(repo, *args):
    return subprocess.run(['git', '-C', repo, *args], check=True, capture_output=True, text=True).stdout.strip()


def punta(mac):
    ramas = git(mac, 'for-each-ref', '--format=%(refname:short)', 'refs/remotes/origin').split()
    buenas = [r for r in ramas if r in ('origin/main', 'origin/mac-desarrollo')
              or (r.startswith('origin/claude/') and r != f'origin/{FUERA}')]
    fechas = sorted(((int(git(mac, 'log', '-1', '--format=%ct', r)), r) for r in buenas), reverse=True)
    for f, r in fechas:
        print(f'  {r}: {git(mac, "log", "-1", "--format=%h %ci", r)}')
    return fechas[0][1]


def ficheros(raiz):
    fuera = set()
    for d, _, fs in os.walk(raiz):
        for f in fs:
            fuera.add(os.path.relpath(os.path.join(d, f), raiz))
    return fuera


def existe(p):
    return os.path.isfile(p)


def igual(a, b):
    if existe(a) != existe(b):
        return False
    return not existe(a) or filecmp.cmp(a, b, shallow=False)


def formatear(origen, destino):
    """`rustfmt` por la entrada estandar (no sigue los `mod`); si falla, tal cual."""
    with open(origen, 'rb') as e:
        r = subprocess.run([*os.environ.get('RUSTFMT', 'rustfmt').split(), '--edition', '2024', '--emit', 'stdout'], stdin=e, capture_output=True)
    if r.returncode == 0 and r.stdout:
        with open(destino, 'wb') as s:
            s.write(r.stdout)
    elif origen != destino:
        shutil.copy(origen, destino)


def fusionar(i, b, n, tmp, rs):
    """`git merge-file` sobre `i` (el del iPad). Devuelve True si hubo conflicto."""
    bb, nn = (b if existe(b) else '/dev/null'), n
    if rs:
        formatear(i, i)
        if bb != '/dev/null':
            formatear(bb, os.path.join(tmp, 'base.rs'))
            bb = os.path.join(tmp, 'base.rs')
        formatear(n, os.path.join(tmp, 'nuevo.rs'))
        nn = os.path.join(tmp, 'nuevo.rs')
        if igual(i, nn):
            return False
    r = subprocess.run(['git', 'merge-file', '-L', 'ipad', '-L', 'base', '-L', 'mac', i, bb, nn])
    return r.returncode != 0


def tipos_del_ipad():
    nombres, ficheros_swift = {}, {}
    raiz = os.path.join(IPAD, KIT)
    for d, _, fs in os.walk(raiz):
        for f in fs:
            if not f.endswith('.swift'):
                continue
            p = os.path.join(d, f)
            rel = os.path.relpath(p, IPAD)
            ficheros_swift.setdefault(f, []).append(rel)
            for t in TIPO.findall(open(p, encoding='utf-8', errors='replace').read()):
                nombres.setdefault(t, rel)
    return nombres, ficheros_swift


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    sin_enlaces = '--sin-enlaces' in sys.argv
    if '-h' in sys.argv or '--help' in sys.argv:
        sys.exit(__doc__)
    mac = os.path.abspath(args[0] if args else os.environ.get('PIXPIN_MAC', os.path.join(os.path.dirname(IPAD), 'PIXPIN_PRO_MAC')))
    if len(args) > 1:
        pedido = args[1]
    else:
        print('Puntas del Mac:')
        pedido = punta(mac)
        print(f'Se trae la mas reciente: {pedido}')
    nuevo = git(mac, 'rev-parse', pedido)
    base = open(APUNTE).read().split()[0]
    tmp = tempfile.mkdtemp(prefix='traer-')
    for cual, commit in (('base', base), ('nuevo', nuevo)):
        os.makedirs(os.path.join(tmp, cual))
        tar = subprocess.run(['git', '-C', mac, 'archive', commit, 'apple'], check=True, capture_output=True).stdout
        subprocess.run(['tar', '-x', '-C', os.path.join(tmp, cual)], input=tar, check=True)

    informe = {k: [] for k in ('tomado', 'nuevo', 'fusion', 'conflicto', 'borrado', 'borrado_en_ipad_cambia_en_mac',
                               'por_adaptar', 'swift_fusion', 'swift_conflicto', 'swift_copiado',
                               'swift_a_portar', 'swift_borrado_en_mac', 'pruebas_y_app_del_mac', 'avisos')}
    informe['mac'] = {'base': base, 'nuevo': nuevo, 'pedido': pedido}

    def ruta(cual, rel):
        return os.path.join(tmp, cual, rel)

    cambiados = [l for l in git(mac, 'diff', '--name-only', '--no-renames', base, nuevo, '--', 'apple').splitlines()]
    # Rust y Cargo: todo lo de las zonas, aunque el Mac no lo haya tocado
    # (si no cambio en el Mac, se queda el del iPad: no hace falta mirarlo).
    for rel in cambiados:
        b, i, n = ruta('base', rel), os.path.join(IPAD, rel), ruta('nuevo', rel)
        if rel == 'apple/Cargo.lock':
            if existe(n):
                shutil.copy(n, i)
                informe['tomado'].append(rel)
            continue
        if rel.startswith('apple/PixPinCore/Sources/'):
            continue
        if any(rel == z or rel.startswith(z + '/') for z in MANDA_MAC):
            if existe(n):
                os.makedirs(os.path.dirname(i), exist_ok=True)
                if not igual(i, n):
                    shutil.copy(n, i)
                    informe['por_adaptar'].append(rel)
            elif existe(i):
                os.remove(i)
                informe['por_adaptar'].append(rel + ' (borrado)')
            continue
        if any(rel == z or rel.startswith(z + '/') for z in FUSION):
            if igual(i, n):
                continue
            if igual(b, i):
                if existe(n):
                    os.makedirs(os.path.dirname(i), exist_ok=True)
                    shutil.copy(n, i)
                    informe['tomado' if existe(b) else 'nuevo'].append(rel)
                else:
                    os.remove(i)
                    informe['borrado'].append(rel)
                continue
            if not existe(i):
                informe['borrado_en_ipad_cambia_en_mac'].append(rel)
                continue
            if not existe(n):
                informe['avisos'].append(f'{rel}: el Mac lo borro y el iPad lo cambio; se queda el del iPad')
                continue
            conflicto = fusionar(i, b, n, tmp, rel.endswith('.rs'))
            informe['conflicto' if conflicto else 'fusion'].append(rel)
            continue
        if any(rel.startswith(z + '/') for z in SOLO_LISTAR):
            informe['pruebas_y_app_del_mac'].append(rel)

    # Los crates de por-adaptar que el iPad no tenia (el Mac no los toco desde
    # la base, asi que no salen en el diff): tambien vienen.
    pa = ruta('nuevo', 'apple/por-adaptar')
    if os.path.isdir(pa):
        for rel in sorted(ficheros(pa)):
            i = os.path.join(IPAD, 'apple/por-adaptar', rel)
            if not existe(i):
                os.makedirs(os.path.dirname(i), exist_ok=True)
                shutil.copy(os.path.join(pa, rel), i)
                informe['por_adaptar'].append(f'apple/por-adaptar/{rel} (no estaba)')

    # Swift de PixPinKit.
    tipos, nombres_fichero = tipos_del_ipad()
    tipos_mac = {}
    kit_mac = ruta('nuevo', KIT)
    for rel in sorted(ficheros(kit_mac)):
        if rel.endswith('.swift'):
            for t in TIPO.findall(open(os.path.join(kit_mac, rel), encoding='utf-8', errors='replace').read()):
                tipos_mac.setdefault(t, f'{KIT}/{rel}')
    for rel in cambiados:
        if not rel.startswith(KIT + '/') or not rel.endswith('.swift'):
            continue
        b, i, n = ruta('base', rel), os.path.join(IPAD, rel), ruta('nuevo', rel)
        if not existe(n):
            if existe(i):
                informe['swift_borrado_en_mac'].append(rel)
            continue
        texto = open(n, encoding='utf-8', errors='replace').read()
        if existe(i) and not existe(b):
            if not igual(i, n):
                informe['swift_a_portar'].append({'fichero': rel, 'nuevo': True,
                                                  'por_que': ['el iPad ya tiene uno suyo con la misma ruta: alinearlo a mano']})
            continue
        if existe(i):
            if igual(i, n):
                continue
            if igual(b, i):
                shutil.copy(n, i)
                informe['swift_fusion'].append(rel + ' (tomado)')
            else:
                conflicto = fusionar(i, b, n, tmp, False)
                informe['swift_conflicto' if conflicto else 'swift_fusion'].append(rel)
            if SOLO_MAC.search(open(i, encoding='utf-8', errors='replace').read()):
                informe['avisos'].append(f'{rel}: tras fusionar importa algo solo del Mac; adaptarlo')
            continue
        porque = []
        m = SOLO_MAC.search(texto)
        if m:
            porque.append(f'importa {m.group(2)}')
        base_nombre = os.path.basename(rel)
        if base_nombre in nombres_fichero:
            porque.append(f'el iPad ya tiene {", ".join(nombres_fichero[base_nombre])}')
        propios = set(TIPO.findall(texto))
        chocan = sorted(t for t in propios if t in tipos)
        faltan = sorted({t for t in re.findall(r'\b[A-Z]\w+\b', texto)
                         if t in tipos_mac and t not in propios and t not in tipos})
        if chocan:
            porque.append('tipos que ya existen en el iPad: ' + ', '.join(f'{t} ({tipos[t]})' for t in chocan))
        if faltan:
            porque.append('usa tipos del Mac que el iPad no tiene: ' + ', '.join(f'{t} ({tipos_mac[t]})' for t in faltan))
        if porque:
            informe['swift_a_portar'].append({'fichero': rel, 'nuevo': not existe(b), 'por_que': porque})
        else:
            os.makedirs(os.path.dirname(i), exist_ok=True)
            shutil.copy(n, i)
            informe['swift_copiado'].append(rel)

    # El Cargo.lock del Mac, ajustado a los crates del iPad.
    if 'apple/Cargo.lock' in informe['tomado']:
        r = subprocess.run(['cargo', 'update', '--workspace', '--offline'], cwd=APPLE, capture_output=True, text=True)
        if r.returncode:
            r = subprocess.run(['cargo', 'update', '--workspace'], cwd=APPLE, capture_output=True, text=True)
        if r.returncode:
            informe['avisos'].append('cargo update --workspace fallo (¿conflictos en Cargo.toml?): ' + r.stderr.strip()[-400:])

    conflictos_rs = [c for c in informe['conflicto']]
    if conflictos_rs or sin_enlaces:
        informe['avisos'].append('Enlaces de Swift sin regenerar: resolver los conflictos y luego (desde apple/) '
                                 'cargo build -p pixpin-nucleo --lib && cargo run -p pixpin-nucleo --features bindgen '
                                 '--bin uniffi-bindgen -- generate --library target/debug/libpixpin_nucleo.so --language swift '
                                 '--out-dir target/enlaces && cp target/enlaces/pixpin_nucleo.swift PixPinCore/Sources/PixPinCore/')
    else:
        so = 'dylib' if sys.platform == 'darwin' else 'so'
        pasos = [['cargo', 'build', '-p', 'pixpin-nucleo', '--lib'],
                 ['cargo', 'run', '-p', 'pixpin-nucleo', '--features', 'bindgen', '--bin', 'uniffi-bindgen', '--', 'generate',
                  '--library', f'target/debug/libpixpin_nucleo.{so}', '--language', 'swift', '--out-dir', 'target/enlaces']]
        for p in pasos:
            if subprocess.run(p, cwd=APPLE).returncode:
                informe['avisos'].append('No se pudieron regenerar los enlaces: ' + ' '.join(p))
                break
        else:
            shutil.copy(os.path.join(APPLE, 'target/enlaces/pixpin_nucleo.swift'),
                         os.path.join(APPLE, 'PixPinCore/Sources/PixPinCore/pixpin_nucleo.swift'))
            informe['avisos'].append('Enlaces de Swift regenerados')

    os.makedirs(os.path.join(APPLE, 'target'), exist_ok=True)
    with open(os.path.join(APPLE, 'target', 'traer-del-mac.json'), 'w') as f:
        json.dump(informe, f, indent=1, ensure_ascii=False)
    with open(APUNTE, 'w') as f:
        f.write(f'{nuevo}\n')
    shutil.rmtree(tmp)
    print(f'Mac {base[:7]} -> {nuevo[:7]} ({pedido})')
    for k, v in informe.items():
        if isinstance(v, list):
            print(f'{k}: {len(v)}')
    for c in informe['conflicto'] + informe['swift_conflicto']:
        print('  CONFLICTO', c)
    for c in informe['borrado_en_ipad_cambia_en_mac']:
        print('  BORRADO EN EL IPAD, CAMBIADO EN EL MAC', c)
    for c in informe['swift_a_portar']:
        print('  PORTAR A MANO', c['fichero'], '-', '; '.join(c['por_que']))
    for a in informe['avisos']:
        print('  AVISO', a)


if __name__ == '__main__':
    main()
