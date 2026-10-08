#!/usr/bin/env python3
"""Regenerates crates/lists/data/corredores.txt from the CSV of California's data broker registry.

    python build/corredores.py registry.csv            # rewrites the list; prints what changed
    python build/corredores.py registry.csv salida.txt # writes somewhere else, to compare

The CSV is downloaded by hand from https://cppa.ca.gov/data_broker_registry/ (the program never
downloads anything on its own, brief section 10). Everything in the list comes from the CSV except
the curated lines below (`EXTRA`, `NEW_SECTIONS`), which name the tracking domains each registrant
uses and the `empresas.txt` sections that are registrants; those are maintained here, by hand, and
each one says whose it is. Decision 194.
"""
import csv
import datetime
import hashlib
import os
import re
import sys
import urllib.parse

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SALIDA = os.path.join(RAIZ, 'crates', 'lists', 'data', 'corredores.txt')

# Column indexes of registry.csv (header of October 2026): name, DBA, website, country, the page
# for consumer rights, and the yes/no questions the registrant answers about itself.
COL = dict(nombre=0, dba=1, web=2, pais=9, derechos=10, menores=11, biometria=18, ubicacion=19,
           extranjero=21, federal=22, policia=24, ia=25)
LETRAS = [('menores', 'm'), ('ubicacion', 'g'), ('biometria', 'b'), ('extranjero', 'x'),
          ('federal', 'f'), ('policia', 'p'), ('ia', 'i')]
PAISES = {'UNITED STATES': 'US', 'GERMANY': 'DE', 'SINGAPORE': 'SG', 'NEW ZEALAND': 'NZ',
          'UNITED KINGDOM': 'GB', 'CANADA': 'CA', 'FINLAND': 'FI', 'ISRAEL': 'IL',
          'UNITED ARAB EMIRATES': 'AE', 'NORWAY': 'NO', 'SPAIN': 'ES', 'FRANCE': 'FR',
          'SWEDEN': 'SE', 'INDIA': 'IN', 'POLAND': 'PL', 'AUSTRALIA': 'AU'}
# A DBA that is not a name.
BASURA = {'', 'na', 'no', 'none', 'yes', 'not applicable', 'n/a'}
# When several registrants declare the same website, the one whose name carries the domain keeps
# it; these settle the ones where no name does.
DUENO = {'leadfeeder.com': 'Dealfront Group GmbH', 'transunion.com': 'Trans Union LLC',
         'nielsen.com': 'The Nielsen Company, LLC'}
# Shown name for a registrant whose own name is not the one people know.
NOMBRE = {'Trans Union LLC': 'TransUnion', 'The Nielsen Company, LLC': 'The Nielsen Company, LLC',
          'eXelate, Inc.': 'eXelate, Inc.'}

# Registrants that lost every domain to a sister company (same website) but have a tracking
# domain of their own, as new sections; `=Empresa` means that section of empresas.txt is it.
NEW_SECTIONS = {
    'Tapad': ['tapad.com  # Tapad: el identificador «cross-device» de Experian'],
    'iovation Inc.': ['iovation.com', 'iesnare.com  # la huella de dispositivo de iovation (TransUnion)'],
    'Neustar Information Services, Inc.': ['agkn.com  # AdAdvisor, de Neustar (TransUnion)'],
    'eXelate, Inc.': ['exelator.com  # eXelate, de Nielsen'],
    'PlaceIQ, Inc.': ['placeiq.com'],
    'Chartboost, LLC': ['=Chartboost'],
}
# Curated lines per registrant (by its shown name): tracking domains the registry's "primary
# website" column does not list, each with whose it is, or `=Empresa` for a section of
# empresas.txt that is the registrant or its group.
EXTRA = {
    '33Across, Inc.': ['tynt.com  # Tynt, de 33Across desde 2013'],
    '6sense Insights, Inc.': ['6sc.co'],
    'Acxiom LLC': ['acxiom-online.com'],
    'AppLovin Corporation': ['=AppLovin'],
    'Audigent': ['ad.gt'],
    'Beeswax': ['bidr.io'],
    'Bombora, Inc.': ['ml314.com'],
    'Comscore Inc': ['=Comscore'],
    'Criteo Corp.': ['=Criteo'],
    'Datonics LLC': ['pro-market.net'],
    'Demandbase, Inc.': ['company-target.com'],
    'Disqus': ['=Disqus'],
    'Dstillery, Inc.': ['media6degrees.com  # Dstillery se llamaba Media6Degrees'],
    'Epsilon Data Management, LLC': ['conversantmedia.com  # Conversant, de Epsilon', 'dotomi.com',
                                     'mediaplex.com', 'fastclick.net  # ValueClick, que pasó a ser Conversant'],
    'Eyeota Pte Ltd': ['eyeota.net'],
    'Fetch Rewards, LLC': ['fetchrewards.com'],
    'Foursquare Labs, Inc.': ['placed.com  # Placed, de Foursquare desde 2019',
                              'factual.com  # Factual, unida a Foursquare en 2020'],
    'FreeWheel Media Inc': ['=FreeWheel'],
    'GroundTruth': ['xad.com  # GroundTruth se llamaba xAd'],
    'HubSpot, Inc.': ['=HubSpot'],
    'ID5': ['=ID5'],
    'Index Exchange Inc.': ['=Index Exchange'],
    'LexisNexis Risk Solutions FL Inc.': ['threatmetrix.com  # ThreatMetrix, de LexisNexis Risk desde 2018',
                                          'online-metrix.net  # la huella de dispositivo de ThreatMetrix'],
    'LiveIntent': ['liadm.com'],
    'LiveRamp Holdings, Inc.': ['=LiveRamp'],
    'LoopMe Limited': ['loopme.me'],
    'Magnite Inc': ['=Magnite'],
    'Merkle Inc.': ['merkleinc.com', 'rkdms.com'],
    'Nexxen Inc': ['unrulymedia.com  # Unruly, de Nexxen', 'tremorhub.com',
                   'amobee.com  # Amobee, de Nexxen desde 2022', 'turn.com  # Turn, de Amobee desde 2017'],
    'NextRoll Inc': ['=NextRoll'],
    'The Nielsen Company, LLC': ['=Nielsen'],
    'Ogury Ltd': ['=Ogury'],
    'OpenX Technologies, Inc.': ['=OpenX'],
    'PubMatic Inc.': ['=PubMatic'],
    'PulsePoint': ['contextweb.com  # PulsePoint se llamaba ContextWeb'],
    'Resonate': ['reson8.com'],
    'Semasio': ['semasio.net'],
    'Sovrn, Inc.': ['lijit.com'],
    'Taboola, Inc.': ['=Taboola'],
    'Teads Holding Co': ['=Outbrain  # Outbrain pasó a llamarse Teads en 2025'],
    'Teads Inc': ['=Teads'],
    'TripleLift, Inc.': ['=TripleLift'],
    'Viant US LLC': ['adelphic.com  # Adelphic, el DSP de Viant'],
    'Wunderkind': ['bounceexchange.com  # Wunderkind se llamaba BounceX'],
    'Zeta Global': ['zetaglobal.net', 'rezync.com'],
}

CABECERA = """# Guardiana — lista "corredores": empresas registradas como corredoras de datos (data brokers).
#
# Un corredor de datos, según la ley de California (Civil Code 1798.99.80), es una empresa que
# recoge y vende a terceros datos personales de personas con las que no tiene relación directa.
# Desde 2024 todas tienen que inscribirse en el registro público de la Agencia de Protección de
# la Privacidad de California (CPPA), que es la fuente de este archivo:
#
#   https://cppa.ca.gov/data_broker_registry/   (archivo registry.csv)
#   Copia del {fecha}, {n} inscripciones,
#   SHA-256 del CSV: {sha}
#
# Es una etiqueta, nunca un veredicto: el panel dice «esta empresa está inscrita como corredora
# de datos» y de dónde es, y la persona decide si la corta. Nada de lo que hay aquí lo inventa
# Guardiana: el nombre, el país, lo que declara y la página de derechos vienen del registro; los
# dominios de rastreo añadidos a mano llevan al lado de quién son y desde cuándo.
#
# FORMATO: `@Nombre|XX|declara|url`
#   Nombre   el que figura en el registro (su marca comercial si la declaró; si no, la razón social)
#   XX       país de la inscrita, código ISO de dos letras, tal como lo escribió ella en el registro
#   declara  letras de lo que la propia empresa marcó en el registro:
#              m  recoge datos de menores
#              g  recoge la ubicación exacta
#              b  recoge datos biométricos
#              x  vendió o compartió datos con actores extranjeros el último año
#              f  vendió o compartió datos con el Gobierno federal de EE. UU. el último año
#              p  vendió o compartió datos con la policía sin orden judicial el último año
#              i  vendió o compartió datos con desarrolladores de IA generativa el último año
#   url      la página que ella misma declaró para pedir que borren o no vendan tus datos
# Debajo, una línea por dominio (coincide con el dominio y sus subdominios): primero el sitio
# principal que declaró en el registro; después, con comentario, los nombres de rastreo que usa y
# que el registro no pide. Una línea `=Empresa` dice que esa sección de empresas.txt ES la
# inscrita (o su grupo): todos los nombres de esa empresa cuentan como suyos.
#
# Varias filiales del mismo grupo se inscriben con la misma web (TransUnion, Equifax, Experian,
# Deloitte, Nielsen…): el dominio va con la inscrita cuyo nombre lo lleva, y las demás no salen.
# Si el registro cambia, se vuelve a generar con build/corredores.py desde el CSV nuevo, que
# anota aquí la fecha y la huella.
"""

MESES = 'enero febrero marzo abril mayo junio julio agosto septiembre octubre noviembre diciembre'.split()


def norm(s):
    return re.sub(r'[^a-z0-9]', '', s.lower())


def dominios(celda):
    vistos = []
    for d in re.split(r'[;\s]+', urllib.parse.unquote(celda or '')):
        d = d.strip().lower()
        d = re.sub(r'^https?://', '', d)
        d = re.sub(r'/.*$', '', d)
        d = re.sub(r'^www\.', '', d)
        if '.' in d and re.fullmatch(r'[a-z0-9.-]+', d) and d not in vistos:
            vistos.append(d)
    return vistos


def leer(ruta):
    with open(ruta, newline='', encoding='utf-8-sig') as f:
        filas = list(csv.reader(f))
    inscritas = []
    for r in filas[1:]:
        if not r or not r[COL['nombre']].strip():
            continue
        legal = ' '.join(r[COL['nombre']].split())
        dba = ' '.join((r[COL['dba']] or '').split())
        nombre = dba if (dba.lower() not in BASURA and not re.search(r'[;,]', dba)
                         and dba.lower() != legal.lower()) else legal
        nombre = NOMBRE.get(legal, nombre)
        pais = (r[COL['pais']] or '').strip().upper()
        if pais and pais not in PAISES:
            sys.exit('país sin código ISO en este guion: %s (%s)' % (pais, legal))
        letras = ''.join(l for campo, l in LETRAS if (r[COL[campo]] or '').strip() == 'Yes')
        url = ((r[COL['derechos']] or '').strip().split() or [''])[0]
        if url and not re.match(r'^https?://', url, re.I):
            url = 'https://' + url
        if not re.match(r'^https?://[^\s|#]+\.[^\s|#]+', url, re.I) or '#' in url:
            url = ''
        url = url.rstrip(';.,')
        inscritas.append(dict(legal=legal, nombre=nombre, dominios=dominios(r[COL['web']]),
                              pais=PAISES.get(pais, ''), letras=letras, url=url))
    return inscritas


def asignar(inscritas):
    """Which registrant keeps each website when several declared the same one."""
    por = {}
    for i, g in enumerate(inscritas):
        for d in g['dominios']:
            por.setdefault(d, []).append(i)
    dueno = {}
    for d, cands in por.items():
        elegido = cands[0]
        if d in DUENO:
            elegido = next(i for i in cands if inscritas[i]['legal'] == DUENO[d])
        elif len(cands) > 1:
            raiz = norm(d.split('.')[0])
            for i in cands:
                if raiz in norm(inscritas[i]['nombre']) or raiz in norm(inscritas[i]['legal']):
                    elegido = i
                    break
        dueno[d] = elegido
    return dueno


def generar(ruta_csv):
    inscritas = leer(ruta_csv)
    dueno = asignar(inscritas)
    extra = dict(EXTRA)
    lineas = []
    for i, g in enumerate(inscritas):
        propios = [d for d in g['dominios'] if dueno[d] == i]
        if not propios:
            continue
        lineas.append('')
        lineas.append('@' + '|'.join([g['nombre'], g['pais'], g['letras'], g['url']]))
        lineas.extend(propios)
        lineas.extend(extra.pop(g['nombre'], []))
    por_legal = {g['legal']: g for g in inscritas}
    for legal, doms in NEW_SECTIONS.items():
        g = por_legal.get(legal)
        if g is None:
            sys.exit('ya no está en el registro: %s' % legal)
        lineas.append('')
        lineas.append('@' + '|'.join([g['nombre'], g['pais'], g['letras'], g['url']]))
        lineas.extend(doms)
    if extra:
        sys.exit('líneas a mano para inscritas que ya no están en el registro: %s' % sorted(extra))
    raw = open(ruta_csv, 'rb').read()
    hoy = datetime.date.today()
    cabecera = CABECERA.format(fecha='%d de %s de %d' % (hoy.day, MESES[hoy.month - 1], hoy.year),
                               n=len(inscritas), sha=hashlib.sha256(raw).hexdigest())
    return cabecera + '\n'.join(lineas) + '\n'


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    salida = sys.argv[2] if len(sys.argv) == 3 else SALIDA
    texto = generar(sys.argv[1])
    antes = open(SALIDA, encoding='utf-8').read() if os.path.exists(SALIDA) else ''
    with open(salida, 'w', encoding='utf-8', newline='\n') as f:
        f.write(texto)
    a = [l for l in antes.splitlines() if not l.startswith('#')]
    b = [l for l in texto.splitlines() if not l.startswith('#')]
    print('%s: %d secciones, %d líneas; %+d líneas respecto a la copia anterior'
          % (salida, sum(1 for l in b if l.startswith('@')), len(b), len(b) - len(a)))
    return 0


if __name__ == '__main__':
    sys.exit(main())
