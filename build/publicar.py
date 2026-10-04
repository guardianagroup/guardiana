#!/usr/bin/env python3
"""Publishes the public code and the website from this working repository, on any machine.

    python build/publicar.py codigo            # prepares the public code repo and shows the changes
    python build/publicar.py web               # prepares the website repo and shows the changes
    python build/publicar.py codigo --empujar --mensaje "Why this change"   # ...and publishes it
    python build/publicar.py codigo --empujar --rama candidato-2026-10-05 --mensaje "..."

The same steps as the recipe in docs/PUBLICAR.md, which used rsync and therefore only ran on the
Mac; the Mac was sold on 5 Oct 2026 and the Windows PC has no rsync. Nothing leaves the machine
without --empujar: by default it clones, copies, and stops at `git status`, so whoever publishes
reads the list first (no DECISIONES, PRUEBAS, WEB, PLAN-*, EMPRESA-*, mail or PDF must ever be in
it). The clones are made with core.autocrlf=false, so files are compared byte for byte whatever
Git for Windows is set to.
"""
import argparse
import fnmatch
import os
import shutil
import subprocess
import sys
import tempfile

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SSH = os.path.join(os.path.expanduser('~'), '.ssh')
DESTINOS = {
    'codigo': ('git@github.com:guardianagroup/guardiana.git', 'guardiana_code', 'repo-publico'),
    'web': ('git@github.com:guardianagroup/guardianagroup.com.git', 'guardiana_github', 'web-repo'),
}
BASURA = {'.DS_Store', '__pycache__', 'Thumbs.db'}
RAIZ_CODIGO = ['Cargo.toml', 'Cargo.lock', 'deny.toml', 'LICENSE', 'README.md', 'README.en.md',
               'README.pt.md', 'ledger.jsonl', 'rustfmt.toml', '.gitignore', '.gitattributes']
DOCS_CODIGO = ['BETA', 'BRIEF', 'HOGAR', 'LISTS', 'SEGURIDAD', 'THREAT_MODEL', 'VERIFY',
               'WHAT_IT_DOES_NOT_DO']


def git(args, cwd=None, llave=None, check=True):
    env = dict(os.environ)
    if llave:
        # Forward slashes: git hands this line to a POSIX shell, even on Windows, where a
        # backslash would be read as an escape.
        env['GIT_SSH_COMMAND'] = 'ssh -i "%s" -o IdentitiesOnly=yes' % os.path.join(SSH, llave).replace('\\', '/')
    r = subprocess.run(['git', '-c', 'core.autocrlf=false', '-c', 'core.safecrlf=false'] + args,
                       cwd=cwd, env=env, capture_output=True, text=True, encoding='utf-8', errors='replace')
    if check and r.returncode != 0:
        sys.exit('git %s: %s' % (' '.join(args), (r.stderr or r.stdout).strip()))
    return r.stdout


def es_basura(nombre):
    return nombre in BASURA or nombre.endswith('.pyc')


def copiar(origen, destino, borrar=False, excluir=lambda rel: False):
    """rsync -a origen/ destino/ [--delete], with an exclusion function on the relative path."""
    n = 0
    vistos = set()
    for dirpath, dirs, files in os.walk(origen):
        relDir = os.path.relpath(dirpath, origen)
        relDir = '' if relDir == '.' else relDir.replace(os.sep, '/')
        dirs[:] = [d for d in dirs if not es_basura(d) and not excluir((relDir + '/' + d).lstrip('/'))]
        for d in dirs:
            vistos.add((relDir + '/' + d).lstrip('/'))
        for f in files:
            rel = (relDir + '/' + f).lstrip('/')
            if es_basura(f) or excluir(rel):
                continue
            vistos.add(rel)
            a = os.path.join(origen, rel)
            b = os.path.join(destino, rel)
            if os.path.islink(a):
                continue
            if os.path.exists(b) and os.path.getsize(a) == os.path.getsize(b) and open(a, 'rb').read() == open(b, 'rb').read():
                continue
            os.makedirs(os.path.dirname(b), exist_ok=True)
            shutil.copyfile(a, b)
            n += 1
    if borrar and os.path.isdir(destino):
        for dirpath, dirs, files in os.walk(destino, topdown=False):
            relDir = os.path.relpath(dirpath, destino)
            relDir = '' if relDir == '.' else relDir.replace(os.sep, '/')
            if relDir.split('/')[0] == '.git':
                continue
            for f in files:
                rel = (relDir + '/' + f).lstrip('/')
                if rel not in vistos:
                    os.remove(os.path.join(destino, rel))
                    n += 1
            if relDir and relDir not in vistos and not os.listdir(dirpath):
                os.rmdir(dirpath)
    return n


def preparar_codigo(clon):
    c = 0
    c += copiar(os.path.join(RAIZ, 'crates'), os.path.join(clon, 'crates'), borrar=True,
                excluir=lambda rel: rel.split('/')[-1] == 'target')
    # build/lista (the Listmonk kit) is not part of the program; build/out is output.
    c += copiar(os.path.join(RAIZ, 'build'), os.path.join(clon, 'build'),
                excluir=lambda rel: 'lista' in rel.split('/') or rel.split('/')[0] == 'out')
    # build/mac/dns-suelto.sh repaired the development Mac and serves nobody else.
    for quitar in ('build/mac/dns-suelto.sh', 'docs/BOVEDA.md'):
        p = os.path.join(clon, quitar)
        if os.path.exists(p):
            os.remove(p)
            c += 1
    c += copiar(os.path.join(RAIZ, '.github'), os.path.join(clon, '.github'))
    for f in RAIZ_CODIGO:
        a = os.path.join(RAIZ, f)
        if os.path.exists(a):
            b = os.path.join(clon, f)
            if not (os.path.exists(b) and open(a, 'rb').read() == open(b, 'rb').read()):
                shutil.copyfile(a, b)
                c += 1
    os.makedirs(os.path.join(clon, 'docs'), exist_ok=True)
    for d in DOCS_CODIGO:
        a = os.path.join(RAIZ, 'docs', d + '.md')
        b = os.path.join(clon, 'docs', d + '.md')
        if not (os.path.exists(b) and open(a, 'rb').read() == open(b, 'rb').read()):
            shutil.copyfile(a, b)
            c += 1
    c += copiar(os.path.join(RAIZ, 'docs', 'guias'), os.path.join(clon, 'docs', 'guias'), borrar=True)
    return c


def preparar_web(clon):
    def excluir(rel):
        partes = rel.split('/')
        if 'src' in partes or '.git' in partes:
            return True
        if len(partes) >= 2 and partes[-2] == 'pdf' and fnmatch.fnmatch(partes[-1], '*.py'):
            return True
        return partes[-2:] == ['radiografias', 'radiografias.json']
    # Never deletes: a page that is retired is removed from the website repo by hand, on purpose.
    return copiar(os.path.join(RAIZ, 'site'), clon, excluir=excluir)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('que', choices=sorted(DESTINOS))
    ap.add_argument('--empujar', action='store_true', help='commit and push (without it, nothing leaves the machine)')
    ap.add_argument('--mensaje', help='commit message: the why, as in every commit')
    ap.add_argument('--rama', default='main', help='branch to push to (main by default)')
    ap.add_argument('--clon', help='folder for the clone (by default one in the temporary folder)')
    a = ap.parse_args()
    url, llave, nombre = DESTINOS[a.que]
    if not os.path.exists(os.path.join(SSH, llave)):
        sys.exit('No está la llave %s en %s.' % (llave, SSH))
    if a.empujar and not a.mensaje:
        sys.exit('Para publicar hace falta --mensaje con el porqué.')
    clon = a.clon or os.path.join(tempfile.gettempdir(), nombre)
    if os.path.exists(clon):
        shutil.rmtree(clon, onerror=lambda f, p, e: (os.chmod(p, 0o700), f(p)))
    print('clonando %s ...' % url, flush=True)
    git(['clone', '-q', url, clon], llave=llave)
    git(['config', 'core.autocrlf', 'false'], cwd=clon)
    n = preparar_codigo(clon) if a.que == 'codigo' else preparar_web(clon)
    git(['add', '-A'], cwd=clon)
    estado = git(['status', '--short'], cwd=clon)
    lineas = [l for l in estado.splitlines() if l.strip()]
    print('%d archivos copiados o quitados; %d cambios para git:' % (n, len(lineas)))
    for l in lineas:
        print('   ' + l)
    if not lineas:
        print('Nada que publicar: el repositorio público ya está igual.')
        return 0
    if not a.empujar:
        print('\nNo se ha publicado nada. Si la lista es la esperada:\n  python build/publicar.py %s --empujar --mensaje "..."%s'
              % (a.que, '' if a.rama == 'main' else ' --rama ' + a.rama))
        return 0
    yo = git(['config', 'user.name'], cwd=RAIZ, check=False).strip()
    correo = git(['config', 'user.email'], cwd=RAIZ, check=False).strip()
    if not yo or not correo:
        sys.exit('Falta la identidad de git (git config --global user.name / user.email).')
    git(['-c', 'user.name=' + yo, '-c', 'user.email=' + correo, 'commit', '-q', '-m', a.mensaje], cwd=clon)
    git(['push', '-q', 'origin', 'HEAD:refs/heads/' + a.rama], cwd=clon, llave=llave)
    print('Publicado en %s, rama %s: %s' % (url, a.rama, git(['rev-parse', '--short', 'HEAD'], cwd=clon).strip()))
    if a.que == 'web':
        print('GitHub Pages tarda uno o dos minutos: comprueba la portada en vivo antes de darlo por publicado.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
