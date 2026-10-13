# «Haz que te borren»: la base legal, país por país (decisión 204)

Comprobado el **11 de octubre de 2026**. Esta es la tabla que hay detrás de la página «Haz que te
borren» del panel (`crates/panel/src/borrado.rs`, textos en `crates/panel/i18n/*.json`, empresas en
`crates/panel/data/borrado-corredores.txt`). Cada fila dice qué derecho hay, qué artículo se cita en
la carta, cuánto tiempo tiene la empresa para responder, a quién se reclama y de dónde sale cada
dato. Lo que no se pudo comprobar en una fuente se dice en la columna de notas y en la sección
final; no se rellena de memoria.

**No somos abogados.** Las cartas son plantillas basadas en la ley citada. La tabla resume, no
sustituye, el texto de cada ley.

## Cómo leer la tabla

- **Derecho**: *borrado* = la ley da derecho a que borren tus datos (con sus condiciones);
  *limitado* = no hay un derecho general de borrado, pero sí acceso, corrección, retirar el
  consentimiento o borrar en casos concretos; *ninguno* = no hay una ley general aplicable.
- **Plazo**: el de la ley para responder. «hábiles» = días hábiles; el panel los cuenta de lunes a
  viernes y avisa de que no conoce los festivos.
- **Prórroga**: la ampliación que la ley permite, normalmente avisando dentro del primer plazo.
- Código: el que usa el panel (`borrado_*_<código>` en los textos).

## Europa

| Código | Lugar | Ley y artículo citado | Plazo | Prórroga | Derecho | Autoridad | Fuentes |
|---|---|---|---|---|---|---|---|
| `es` | España | RGPD art. 17 y LOPDGDD (LO 3/2018) art. 15 | 1 mes (RGPD art. 12.3) | 2 meses | borrado | AEPD, sede electrónica: https://sedeagpd.gob.es/sede-electronica-web/vistas/infoSede/tramitesCiudadano.jsf | Formulario oficial de la Guardia Civil que cita «artículo 17 del RGPD» y «artículo 15 de la LO 3/2018»: https://web.guardiacivil.es/export/sites/guardiaCivil/documentos/pdfs/registro_actividades_tratamiento/formularios/EJERCICIO-DERECHO-DE-SUPRESION_.pdf · Cómo reclamar y enlace a la sede de la AEPD (Govern de les Illes Balears): https://caib.es/sites/protecciodedadespersonals/es/camo_puedo_reclamar/ |
| `pt` | Portugal | RGPD art. 17; Lei n.º 58/2019 ejecuta el RGPD | 1 mes | 2 meses | borrado | CNPD: https://www.cnpd.pt (formularios de «participação») | https://www.legiscope.com/blog/cnpd-guia-autoridade.html (secundaria: no se abrió la web de la CNPD) |
| `ie` | Irlanda | RGPD art. 17 | 1 mes | 2 meses | borrado | Data Protection Commission, formulario: https://forms.dataprotection.ie/contact | https://www.dataprotection.ie/en/faqs/initial-contact-dpc/making-complaint-dpc |
| `ue` | Resto de la UE y EEE | RGPD art. 17 | 1 mes | 2 meses | borrado | La autoridad de cada país, lista del CEPD: https://www.edpb.europa.eu/about-edpb/about-edpb/members_en | Texto del RGPD: https://eur-lex.europa.eu/eli/reg/2016/679/oj |
| `gb` | Reino Unido | UK GDPR art. 17 (y Data Protection Act 2018) | 1 mes natural | hasta 2 meses | borrado | ICO: https://ico.org.uk/make-a-complaint/ | https://ico.org.uk/for-the-public/your-right-to-get-your-data-deleted · art. 12(3): https://www.legislation.gov.uk/eur/2016/679/article/12/adopted/data.htm · Data (Use and Access) Act 2025, quejas a la empresa desde el 19 jun 2026: https://www.privacyworld.blog/2026/06/the-data-use-and-access-act-2025-and-the-new-right-for-individuals-to-complain-to-controllers-what-organisations-need-to-do-before-19-june-2026/ |

Notas de Europa:
- El plazo de un mes y la prórroga de dos se leyeron en el art. 12(3) del texto que publica
  legislation.gov.uk (la versión adoptada del Reglamento 2016/679, igual en la UE). La página del
  CEPD con el resumen en portugués dio error al abrirla.
- AEPD: la página del Govern balear no exige esperar antes de reclamar; la práctica de pedirlo
  primero a la empresa y esperar el mes es la que recoge la carta.
- ICO: pide quejarse primero a la empresa. Desde el 19 de junio de 2026 (DUAA 2025) las empresas
  deben acusar recibo de una queja en 30 días. No se encontró que la DUAA cambie el mes del art. 12.
- DPC: no exige un paso previo, pero pide copia de la correspondencia con la empresa, con fechas.
- España: en ficheros de morosos (ASNEF, Badexcug) una deuda cierta puede conservarse un tiempo
  aunque se pida el borrado; el panel lo dice sin citar el artículo concreto.

## Estados Unidos

| Código | Lugar | Ley y sección citada | Plazo | Prórroga | Derecho | Autoridad | Fuentes |
|---|---|---|---|---|---|---|---|
| `us_ca` | California | CCPA/CPRA, Civil Code §1798.105 | 45 días (§1798.130(a)(2)) | 45 días | borrado (solo lo «collected from the consumer») | CalPrivacy (CPPA): https://privacy.ca.gov/submit-a-complaint/ | §1798.105(a): https://privacy.gtlaw.com/consumers-right-to-delete-personal-information/ · §1798.130: https://california.public.law/codes/civil_code_section_1798.130 · DROP: https://privacy.ca.gov/drop/ · https://privacy.ca.gov/data-brokers |
| `us_va` | Virginia | VCDPA, Va. Code §59.1-577(A)(3) | 45 días (§59.1-577 B) | 45 días | borrado (incluye lo obtenido de otros) | Attorney General: https://www.oag.state.va.us | https://law.lis.virginia.gov/vacode/59.1-577 |
| `us_co` | Colorado | CPA, C.R.S. §6-1-1306(1)(d) | 45 días (§6-1-1306(2)) | 45 días | borrado | Attorney General: https://complaints.coag.gov/s/data-privacy | https://privacy.gtlaw.com/colorado/ · https://coag.gov/resources/data-privacy/ |
| `us_ct` | Connecticut | CTDPA, Conn. Gen. Stat. §42-518(a)(3) | 45 días (§42-518(c)) | 45 días | borrado | Attorney General: https://portal.ct.gov/ag/common/complaint-form-landing-page | https://www.recordinglaw.com/us-laws/data-privacy-laws/connecticut-data-privacy-laws/ctdpa-consumer-rights/ (secundaria con el texto citado) |
| `us_ut` | Utah | UCPA, Utah Code §13-61-201(2) | 45 días (§13-61-203(2)) | 45 días | borrado **solo de lo que tú diste** | Division of Consumer Protection: https://dcp.utah.gov | https://le.utah.gov/xcode/Title13/Chapter61/C13-61-S201_2026070120250507.html · https://privacy.gtlaw.com/?p=1399 |
| `us_tx` | Texas | TDPSA, Bus. & Com. Code §541.051(b)(3) | 45 días (§541.052(b)) | 45 días | borrado | Attorney General: https://www.texasattorneygeneral.gov/consumer-protection/file-consumer-complaint | https://texas.public.law/statutes/tex._bus._&_com._code_section_541.051 · …541.052 |
| `us_or` | Oregón | OCPA, ORS §646A.574(1)(c) | 45 días (ORS §646A.576(5)(a)) | 45 días | borrado (incluye lo obtenido de otros) | Department of Justice: https://doj.state.or.us/consumer-protection/id-theft-data-breaches | https://oregon.public.law/statutes/ors_646a.574 · https://oregon.public.law/statutes/ors_646a.576 |
| `us_otros` | Otro estado con ley | «la ley de privacidad de mi estado» | 45 días (**referencia**) | 45 días | borrado (no comprobado estado a estado) | Attorney General del estado (sin enlace) | Lista de leyes y fechas: https://www.clym.io/blog/us-privacy-law-comparison-map · https://www.multistate.us/insider/2026/2/4/all-of-the-comprehensive-privacy-laws-that-take-effect-in-2026 |
| `us` | Sin ley estatal | ninguna ley federal general | sin plazo | — | ninguno | — | ídem |

**California, Delete Act y DROP.** DROP (Delete Request and Opt-out Platform) está abierta a los
residentes de California desde el 1 de enero de 2026; **desde el 1 de agosto de 2026** los
corredores de datos inscritos tienen que consultarla y procesar las peticiones al menos cada 45
días, e informar del estado de cada una en 45 días. Página para el consumidor:
https://consumer.drop.privacy.ca.gov. Fuentes: https://privacy.ca.gov/drop/ y
https://privacy.ca.gov/data-brokers (la agencia se presenta ahora como «CalPrivacy»). Como el
§1798.105 solo cubre los datos que la empresa recogió **de ti**, para un corredor que los compró a
otros el panel recomienda DROP antes que la carta.

**Otros estados en vigor a 11 oct 2026** (según clym.io y MultiState; fechas de entrada en
vigor): Montana (1 oct 2024), Iowa (1 ene 2025), Delaware (1 ene 2025), Nebraska (1 ene 2025),
New Hampshire (1 ene 2025), New Jersey (15 ene 2025), Tennessee (1 jul 2025), Minnesota (31 jul
2025), Maryland (1 oct 2025), Indiana, Kentucky y Rhode Island (1 ene 2026), y Florida con un
alcance estrecho (1 jul 2024). **No se comprobó su texto artículo por artículo**: por eso tienen
una sola fila genérica, con el plazo marcado como referencia.

**Registros de corredores** (inscribirse no da por sí solo derecho a borrar):
- Oregón: los corredores se inscriben en el Department of Consumer and Business Services (DCBS),
  que publica el registro. Fuente: https://www.doj.state.or.us/media-home/news-media-releases/faq_category/privacy-for-consumers-other-important-information/
- Texas: Business & Commerce Code, capítulo 509 (registro en la Secretaría de Estado). La página
  del capítulo apareció en la búsqueda (https://law.justia.com/codes/texas/2023/business-and-commerce-code/title-11/subtitle-a/chapter-509/section-509-006)
  pero **no se abrió**.
- Vermont: registro de corredores desde 2018 (9 V.S.A. §2430 y ss.). **No se abrió una fuente
  primaria** en esta sesión; solo referencias secundarias (p. ej.
  https://www.dwt.com/insights/2018/05/new-vermont-data-broker-regulatory-regime-is-enact).
- California: registro de la CPPA, https://cppa.ca.gov/data_broker_registry/ (el que ya usa la
  decisión 194).

## Canadá, Australia y Nueva Zelanda

| Código | Lugar | Ley y lo que se cita | Plazo | Prórroga | Derecho | Autoridad | Fuentes |
|---|---|---|---|---|---|---|---|
| `ca` | Canadá (federal) | PIPEDA, anexo 1, principios 4.3.8 (retirar consentimiento), 4.5.3 (destruir lo innecesario), 4.9 (acceso y corrección) | 30 días (art. 8(3)) | 30 días (art. 8(4)) | limitado | OPC: https://www.priv.gc.ca/en/report-a-concern/file-a-formal-privacy-complaint/file-a-complaint-about-a-business | Art. 8: https://laws-lois.justice.gc.ca/eng/acts/P-8.6/section-8.html · retirar el consentimiento lleva a borrar: https://priv.gc.ca/en/opc-actions-and-decisions/investigations/investigations-into-businesses/2017/pipeda-2017-005 · principio 4.5.3 aplicado en 2026 (Loblaw): https://www.priv.gc.ca/en/opc-actions-and-decisions/investigations/investigations-into-businesses/2026/pipeda-2026-001/ |
| `ca_qc` | Quebec | Ley del sector privado (P-39.1) art. 28 y Código Civil art. 40 | 30 días (art. 32; el silencio es negativa) | — | limitado | Commission d'accès à l'information: https://www.cai.gouv.qc.ca (30 días para acudir, art. 43) | https://www.legisquebec.gouv.qc.ca/en/document/cs/p-39.1 · https://www.blg.com/en/insights/2021/05/the-right-to-erasure-of-personal-information |
| `au` | Australia | Privacy Act 1988, APP 12 (acceso) y 13 (corrección) | «plazo razonable»; la OAIC considera razonables 30 días (**referencia**) | — | limitado | OAIC: https://www.oaic.gov.au | https://oaic.gov.au/privacy/your-privacy-rights/your-personal-information/correct-your-personal-information · reforma con derecho de borrado solo en borrador (31 ago 2026): https://ia.acs.org.au/article/2026/australians-to-gain--right-to-erasure--under-privacy-reforms.html |
| `nz` | Nueva Zelanda | Privacy Act 2020, IPP 6 (acceso) y 7 (corrección, que puede incluir borrar) | 20 días hábiles | — | limitado | Privacy Commissioner: https://www.privacy.org.nz | https://www.privacy.org.nz/resources-and-learning/knowledge-base/view/235/ |

Notas:
- **Canadá**: la PIPEDA no tiene un derecho general de borrado (lo confirma BLG para el sector
  privado canadiense). El plazo de 30 días del art. 8 es el de las solicitudes de acceso; la carta
  lo usa para todo lo que pide. El número exacto del principio 4.3.8 se apoya en la resolución de
  2017 (retirar el consentimiento) y en un índice secundario; el texto del anexo 1 no se abrió.
- **Quebec**: el art. 28 (tras la Ley 25) habla de rectificar, también lo recogido sin permiso de
  la ley; el borrado de lo caducado o injustificado viene del art. 40 del Código Civil. El art. 40
  se cita según BLG; **no se abrió el Código Civil**.
- **Australia**: no hay derecho general de borrado a 11 oct 2026; el borrador del «Tranche 2»
  (31 ago 2026) lo incluiría. La exención de pequeñas empresas **no se comprobó** y el panel no la
  menciona.
- **Nueva Zelanda**: no se encontró el número de sección del plazo de 20 días hábiles; se cita el
  plazo como lo da la propia OPC.

## América Latina

| Código | Lugar | Ley y artículo citado | Plazo | Prórroga | Derecho | Autoridad | Fuentes |
|---|---|---|---|---|---|---|---|
| `br` | Brasil | LGPD (Lei 13.709/2018) art. 18, IV y VI | **sin plazo en días** (art. 18 §5 lo deja a reglamento); el reloj usa 15 días del art. 19 II como **referencia** | — | borrado | ANPD, «petição de titular» en gov.br: https://www.gov.br/pt-br/servicos/abrir-requerimento-relacionado-a-lgpd (exige prueba de haber escrito antes a la empresa) | https://planalto.gov.br/ccivil_03/_ato2015-2018/2018/lei/L13709.htm |
| `mx` | México | LFPDPPP **nueva** (DOF 20 mar 2025) art. 24 (cancelación) | 20 días hábiles (art. 31; art. 2 fr. VIII: «Días: días hábiles») + 15 días para hacerla efectiva | 20 días (una vez) | borrado | Secretaría Anticorrupción y Buen Gobierno (art. 2 fr. XV): https://www.anticorrupcionybg.gob.mx · 15 días para presentar la solicitud de protección (art. 40) | https://www.diputados.gob.mx/LeyesBiblio/pdf/LFPDPPP.pdf |
| `co` | Colombia | Ley 1581 de 2012 art. 8 lit. e) | 15 días hábiles (art. 15) | 8 días hábiles | borrado | SIC: https://www.sic.gov.co (solo tras reclamar a la empresa, art. 16) | https://barranquilla.gov.co/documento/ley-1581-del-2012 |
| `ar` | Argentina | Ley 25.326 art. 16 | 5 días hábiles (art. 16.2) | — | borrado («cuando corresponda») | AAIP: https://www.argentina.gob.ar/aaip; o acción de hábeas data (art. 16.3) | https://www.argentina.gob.ar/normativa/nacional/64790/texto |
| `cl` | Chile | Ley 19.628 art. 12 (hoy); Ley 21.719 desde el **1 dic 2026** | 2 días hábiles (art. 16) | — | borrado (sin fundamento legal o caducos) | Hoy ninguna: juez de letras en lo civil (art. 16). Desde el 1 dic 2026, la Agencia de Protección de Datos Personales | Texto de 1999: https://www.registrosocial.gob.cl/docs/LEY-19628_28-AGO-1999.pdf · vigencia de la 21.719: https://www.diarioconstitucional.cl/2026/06/12/la-ley-21-719-entra-en-vigor-el-1-de-diciembre-y-expone-vacios-en-regulacion-de-pequenas-empresas/ |
| `pe` | Perú | Ley 29733 art. 20; Reglamento DS 016-2024-JUS (en vigor 30 mar 2025) | 10 días hábiles (Reglamento art. 69.3) | igual plazo, una vez (art. 71) | borrado | Autoridad Nacional de Protección de Datos Personales: https://www.gob.pe/anpd (art. 24 de la Ley) | Reglamento: https://www.congreso.gob.pe/Docs/DGP/DIDP/files/ds_016-2024-jus.pdf · arts. 20 y 24 citados en una resolución de la DPDP: https://cdn.www.gob.pe/uploads/document/file/3512953/EXP.%20014-2022%20-%20RD%202255-2022-DPDP.pdf.pdf · vigencia: https://kom.pe/reglamento-ds-016-2024-jus-datos-personales/ |

Notas:
- **Brasil**: el art. 18 §4 obliga a responder de inmediato o explicando por qué no; el plazo en
  días quedó a un reglamento de la ANPD que **no encontramos en vigor**. Por eso el reloj cuenta
  15 días y lo dice. La agenda regulatoria de la ANPD 2025-2026 no se pudo abrir.
- **México**: es la ley nueva (el INAI desapareció). Los 20 días del art. 31 son hábiles por la
  definición del art. 2. Tras la respuesta, 15 días para hacerla efectiva.
- **Colombia**: en datos de crédito rige además la Ley 1266 de 2008 (permanencia de los datos
  negativos); el panel lo menciona sin dar plazos.
- **Chile**: el texto del art. 12 y del art. 16 se leyó en la versión original de 1999 publicada
  por el Registro Social de Hogares; **no se abrió el texto consolidado** de la BCN (la página no
  cargó). Los plazos de la Ley 21.719 para responder (cuando entre en vigor) **no se comprobaron**.
- **Perú**: el «procedimiento trilateral de tutela» no se comprobó y no se nombra; el panel dice
  «reclamación ante la Autoridad».

## Asia y Oriente Medio (1.0.19)

Comprobado el 13 oct 2026 leyendo el texto oficial (o la traducción oficial al inglés) de cada
ley. La carta va en **inglés** en todos: el panel no está en esos idiomas, y la página lo dice.

| Código | Lugar | Ley y artículo citado (borrado · acceso) | Plazo borrado | Plazo acceso | Derecho | Autoridad | Fuentes |
|---|---|---|---|---|---|---|---|
| `tr` | Turquía | Ley 6698 (KVKK) art. 11.1.e · art. 11.1.a-b | 30 días (art. 13) | igual | borrado | KVKK https://www.kvkk.gov.tr/ (solo tras pedir a la empresa: 30 días desde su respuesta o 60 desde la solicitud, art. 14) | https://www.kvkk.gov.tr/Icerik/6649/Personal-Data-Protection-Law |
| `jp` | Japón | APPI arts. 33 y 35 · art. 33 | «sin demora»; el reloj cuenta **2 semanas de referencia** (art. 39: esperar antes de demandar) | igual | limitado (art. 35: fuera del fin, obtenidos mal, ya no necesarios, riesgo para tus derechos) | PPC https://www.ppc.go.jp/en/ | https://www.japaneselawtranslation.go.jp/en/laws/view/4241 |
| `kr` | Corea del Sur | PIPA art. 36 · art. 35 | 10 días (Decreto art. 43.3) | 10 días (Decreto art. 41.4) | borrado | PIPC https://www.pipc.go.kr/eng | https://elaw.klri.re.kr/eng_service/lawView.do?hseq=62389&lang=ENG · Decreto: hseq=63755 |
| `cn` | China continental | PIPL art. 47 · art. 45 | **sin días** («a tiempo») | sin días | borrado (con condiciones del art. 47) | CAC https://www.cac.gov.cn; o demanda judicial si se niegan (art. 50) | http://www.npc.gov.cn/c2/c30834/202108/t20210820_313088.html |
| `hk` | Hong Kong | PDPO ss. 18, 22 y 26 · s. 18 | 40 días (ss. 19 y 23) | igual | limitado (s. 26 es un deber de la empresa, no un derecho a pedir) | PCPD https://www.pcpd.org.hk | https://www.elegislation.gov.hk/hk/cap486!en |
| `tw` | Taiwán | PDPA art. 11 · art. 10 | 30 + 30 días (art. 13) | 15 + 15 días (art. 13) | borrado | el organismo que supervisa el sector de la empresa (sin enlace) | https://law.moj.gov.tw/ENG/LawClass/LawAll.aspx?pcode=I0050021 |
| `sg` | Singapur | PDPA ss. 16, 21 y 25 · s. 21 | 30 días de **referencia** (Reglamento 2021, reg. 5: avisar si tardan más) | igual | limitado (sin derecho de borrado) | PDPC https://www.pdpc.gov.sg | https://sso.agc.gov.sg/Act/PDPA2012 · https://sso.agc.gov.sg/SL/PDPA2012-S63-2021?WholeDoc=1 |
| `th` | Tailandia | PDPA B.E. 2562 s. 33 · s. 30 | **sin días** | 30 días (s. 30) | borrado | Office of the PDPC https://www.pdpc.or.th (s. 73; comité de expertos, s. 33) | https://www.mdes.go.th/uploads/tinymce/source/%E0%B8%AA%E0%B8%84%E0%B8%AA/Personal%20Data%20Protection%20Act%202019.pdf |
| `my` | Malasia | Ley 709 ss. 10, 30, 34 y 38 · s. 30 | 21 días (ss. 31 y 35) | 21 + 14 días (s. 31) | limitado | Comisionado (JPDP) https://www.pdp.gov.my (s. 104) | Reimpresión oficial 2026: https://lom.agc.gov.my/ilims/upload/portal/akta/outputaktap/3301061_BI/Act%20709%20(Reprint%202026).pdf |
| `ph` | Filipinas | RA 10173 s. 16(e) · s. 16(c) | 15 días naturales de **referencia** (Reglas de procedimiento de la NPC 2021, regla II s. 2: sin respuesta en 15 días se puede reclamar) | igual | borrado (bloqueo, retirada o destrucción, con condiciones) | NPC https://privacy.gov.ph | https://privacy.gov.ph/data-privacy-act/ · https://privacy.gov.ph/wp-content/uploads/2024/03/2021-Rules-of-Procedure-of-the-NPC-As-Amended.pdf |
| `id` | Indonesia | UU 27/2022 arts. 8 y 43 · arts. 7 y 32 | **sin días** (art. 45: avisar del borrado) | 3 × 24 horas (art. 32.2) | borrado | la agencia que designe el Presidente (art. 58), sin enlace; conflicto: arbitraje o tribunales (art. 64) | https://peraturan.bpk.go.id/Details/229798/uu-no-27-tahun-2022 |
| `sa` | Arabia Saudí | PDPL art. 4.5 · art. 4.2 | 30 + 30 días (Reglamento de aplicación art. 3.1.a) | igual | borrado (cuando ya no hace falta, salvo art. 18) | SDAIA https://sdaia.gov.sa (art. 34) | https://sdaia.gov.sa/en/SDAIA/about/Documents/PersonalDataProtectionLaw.pdf · https://sdaia.gov.sa/en/SDAIA/about/Documents/ImplementingRegulationPersonalDataProtectionLaw.pdf |

Notas:
- **Los que no entran todavía**: India (los derechos de acceso y borrado de la DPDP Act 2023 y su
  Reglamento de 2025, con 90 días para responder, se aplican desde mayo de 2027; hoy no hay una ley
  que la carta pueda citar), Vietnam, Emiratos e Israel (no se pudo leer el texto oficial esta
  noche). Prefiero un hueco a un dato inventado.
- **Taiwán**: el plazo de 15 + 15 días para el acceso y 30 + 30 para el borrado están en el art. 13
  de la ley; la autoridad sectorial depende de la empresa, así que no se enlaza ninguna.
- **Japón y Filipinas**: la ley no fija días; el reloj cuenta un plazo de **referencia** que la
  propia ley o su autoridad usa para el paso siguiente (demanda o queja), y lo dice.
- Cuando el plazo es «sin días», la queja ya no deja una fecha vacía: dice cuántos días pasaron
  desde el envío.

## Derecho de acceso (Expedientes, 1.0.17)

Desde la 1.0.17 la carta puede pedir **acceso** («qué datos tienen de mí») además de borrado. El artículo citado está en `borrado_articulo_acceso_<código>` y, cuando el plazo de acceso es distinto del de borrado, en `borrado_plazo_acceso_<código>`. Mismo criterio que arriba: un plazo que no se comprobó no se cuenta.

| Código | Lo que se cita para el acceso | Plazo que cuenta el reloj | Fuente |
|---|---|---|---|
| `es`, `pt`, `ie`, `ue` | RGPD art. 15 (España: y LOPDGDD art. 13) | 1 mes + 2 (RGPD art. 12.3), igual que el borrado | RGPD (arriba) |
| `gb` | UK GDPR art. 15 | igual que el borrado | ICO (arriba) |
| `us_ca` | Civil Code §1798.110 (derecho a saber) | 45 + 45 días (§1798.130), igual | §1798.130 (arriba) |
| `us_va` … `us_or`, `us_otros` | el derecho de acceso de cada ley, sin número de sección | 45 + 45 días, igual | las mismas leyes (arriba) |
| `us` | ninguna ley federal; es una petición | sin plazo | — |
| `ca` | PIPEDA, principio 4.9 | 30 días (art. 8(3)), igual | arriba |
| `ca_qc` | Ley P-39.1 art. 27 | 30 días (art. 32), igual | arriba |
| `au` | APP 12 | 30 días de referencia, igual | arriba |
| `nz` | IPP 6 | 20 días hábiles, igual | arriba |
| `br` | LGPD art. 18, II | **15 días fijados por la ley** (art. 19, II): aquí no es referencia | https://www.legjur.com/legislacao/art/LEI_00137092018-19 (comprobado 12 oct 2026) |
| `mx` | el derecho de acceso de la LFPDPPP | 20 días hábiles (art. 31), igual | arriba |
| `co` | Ley 1581 art. 8 lit. a) y art. 14 (consultas) | **10 días hábiles + 5** (art. 14) | https://leyes.co/se_dictan_disposiciones_generales_para_la_proteccion_de_datos_personales/14.htm (comprobado 12 oct 2026) |
| `ar` | Ley 25.326 art. 14 | **10 días corridos** (art. 14.2) | https://leyes-ar.com/ley_de_proteccion_de_los_datos_personales/14.htm (comprobado 12 oct 2026) |
| `cl` | Ley 19.628 art. 12 | 2 días hábiles (art. 16), igual | arriba |
| `pe` | Ley 29733 art. 19 | **sin contar**: no se comprobó el plazo del Reglamento para el acceso | — |

Pendiente, como el resto: que un abogado revise esta tabla.

## Las empresas (`crates/panel/data/borrado-corredores.txt`)

30 empresas, cada una con su `fuente` y la fecha en que se miró. Dos clases de fuente:

1. La página de privacidad de la propia empresa, abierta el 11 oct 2026: Spokeo, Acxiom, LiveRamp,
   Lotame (remite al formulario de Epsilon), Bombora, Criteo, Taboola, Quantcast, Equifax Ibérica
   (ASNEF), Experian Bureau de Crédito (Badexcug), Serasa Experian, Equifax BoaVista, DataCrédito
   Experian, Radaris (ver abajo), Infobel (encontrada, no se pudo abrir).
2. Para las inscritas en California, la página de derechos que **ellas mismas** declararon en el
   registro de la CPPA (copia del 8 oct 2026, la de `crates/lists/data/corredores.txt`): Whitepages,
   BeenVerified, Intelius, TruthFinder, Instant Checkmate, PeopleFinders, MyLife, Epsilon, Nielsen,
   Comscore, Outbrain (Teads), Experian (US), Equifax (US), TransUnion. Algunas solo dieron su
   portada (MyLife, Nielsen, TransUnion); el panel enlaza eso y nada más.

Correos: solo los que la empresa publica para estos derechos, con el ámbito para el que los da:
Spokeo `CustomerCare@spokeo.com` (peticiones del EEE y Suiza), LiveRamp `ukprivacy@liveramp.com`
(Reino Unido), Bombora `dsars@bombora.com`, Criteo `dpo@criteo.com` (lo da para el acceso; el enlace
a su formulario de borrado no funcionaba el 11 oct 2026), DataCrédito
`servicioalciudadano@experian.com` (Colombia). Las demás no publican un correo para esto: la carta
se pega en su formulario.

Hallazgos que cambian lo que se le dice a la persona:
- **Oracle Advertising (BlueKai, AddThis, Moat)** dejó de dar servicio el 30 sep 2024
  (https://en.wikipedia.org/wiki/Oracle_Advertising). Sale como «cerró ese negocio».
- **Radaris**: la portada de radaris.com dice hoy que el dominio fue transferido por orden judicial
  a Atlas Data Privacy Corporation (sentencia del 27 ago 2026, ley Daniel de Nueva Jersey) y no
  ofrece formulario de borrado (https://radaris.com/control-privacy). Sale como «su web ya no es
  suya».
- **Lotame** es hoy de Epsilon (Publicis); su página de derechos manda al formulario de Epsilon.
  Su nombre de rastreo `crwdcntrl.net` está en su propia documentación.

Los nombres de cada empresa vienen del registro de California, de `empresas.txt` (líneas
`=Empresa`) o de la documentación de la propia empresa (Lotame). Un test comprueba que cada
`=Empresa` existe de verdad en `empresas.txt`.

## Lo que no se pudo comprobar (resumen)

- Texto primario de los principios 4.3.8 y 4.9 del anexo 1 de la PIPEDA; art. 40 del Código Civil de
  Quebec (citado a través de BLG).
- Sección exacta del plazo de 20 días hábiles en Nueva Zelanda.
- Texto consolidado actual de la Ley 19.628 (Chile) y plazos de la Ley 21.719.
- Si la ANPD (Brasil) ha fijado por reglamento el plazo del art. 18.
- Leyes de los otros doce estados de EE. UU., artículo por artículo.
- Registros de Vermont y Texas en fuente primaria.
- Exención de pequeñas empresas en Australia (no se menciona).
- Web de la CNPD (Portugal) y de Infobel, que no se abrieron.
