#!/usr/bin/env python3
"""A customer installs GUARDIANA, uses it, stops it, uninstalls it and installs it again.

    python3 build/cliente/instalacion.py --tipo msi|deb|tar|app --dir PAQUETES [--anterior DIR]

Meant for a fresh, throwaway machine (a GitHub runner): it installs services and changes the
system DNS, exactly as a customer's computer would see. Never run it on a computer you use.

What it checks is what the install page and the panel promise, in this order:
  install -> the service runs -> the panel answers (and refuses without its key) -> the trial
  says 7 days -> verify -> the system DNS points at Guardiana -> names resolve through it and
  are written in the extract -> the chain of the extract verifies -> stopping the service does
  not leave the computer without names -> starting it again takes the DNS back -> uninstalling
  leaves the DNS exactly as it was -> installing again does not give the 7 days back.
On Windows also: an update from the previous version leaves one program, not two.

Every check is a GitHub annotation (::notice / ::error), readable on the run page and through the
public API without signing in. Whatever happens, the DNS is put back at the end. Exit 1 if any
check failed. Written on 1 Oct 2026 (.github/workflows/cliente.yml).
"""
import argparse
import glob
import json
import os
import platform
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request

SISTEMA = platform.system()  # Windows, Linux, Darwin
PANEL = "http://127.0.0.1:7443"
SUFIJO = "prueba.guardiana.hogar"
DIA_MS = 24 * 3600 * 1000
fallos = []
# --candidato: packages built by CI from a branch, not released. They carry no signature and are
# not in the public ledger yet; everything else must hold exactly as for a release.
CANDIDATO = False


buenos = []


def limpio(t):
    return str(t).replace("\r", " ").replace("\n", " ")[:900]


def ok(titulo, detalle=""):
    # Plain log line now, and all the good ones in one notice at the end: GitHub keeps only ten
    # notices and ten errors per step, and on the first run the cap hid half of the results.
    buenos.append(f"{titulo}: {limpio(detalle)}" if detalle else titulo)
    print(f"  ok  {titulo} · {limpio(detalle)}", flush=True)


def mal(titulo, detalle=""):
    fallos.append(f"{titulo}: {limpio(detalle)}" if detalle else titulo)
    print(f"::error title=FALLO · {titulo}::{limpio(detalle)}", flush=True)


def nota(texto):
    print(f"--- {texto}", flush=True)


def run(cmd, timeout=180, env=None, sudo=False):
    """(exit code, stdout+stderr). Never raises: a customer's command either works or does not."""
    if sudo and SISTEMA != "Windows":
        cmd = ["sudo", "-E"] + cmd if env else ["sudo"] + cmd
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout,
                           env={**os.environ, **(env or {})}, errors="replace")
        return p.returncode, (p.stdout or "") + (p.stderr or "")
    except subprocess.TimeoutExpired:
        return 124, f"tiempo agotado tras {timeout}s: {' '.join(cmd)}"
    except OSError as e:
        return 127, str(e)


def ps(script, timeout=180):
    return run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script], timeout=timeout)


def esperar(cond, segundos, paso=1.0):
    fin = time.time() + segundos
    while time.time() < fin:
        try:
            if cond():
                return True
        except Exception:  # noqa: BLE001
            pass
        time.sleep(paso)
    try:
        return bool(cond())
    except Exception:  # noqa: BLE001
        return False


# ----- the system resolver, as any program on the machine uses it --------------------------

def vaciar_cache():
    if SISTEMA == "Windows":
        run(["ipconfig", "/flushdns"])
    elif SISTEMA == "Darwin":
        run(["dscacheutil", "-flushcache"])
        run(["killall", "-HUP", "mDNSResponder"], sudo=True)
    else:
        run(["resolvectl", "flush-caches"], sudo=True)


def resuelve(nombre, segundos=10):
    """Addresses the system resolver returns for a name, or None on failure or timeout."""
    out = {}

    def hilo():
        try:
            out["r"] = sorted({a[4][0] for a in socket.getaddrinfo(nombre, 443, proto=socket.IPPROTO_TCP)})
        except OSError as e:
            out["e"] = str(e)

    t = threading.Thread(target=hilo, daemon=True)
    t.start()
    t.join(segundos)
    return out.get("r")


def nombre_prueba():
    return f"g{int(time.time() * 1000)}x{os.getpid()}.{SUFIJO}"


def pasa_por_guardiana():
    r = resuelve(nombre_prueba())
    return bool(r) and all(a in ("127.0.0.1", "::1") for a in r)


def hay_internet():
    vaciar_cache()
    for n in ("github.com", "www.wikipedia.org", "example.com"):
        t0 = time.time()
        if resuelve(n, 10):
            return True, f"{n} en {time.time() - t0:.1f}s"
    return False, "ni github.com, ni wikipedia.org, ni example.com"


# ----- the system DNS settings, per system ---------------------------------------------------

def dns_actual():
    if SISTEMA == "Windows":
        code, out = ps("Get-NetIPInterface -ConnectionState Connected | Where-Object { $_.InterfaceAlias -notlike 'Loopback*' } | "
                       "ForEach-Object { Get-DnsClientServerAddress -InterfaceIndex $_.ifIndex -AddressFamily $_.AddressFamily } | "
                       "Select-Object InterfaceAlias,AddressFamily,ServerAddresses | ConvertTo-Json -Compress")
        try:
            v = json.loads(out) if out.strip() else []
            v = v if isinstance(v, list) else [v]
            return {f"{x['InterfaceAlias']}/{x['AddressFamily']}": list(x.get('ServerAddresses') or []) for x in v}
        except ValueError:
            return {"?": [out.strip()[:200]]}
    if SISTEMA == "Darwin":
        _, lista = run(["networksetup", "-listallnetworkservices"])
        r = {}
        for s in lista.splitlines()[1:]:
            s = s.strip()
            if s and not s.startswith("*"):
                _, o = run(["networksetup", "-getdnsservers", s])
                r[s] = [l.strip() for l in o.splitlines() if l.strip() and " " not in l.strip()]
        return r
    r = {"resolv.conf -> ": [os.path.realpath("/etc/resolv.conf")]}
    try:
        r["resolv.conf"] = [l.strip() for l in open("/etc/resolv.conf") if l.startswith("nameserver")]
    except OSError:
        r["resolv.conf"] = []
    r["drop-ins"] = sorted(glob.glob("/etc/systemd/resolved.conf.d/*guardiana*"))
    return r


def guardiana_primero(d):
    vals = [v for v in d.values() if v]
    if SISTEMA == "Linux":
        return any(l.startswith("nameserver 127.0.0.1") or l.startswith("nameserver ::1") for l in d.get("resolv.conf", [])[:1]) \
            or bool(d.get("drop-ins"))
    return any(v[0] in ("127.0.0.1", "::1") for v in vals)


def restaurar_dns(antes):
    """The last resort, whatever happened: the machine goes back to the DNS it had."""
    if dns_actual() == antes:
        return
    nota("devolviendo el DNS a como estaba antes de la prueba")
    if SISTEMA == "Windows":
        ps("Get-NetAdapter | ForEach-Object { Set-DnsClientServerAddress -InterfaceIndex $_.ifIndex -ResetServerAddresses -ErrorAction SilentlyContinue }")
    elif SISTEMA == "Darwin":
        for s, servers in antes.items():
            run(["networksetup", "-setdnsservers", s] + (servers or ["empty"]), sudo=True)
    else:
        for f in glob.glob("/etc/systemd/resolved.conf.d/*guardiana*"):
            run(["rm", "-f", f], sudo=True)
        destino = antes.get("resolv.conf -> ", [""])[0]
        if destino and destino != "/etc/resolv.conf":
            run(["ln", "-sf", destino, "/etc/resolv.conf"], sudo=True)
        run(["systemctl", "restart", "systemd-resolved"], sudo=True)
    vaciar_cache()


# ----- the installed program -----------------------------------------------------------------

def binario():
    for p in (r"C:\Program Files\Guardiana\guardiana.exe", "/usr/bin/guardiana",
              "/usr/local/bin/guardiana", "/usr/local/guardiana/guardiana"):
        if os.path.exists(p):
            return p
    return None


def datos():
    if SISTEMA == "Windows":
        return os.path.join(os.environ.get("ProgramData", r"C:\ProgramData"), "Guardiana")
    if SISTEMA == "Darwin":
        return "/Library/Application Support/Guardiana"
    return "/var/lib/guardiana"


def cli(*args, timeout=120):
    b = binario()
    if not b:
        return 127, "no hay programa instalado"
    env = {"GUARDIANA_DATA": datos()} if SISTEMA == "Darwin" else None
    return run([b, *args], timeout=timeout, env=env, sudo=SISTEMA != "Windows")


def servicio():
    """running | stopped | absent"""
    if SISTEMA == "Windows":
        code, out = run(["sc.exe", "query", "guardiana"])
        if code != 0:
            return "absent"
        return "running" if "RUNNING" in out else "stopped"
    if SISTEMA == "Darwin":
        if not os.path.exists("/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"):
            return "absent"
        code, _ = run(["pgrep", "-f", "/usr/local/guardiana/guardiana"])
        return "running" if code == 0 else "stopped"
    code, out = run(["systemctl", "is-active", "guardiana"])
    if "inactive" in out or "failed" in out:
        code2, out2 = run(["systemctl", "cat", "guardiana"])
        return "stopped" if code2 == 0 else "absent"
    return "running" if out.strip() == "active" else ("absent" if code == 4 else "stopped")


def parar():
    if SISTEMA == "Windows":
        return ps("Stop-Service guardiana -Force")
    if SISTEMA == "Darwin":
        return run(["launchctl", "bootout", "system", "/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"], sudo=True)
    return run(["systemctl", "stop", "guardiana"], sudo=True)


def arrancar():
    if SISTEMA == "Windows":
        return ps("Start-Service guardiana")
    if SISTEMA == "Darwin":
        return run(["launchctl", "bootstrap", "system", "/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"], sudo=True)
    return run(["systemctl", "start", "guardiana"], sudo=True)


def token():
    p = os.path.join(datos(), "panel.token")
    if SISTEMA == "Linux":
        code, out = run(["cat", p], sudo=True)
        return out.strip() if code == 0 else ""
    try:
        return open(p, encoding="utf-8").read().strip()
    except OSError:
        return ""


def panel(ruta, con_llave=True, post=False):
    req = urllib.request.Request(PANEL + ruta, data=b"{}" if post else None, method="POST" if post else "GET")
    if post:
        req.add_header("Content-Type", "application/json")
    if con_llave:
        req.add_header("x-guardiana-token", token())
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")
    except Exception as e:  # noqa: BLE001
        return 0, str(e)


def duenos_de_puertos():
    """Who listens on 53 and 80 outside the loopback: address, port and program."""
    if SISTEMA == "Windows":
        _, out = ps("$p=@{}; Get-Process | ForEach-Object { $p[$_.Id]=$_.ProcessName }; "
                    "Get-NetTCPConnection -State Listen -LocalPort 53,80 -ErrorAction SilentlyContinue | ForEach-Object { 'tcp ' + $_.LocalAddress + ':' + $_.LocalPort + ' ' + $p[[int]$_.OwningProcess] }; "
                    "Get-NetUDPEndpoint -LocalPort 53 -ErrorAction SilentlyContinue | ForEach-Object { 'udp ' + $_.LocalAddress + ':' + $_.LocalPort + ' ' + $p[[int]$_.OwningProcess] }")
    elif SISTEMA == "Darwin":
        _, out = run(["lsof", "-nP", "-i:53", "-i:80"], sudo=True)
    else:
        _, out = run(["ss", "-lntup", "( sport = :53 or sport = :80 )"], sudo=True)
    return [l.strip() for l in out.splitlines() if l.strip() and "127.0.0.1" not in l and "[::1]" not in l and "::1:" not in l][:12]


def buscar(obj, clave):
    if isinstance(obj, dict):
        if clave in obj:
            return obj[clave]
        for v in obj.values():
            r = buscar(v, clave)
            if r is not None:
                return r
    if isinstance(obj, list):
        for v in obj:
            r = buscar(v, clave)
            if r is not None:
                return r
    return None


def prueba_actual():
    code, body = panel("/api/licencia")
    if code != 200:
        return None, f"/api/licencia respondió {code}"
    try:
        j = json.loads(body)
    except ValueError:
        return None, "respuesta que no es JSON"
    return j, ""


def marca_prueba():
    """(where, value) of the trial mark, or (None, what was looked at)."""
    if SISTEMA == "Windows":
        code, out = run(["reg", "query", r"HKLM\SOFTWARE\Guardiana", "/v", "prueba"])
        return (r"HKLM\SOFTWARE\Guardiana\prueba", out.split()[-1]) if code == 0 and out.split() else (None, "HKLM")
    for p in ("/etc/guardiana/prueba-empezada", os.path.join(datos(), "prueba-empezada")):
        code, out = run(["cat", p], sudo=True)
        if code == 0 and out.strip():
            return p, out.strip()
    return None, "/etc/guardiana/prueba-empezada ni la carpeta de datos"


# ----- install and uninstall, per package ----------------------------------------------------

def instalar(tipo, paquete, carpeta):
    if tipo == "msi":
        log = os.path.join(tempfile.gettempdir(), "guardiana-msi.log")
        code, out = run(["msiexec", "/i", paquete, "/qn", "/norestart", "/l*v", log], timeout=600)
        if code not in (0, 3010):
            try:
                out += "\n" + open(log, encoding="utf-16", errors="replace").read()[-1500:]
            except OSError:
                pass
        return code in (0, 3010), out
    if tipo == "deb":
        code, out = run(["apt-get", "install", "-y", os.path.abspath(paquete)], timeout=600, sudo=True)
        return code == 0, out
    if tipo == "tar":
        code, out = run(["tar", "-xzf", paquete, "-C", carpeta])
        dentro = [d for d in glob.glob(os.path.join(carpeta, "guardiana-*")) if os.path.isdir(d)]
        if code != 0 or not dentro:
            return False, out or "el .tar.gz no trae una carpeta guardiana-*"
        code, out = run(["bash", os.path.join(dentro[0], "instalar.sh")], timeout=600, sudo=True)
        return code == 0, out
    if tipo == "app":
        code, out = run(["ditto", "-x", "-k", paquete, carpeta])
        app = os.path.join(carpeta, "GUARDIANA.app")
        if code != 0 or not os.path.isdir(app):
            return False, out or "el .zip no trae GUARDIANA.app"
        res = os.path.join(app, "Contents", "Resources")
        code, out = run(["bash", os.path.join(res, "instalar.sh"), os.path.join(res, "guardiana"),
                         os.environ.get("USER", "runner"), "en"], timeout=600, sudo=True)
        return code == 0, out
    return False, f"tipo desconocido {tipo}"


def desinstalar(tipo, paquete, carpeta):
    if tipo == "msi":
        code, out = run(["msiexec", "/x", paquete, "/qn", "/norestart"], timeout=600)
        return code in (0, 3010), out
    if tipo == "deb":
        code, out = run(["apt-get", "purge", "-y", "guardiana"], timeout=600, sudo=True)
        return code == 0, out
    if tipo == "tar":
        dentro = [d for d in glob.glob(os.path.join(carpeta, "guardiana-*")) if os.path.isdir(d)]
        code, out = run(["bash", os.path.join(dentro[0], "desinstalar.sh")], timeout=600, sudo=True)
        return code == 0, out
    if tipo == "app":
        res = os.path.join(carpeta, "GUARDIANA.app", "Contents", "Resources")
        code, out = run(["bash", os.path.join(res, "desinstalar.sh"), "en"], timeout=600, sudo=True)
        return code == 0, out
    return False, ""


def paquete_de(tipo, carpeta):
    patron = {"msi": "guardiana-*-windows-x64.msi", "deb": "guardiana_*_amd64.deb",
              "tar": "guardiana-*-linux-x86_64.tar.gz", "app": "guardiana-*-macos.app.zip"}[tipo]
    v = sorted(glob.glob(os.path.join(carpeta, patron)))
    return v[0] if v else None


# ----- the customer's afternoon --------------------------------------------------------------

def tarde(tipo, paquete, version):
    carpeta = tempfile.mkdtemp(prefix="guardiana-cliente-")
    antes = dns_actual()
    nota(f"DNS antes de instalar: {antes}")
    hay, cual = hay_internet()
    if not hay:
        mal("internet antes de empezar", "la máquina de prueba no resuelve nombres: la prueba no vale")
        return

    # 1. Install.
    bien, out = instalar(tipo, paquete, carpeta)
    if not bien:
        mal("instalar", out[-1500:])
        return
    ok("instalar", os.path.basename(paquete))

    if esperar(lambda: servicio() == "running", 40):
        ok("servicio en marcha")
    else:
        mal("servicio en marcha", f"estado: {servicio()}")

    code, out = run([binario() or "guardiana", "--version"])
    if version and version in out:
        ok("versión", out.strip())
    else:
        mal("versión", f"esperaba {version}, dice «{out.strip()}»")

    # 2. The panel, and its key.
    if esperar(lambda: panel("/", con_llave=False)[0] == 200, 30):
        ok("panel", f"{PANEL}/ responde")
    else:
        mal("panel", f"{PANEL}/ no responde")
    code, _ = panel("/api/estado", con_llave=False)
    if code in (401, 403):
        ok("el panel sin llave no enseña nada", f"/api/estado sin llave: {code}")
    else:
        mal("el panel sin llave no enseña nada", f"/api/estado sin llave respondió {code}")
    code, body = panel("/api/estado")
    if code == 200:
        ok("el panel con llave", "/api/estado: 200")
    else:
        mal("el panel con llave", f"/api/estado: {code} {body[:200]}")

    # 3. The trial.
    lic, err = prueba_actual()
    termina = None
    if lic is None:
        mal("prueba de 7 días", err)
    else:
        dias = buscar(lic, "dias_restantes")
        empieza, termina = buscar(lic, "empieza"), buscar(lic, "termina")
        if dias in (6, 7) and empieza and termina and abs((termina - empieza) - 7 * DIA_MS) < 60_000:
            ok("prueba de 7 días", f"quedan {dias} días")
        else:
            mal("prueba de 7 días", f"dias_restantes={dias}, empieza={empieza}, termina={termina}")
        texto = buscar(lic, "texto") or ""
        if "{" in texto or "quien_" in texto:
            mal("texto de la licencia", texto[:300])

    # 4. verify, as the install page tells the customer to run it.
    code, out = cli("verify", "--json")
    try:
        rep = json.loads(out[out.find("{"):])
    except ValueError:
        rep = None
    if rep is None:
        mal("guardiana verify", out[-800:])
    else:
        firma = rep.get("signature")
        if CANDIDATO and firma == "no_signature_file":
            # A candidate is built by CI, which never holds the key (brief §10): no .minisig yet.
            ok("verify · firma del programa: un candidato no va firmado (esperado)", str(firma))
        else:
            (ok if firma == "valid" else mal)("verify · firma del programa", str(firma))
        (ok if rep.get("service") == "running" else mal)("verify · servicio", str(rep.get("service")))
        abiertos = rep.get("lan_ports_open") or []
        if not abiertos:
            ok("verify · ningún puerto abierto a la red")
        else:
            duenos = duenos_de_puertos()
            de_guardiana = [d for d in duenos if "guardiana" in d.lower()]
            if de_guardiana:
                mal("Guardiana no abre puertos a la red con Modo Hogar apagado", "; ".join(de_guardiana))
            else:
                ok("Guardiana no abre puertos a la red con Modo Hogar apagado", "los que hay son de otros programas: " + "; ".join(duenos))
                mal("verify culpa a Guardiana de puertos de otros programas", f"verify lista {abiertos}; sus dueños: {'; '.join(duenos) or 'desconocidos'}")
        (ok if rep.get("chain_ok") in (True, None) else mal)("verify · cadena del extracto", str(rep.get("chain_ok")))
        led = rep.get("ledger")
        if isinstance(led, dict) and "found" in json.dumps(led).lower():
            ok("verify · registro público", json.dumps(led)[:300])
        elif CANDIDATO and (
            json.dumps(led).strip('"') in ("not_found", "no_ledger_file")
            # On the Mac the ledger lists the .zip, which the installed app cannot reproduce; for a
            # version that is not released yet there is no line to find (zip: null).
            or led == {"zip_only": {"zip": None}}
        ):
            # Its hash is written in ledger.jsonl only when it is released.
            ok("verify · registro público: un candidato aún no está en el registro (esperado)", json.dumps(led)[:120])
        else:
            mal("verify · registro público", f"verify dice {json.dumps(led)[:300]}: la web promete que verify repite la comprobación solo")

    # 5. Pointing the computer at Guardiana (the Windows installer already does; elsewhere, the
    #    panel's button, which is the same command).
    if not guardiana_primero(dns_actual()):
        # The panel's button, as the customer presses it: the change is made by the service,
        # with the service's permissions. Run from a root terminal instead, it would succeed
        # where the button fails (the .tar.gz unit's ProtectSystem=full makes /etc read-only
        # for the service, review entry 8), and the test would say yes where the customer
        # gets an error.
        code, body = panel("/api/dns/aplicar", post=True)
        if code == 200 and '"dns_aplicado":true' in body.replace(" ", ""):
            ok("el botón del panel apunta el DNS a Guardiana", body[:200])
        else:
            mal("el botón del panel apunta el DNS a Guardiana", f"{code} {body[:600]}")
            code, out = cli("dns", "--apply", "--yes")
            if code != 0:
                mal("apuntar el DNS desde la terminal", out[-800:])
    if esperar(lambda: guardiana_primero(dns_actual()), 20):
        ok("el DNS del equipo apunta a Guardiana", str(dns_actual()))
    else:
        mal("el DNS del equipo apunta a Guardiana", str(dns_actual()))
    vaciar_cache()
    if esperar(pasa_por_guardiana, 20, 2):
        ok("las consultas del equipo pasan por Guardiana", "un nombre que solo contesta Guardiana")
    else:
        mal("las consultas del equipo pasan por Guardiana", "el nombre de prueba no lo contestó Guardiana")
    hay, cual = hay_internet()
    (ok if hay else mal)("internet con Guardiana", cual)

    vaciar_cache()
    unico = f"guardiana-cliente-{int(time.time())}.example.com"
    resuelve(unico, 8)
    resuelve("example.com", 8)
    if esperar(lambda: "example.com" in panel("/api/extracto?limit=500")[1], 20, 2):
        ok("el extracto anota lo que se pregunta", "example.com aparece en /api/extracto")
    else:
        mal("el extracto anota lo que se pregunta", "example.com no aparece en /api/extracto")

    code, out = cli("ledger", "--check")
    (ok if code == 0 else mal)("comprobar la cadena del extracto", out.strip()[-300:])

    # 6. Stopping the service must not leave the computer without names.
    parar()
    esperar(lambda: servicio() != "running", 20)
    time.sleep(3)
    hay, cual = hay_internet()
    if hay:
        ok("internet con el servicio parado", cual)
    else:
        mal("internet con el servicio parado", f"{cual}. DNS: {dns_actual()}")
        restaurar_dns(antes)
    if guardiana_primero(dns_actual()) and servicio() != "running":
        mal("el DNS no apunta a un servicio parado", str(dns_actual()))

    code, out = arrancar()
    if esperar(lambda: servicio() == "running", 40):
        ok("el servicio vuelve a arrancar")
    else:
        mal("el servicio vuelve a arrancar", f"{servicio()} · {out.strip()[-300:]}")
    esperar(lambda: panel("/", con_llave=False)[0] == 200, 30)
    if esperar(lambda: guardiana_primero(dns_actual()), 30) or cli("dns", "--apply", "--yes")[0] == 0:
        vaciar_cache()
        if esperar(pasa_por_guardiana, 30, 2):
            ok("al arrancar, Guardiana vuelve a ser el DNS", str(dns_actual()))
        else:
            extra = ""
            if SISTEMA == "Darwin":
                extra = run(["tail", "-n", "15", os.path.join(datos(), "guardiana.log")], sudo=True)[1]
            mal("al arrancar, Guardiana vuelve a ser el DNS", f"{dns_actual()} · servicio {servicio()} · {extra[-600:]}")

    # 7. Uninstall: the DNS exactly as it was, and the program gone.
    bien, out = desinstalar(tipo, paquete, carpeta)
    (ok if bien else mal)("desinstalar", out.strip()[-600:])
    esperar(lambda: servicio() == "absent", 30)
    (ok if servicio() == "absent" else mal)("el servicio desaparece", servicio())
    (ok if not binario() or tipo == "msi" and not os.path.exists(r"C:\Program Files\Guardiana\guardiana.exe") else mal)(
        "el programa desaparece", str(binario()))
    despues = dns_actual()
    if despues == antes:
        ok("el DNS queda exactamente como estaba", str(despues))
    else:
        mal("el DNS queda exactamente como estaba", f"antes {antes}, después {despues}")
        restaurar_dns(antes)
    hay, cual = hay_internet()
    (ok if hay else mal)("internet después de desinstalar", cual)
    donde, valor = marca_prueba()
    if not donde:
        mal("la fecha de la prueba se queda, como dice la web", f"no está en {valor}")
    elif SISTEMA != "Windows" and donde != "/etc/guardiana/prueba-empezada":
        mal("la fecha de la prueba está donde dice la web", f"la web dice /etc/guardiana/prueba-empezada; está en {donde}, dentro de la carpeta de datos, y borrarla devuelve los 7 días")
    else:
        ok("la fecha de la prueba se queda, como dice la web", donde)

    # 8. Installing again does not give the 7 days back.
    bien, out = instalar(tipo, paquete, carpeta)
    if not bien:
        mal("reinstalar", out[-800:])
        return
    esperar(lambda: panel("/", con_llave=False)[0] == 200, 40)
    lic2, err = prueba_actual()
    termina2 = buscar(lic2, "termina") if lic2 else None
    if termina and termina2 == termina:
        ok("reinstalar no devuelve los 7 días", f"la prueba sigue terminando en {termina2}")
    else:
        mal("reinstalar no devuelve los 7 días", f"antes terminaba en {termina}, ahora en {termina2} {err}")
    bien, out = desinstalar(tipo, paquete, carpeta)
    (ok if bien else mal)("desinstalar otra vez", out.strip()[-300:])
    restaurar_dns(antes)


def actualizacion(anterior, nuevo, version):
    """Windows: installing 1.0.1 over 1.0.0 leaves one program, the new one."""
    antes = dns_actual()
    bien, out = instalar("msi", anterior, None)
    if not bien:
        mal("actualizar · instalar la versión anterior", out[-600:])
        return
    esperar(lambda: servicio() == "running", 40)
    bien, out = instalar("msi", nuevo, None)
    (ok if bien else mal)("actualizar · instalar encima la nueva", out[-600:])
    code, out = ps("Get-ItemProperty HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\* | "
                   "Where-Object { $_.DisplayName -like '*uardiana*' } | ForEach-Object { $_.DisplayName + ' ' + $_.DisplayVersion }")
    lineas = [l for l in out.splitlines() if l.strip()]
    if len(lineas) == 1 and version in lineas[0]:
        ok("actualizar · queda un solo programa, el nuevo", lineas[0])
    else:
        mal("actualizar · queda un solo programa, el nuevo", " | ".join(lineas) or "ninguno")
    esperar(lambda: servicio() == "running", 40)
    (ok if servicio() == "running" else mal)("actualizar · el servicio sigue en marcha", servicio())
    desinstalar("msi", nuevo, None)
    restaurar_dns(antes)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tipo", required=True, choices=["msi", "deb", "tar", "app"])
    ap.add_argument("--dir", required=True)
    ap.add_argument("--anterior", help="carpeta con el MSI de la versión anterior (solo Windows)")
    ap.add_argument("--candidato", action="store_true", help="paquetes sin publicar: sin firma ni registro todavía")
    a = ap.parse_args()
    global CANDIDATO
    CANDIDATO = a.candidato
    paquete = paquete_de(a.tipo, a.dir)
    if not paquete:
        mal("paquete", f"no hay paquete {a.tipo} en {a.dir}")
        return 1
    version = os.path.basename(paquete).replace("guardiana_", "guardiana-").split("-")[1].split("_")[0]
    nota(f"{SISTEMA} {platform.release()} · {os.path.basename(paquete)} · versión {version}")
    antes = dns_actual()
    try:
        tarde(a.tipo, paquete, version)
        if a.tipo == "msi" and a.anterior:
            viejo = paquete_de("msi", a.anterior)
            if viejo:
                actualizacion(viejo, paquete, version)
            else:
                # The update from the version people have is a promise too: when its installer
                # could not be fetched, the run says so instead of skipping it in silence
                # (review of 9 Oct 2026).
                mal("actualización", f"no hay instalador anterior en {a.anterior}: la actualización no se probó")
    finally:
        restaurar_dns(antes)
    if buenos:
        print("::notice title=%s · lo que está bien (%d)::%s" % (SISTEMA, len(buenos), "%0A".join(buenos)), flush=True)
    if fallos:
        print("::error title=%s · todos los fallos (%d)::%s" % (SISTEMA, len(fallos), "%0A".join(fallos)), flush=True)
    print(f"\n{len(fallos)} fallos" + (":\n  - " + "\n  - ".join(fallos) if fallos else ""), flush=True)
    return 1 if fallos else 0


if __name__ == "__main__":
    sys.exit(main())
