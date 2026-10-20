// GUARDIANA · «Órbita» (prototype, 19 Oct 2026). The home screen the owner chose among three
// (A · Órbita, designed with ChatGPT and Grok): the Earth fills the window and turns; nine doors
// sit on one steel ring, all at the same distance from it; every cut GUARDIANA makes is drawn on
// the planet the moment the panel measures it. A door opens its section in the same screen: the
// planet steps back behind glass, still turning and still cutting; Esc, «Órbita» or Back bring it
// home.
//
// Measurements only. The planet draws what `/api/por-dentro` says (the same answer as «En vivo»):
// a cut is a rise in `paradas` of a destination, a flight that went out is a rise in `enviados`.
// A destination with no known country is never drawn as if it had one. The computer is drawn at
// its country, never at the capital of its time zone (owner, 18 Oct 2026). Only a page marked
// `data-demo` (the design review outside the program) invents traffic, and it says
// «Demostración» on screen the whole time.
//
// No third-party code (brief §8): WebGL 1 written here; NASA's pictures served by the panel.
(() => {
  'use strict';
  // The countries' centres (Natural Earth 110 m, public domain) and the time zones of the IANA
  // database: the same tables as por-dentro.js, copied so this page stands alone.
  const P = `FJ 11.6 -16.4 | TZ 34.8 -6.3 | EH -12.1 24.3 | CA -101.6 57.7 | US -99.1 39.5 | KZ 67.3 48.2 | UZ 63.2 41.7 | PG 144.3 -6.6 | ID 114 -0.3 | AR -65.1 -35.2 | CL -71.7 -37.3 | CD 23.6 -2.8 | SO 45.7 4.8 | KE 37.8 0.6 | SD 29.9 16 | TD 18.6 15.3 | HT -73.6 18.6 | DO -70.5 18.9 | RU 124.6 61.2 | BS -77.9 24.5 | FK -59.4 -51.7 | NO 13.4 62.9 | GL -41.5 74.8 | TF 69.5 -49.3 | TL 126 -8.8 | ZA 25.1 -29 | LS 28.2 -29.6 | MX -102.6 23.9 | UY -56 -32.8 | BR -53.1 -10.8 | BO -64.6 -16.7 | PE -74.4 -9.2 | CO -73.1 3.9 | PA -80.1 8.5 | CR -84.2 10 | NI -85 12.8 | HN -86.6 14.8 | SV -88.9 13.7 | GT -90.4 15.7 | BZ -88.7 17.2 | VE -66.2 7.2 | GY -59 4.8 | SR -55.9 4.1 | FR 2.3 46.6 | EC -78.4 -1.5 | PR -66.5 18.2 | JM -77.3 18.1 | CU -79 21.6 | ZW 29.8 -18.9 | BW 23.8 -22.1 | NA 17.2 -22.1 | SN -14.5 14.4 | ML -3.5 17.3 | MR -10.3 20.2 | BJ 2.3 9.6 | NE 9.3 17.3 | NG 8 9.5 | CM 12.6 5.7 | TG 1 8.4 | GH -1.2 7.9 | CI -5.6 7.6 | GN -11.1 10.4 | GW -15.1 12 | LR -9.4 6.4 | SL -11.8 8.5 | BF -1.8 12.3 | CF 20.4 6.5 | CG 15.1 -0.8 | GA 11.7 -0.6 | GQ 10.4 1.6 | ZM 27.7 -13.4 | MW 34.2 -13.2 | MZ 35.5 -17.2 | SZ 31.4 -26.5 | AO 17.5 -12.3 | BI 29.9 -3.4 | IL 34.8 31.5 | LB 35.9 33.9 | MG 46.7 -19.4 | PS 35.3 31.9 | GM -15.4 13.5 | TN 9.5 34.2 | DZ 2.6 28.2 | JO 36.8 31.2 | AE 54.2 23.9 | QA 51.2 25.3 | KW 47.6 29.3 | IQ 43.8 33 | OM 56.1 20.6 | VU 166.9 -15.2 | KH 104.9 12.7 | TH 101 15 | LA 103.7 18.4 | MM 96.5 21 | VN 105.8 20 | KP 127.2 40.1 | KR 127.8 36.4 | MN 102.9 46.8 | IN 79.6 22.9 | BD 90.3 23.8 | BT 90.5 27.4 | NP 84 28.2 | PK 69.4 30 | AF 66.1 33.9 | TJ 71 38.6 | KG 74.6 41.5 | TM 59.3 39.1 | IR 54.3 32.5 | SY 38.5 35 | AM 45 40.2 | SE 16.6 62.8 | BY 28 53.5 | UA 31.2 49.1 | PL 19.3 52.1 | AT 14.1 47.6 | HU 19.4 47.2 | MD 28.4 47.2 | RO 24.9 45.9 | LT 23.9 55.3 | LV 24.8 56.8 | EE 25.8 58.6 | DE 10.3 51.1 | BG 25.2 42.8 | GR 22.6 39.3 | TR 35.4 39 | AL 20 41.1 | HR 17.5 43.7 | CH 8.1 46.8 | LU 6 49.8 | BE 4.6 50.7 | NL 5.5 52.3 | PT -8.1 39.6 | ES -3.6 40.3 | IE -8 53.2 | NC 165.5 -21.3 | SB 159.1 -7.9 | NZ 170.5 -44 | AU 134.4 -25.6 | LK 80.7 7.7 | CN 103.9 36.6 | TW 121 23.7 | IT 12.2 43.5 | DK 9.3 56.2 | GB -2.7 53.9 | IS -18.8 65.1 | AZ 47.7 40.3 | GE 43.5 42.2 | PH 121.5 15.8 | MY 114.7 3.5 | BN 114.9 4.7 | SI 14.9 46.1 | FI 26.2 64.5 | SK 19.5 48.7 | CZ 15.3 49.8 | ER 38.7 15.4 | JP 136.9 36 | PY -58.4 -23.2 | YE 47.5 15.9 | SA 44.5 24.1 | AQ 31.2 -77.3 | CY 33.6 35.3 | CY 33 34.9 | MA -8.4 29.9 | EG 29.8 26.5 | LY 18 27 | ET 39.6 8.7 | DJ 42.5 11.8 | SO 46.2 9.8 | UG 32.4 1.3 | RW 29.9 -2 | BA 17.8 44.2 | MK 21.7 41.6 | RS 20.8 44.2 | ME 19.3 42.8 | XK 20.9 42.6 | TT -61.3 10.4 | SS 30.2 7.3`;
  const PEQUENOS = [['SG', 103.8, 1.35], ['HK', 114.2, 22.3], ['BH', 50.55, 26.07], ['MT', 14.4, 35.9], ['MU', 57.55, -20.25], ['AD', 1.6, 42.5], ['MC', 7.42, 43.74], ['LI', 9.55, 47.16], ['SM', 12.46, 43.94], ['BB', -59.55, 13.17], ['MV', 73.5, 4.2], ['GI', -5.35, 36.14], ['MO', 113.55, 22.17], ['BM', -64.75, 32.3], ['KY', -81.25, 19.3], ['IM', -4.5, 54.2], ['JE', -2.13, 49.21], ['GG', -2.58, 49.45], ['VG', -64.6, 18.42], ['SC', 55.45, -4.6], ['AG', -61.8, 17.07], ['LC', -60.98, 13.9], ['GD', -61.68, 12.1], ['KN', -62.75, 17.33], ['VC', -61.2, 13.25], ['DM', -61.35, 15.42], ['CW', -68.95, 12.17], ['AW', -69.97, 12.52], ['ST', 6.6, 0.3], ['CV', -23.6, 15.1], ['KM', 43.3, -11.7], ['TO', -175.2, -21.2], ['WS', -172.1, -13.75], ['KI', 173.0, 1.4], ['MH', 171.2, 7.1], ['FM', 158.2, 6.9], ['PW', 134.5, 7.5], ['TV', 179.2, -8.5], ['NR', 166.93, -0.53]];
  const ZONAS = {'Africa/Abidjan':['CI',5.32,-4.03],'Africa/Accra':['GH',5.55,-0.22],'Africa/Addis_Ababa':['ET',9.03,38.7],'Africa/Algiers':['DZ',36.78,3.05],'Africa/Asmara':['ER',15.33,38.88],'Africa/Bamako':['ML',12.65,-8.0],'Africa/Bangui':['CF',4.37,18.58],'Africa/Banjul':['GM',13.47,-16.65],'Africa/Bissau':['GW',11.85,-15.58],'Africa/Blantyre':['MW',-15.78,35.0],'Africa/Brazzaville':['CG',-4.27,15.28],'Africa/Bujumbura':['BI',-3.38,29.37],'Africa/Cairo':['EG',30.05,31.25],'Africa/Casablanca':['MA',33.65,-7.58],'Africa/Ceuta':['ES',35.88,-5.32],'Africa/Conakry':['GN',9.52,-13.72],'Africa/Dakar':['SN',14.67,-17.43],'Africa/Dar_es_Salaam':['TZ',-6.8,39.28],'Africa/Djibouti':['DJ',11.6,43.15],'Africa/Douala':['CM',4.05,9.7],'Africa/El_Aaiun':['EH',27.15,-13.2],'Africa/Freetown':['SL',8.5,-13.25],'Africa/Gaborone':['BW',-24.65,25.92],'Africa/Harare':['ZW',-17.83,31.05],'Africa/Johannesburg':['ZA',-26.25,28.0],'Africa/Juba':['SS',4.85,31.62],'Africa/Kampala':['UG',0.32,32.42],'Africa/Khartoum':['SD',15.6,32.53],'Africa/Kigali':['RW',-1.95,30.07],'Africa/Kinshasa':['CD',-4.3,15.3],'Africa/Lagos':['NG',6.45,3.4],'Africa/Libreville':['GA',0.38,9.45],'Africa/Lome':['TG',6.13,1.22],'Africa/Luanda':['AO',-8.8,13.23],'Africa/Lubumbashi':['CD',-11.67,27.47],'Africa/Lusaka':['ZM',-15.42,28.28],'Africa/Malabo':['GQ',3.75,8.78],'Africa/Maputo':['MZ',-25.97,32.58],'Africa/Maseru':['LS',-29.47,27.5],'Africa/Mbabane':['SZ',-26.3,31.1],'Africa/Mogadishu':['SO',2.07,45.37],'Africa/Monrovia':['LR',6.3,-10.78],'Africa/Nairobi':['KE',-1.28,36.82],'Africa/Ndjamena':['TD',12.12,15.05],'Africa/Niamey':['NE',13.52,2.12],'Africa/Nouakchott':['MR',18.1,-15.95],'Africa/Ouagadougou':['BF',12.37,-1.52],'Africa/Porto-Novo':['BJ',6.48,2.62],'Africa/Sao_Tome':['ST',0.33,6.73],'Africa/Tripoli':['LY',32.9,13.18],'Africa/Tunis':['TN',36.8,10.18],'Africa/Windhoek':['NA',-22.57,17.1],'America/Adak':['US',51.88,-176.66],'America/Anchorage':['US',61.22,-149.9],'America/Anguilla':['AI',18.2,-63.07],'America/Antigua':['AG',17.05,-61.8],'America/Araguaina':['BR',-7.2,-48.2],'America/Argentina/Buenos_Aires':['AR',-34.6,-58.45],'America/Argentina/Catamarca':['AR',-28.47,-65.78],'America/Argentina/Cordoba':['AR',-31.4,-64.18],'America/Argentina/Jujuy':['AR',-24.18,-65.3],'America/Argentina/La_Rioja':['AR',-29.43,-66.85],'America/Argentina/Mendoza':['AR',-32.88,-68.82],'America/Argentina/Rio_Gallegos':['AR',-51.63,-69.22],'America/Argentina/Salta':['AR',-24.78,-65.42],'America/Argentina/San_Juan':['AR',-31.53,-68.52],'America/Argentina/San_Luis':['AR',-33.32,-66.35],'America/Argentina/Tucuman':['AR',-26.82,-65.22],'America/Argentina/Ushuaia':['AR',-54.8,-68.3],'America/Aruba':['AW',12.5,-69.97],'America/Asuncion':['PY',-25.27,-57.67],'America/Atikokan':['CA',48.76,-91.62],'America/Bahia':['BR',-12.98,-38.52],'America/Bahia_Banderas':['MX',20.8,-105.25],'America/Barbados':['BB',13.1,-59.62],'America/Belem':['BR',-1.45,-48.48],'America/Belize':['BZ',17.5,-88.2],'America/Blanc-Sablon':['CA',51.42,-57.12],'America/Boa_Vista':['BR',2.82,-60.67],'America/Bogota':['CO',4.6,-74.08],'America/Boise':['US',43.61,-116.2],'America/Buenos_Aires':['AR',-34.6,-58.45],'America/Cambridge_Bay':['CA',69.11,-105.05],'America/Campo_Grande':['BR',-20.45,-54.62],'America/Cancun':['MX',21.08,-86.77],'America/Caracas':['VE',10.5,-66.93],'America/Cayenne':['GF',4.93,-52.33],'America/Cayman':['KY',19.3,-81.38],'America/Chicago':['US',41.85,-87.65],'America/Chihuahua':['MX',28.63,-106.08],'America/Ciudad_Juarez':['MX',31.73,-106.48],'America/Costa_Rica':['CR',9.93,-84.08],'America/Coyhaique':['CL',-45.57,-72.07],'America/Creston':['CA',49.1,-116.52],'America/Cuiaba':['BR',-15.58,-56.08],'America/Curacao':['CW',12.18,-69.0],'America/Danmarkshavn':['GL',76.77,-18.67],'America/Dawson':['CA',64.07,-139.42],'America/Dawson_Creek':['CA',55.77,-120.23],'America/Denver':['US',39.74,-104.98],'America/Detroit':['US',42.33,-83.05],'America/Dominica':['DM',15.3,-61.4],'America/Edmonton':['CA',53.55,-113.47],'America/Eirunepe':['BR',-6.67,-69.87],'America/El_Salvador':['SV',13.7,-89.2],'America/Fort_Nelson':['CA',58.8,-122.7],'America/Fortaleza':['BR',-3.72,-38.5],'America/Glace_Bay':['CA',46.2,-59.95],'America/Godthab':['GL',64.18,-51.73],'America/Goose_Bay':['CA',53.33,-60.42],'America/Grand_Turk':['TC',21.47,-71.13],'America/Grenada':['GD',12.05,-61.75],'America/Guadeloupe':['GP',16.23,-61.53],'America/Guatemala':['GT',14.63,-90.52],'America/Guayaquil':['EC',-2.17,-79.83],'America/Guyana':['GY',6.8,-58.17],'America/Halifax':['CA',44.65,-63.6],'America/Havana':['CU',23.13,-82.37],'America/Hermosillo':['MX',29.07,-110.97],'America/Indiana/Indianapolis':['US',39.77,-86.16],'America/Indiana/Knox':['US',41.3,-86.62],'America/Indiana/Marengo':['US',38.38,-86.34],'America/Indiana/Petersburg':['US',38.49,-87.28],'America/Indiana/Tell_City':['US',37.95,-86.76],'America/Indiana/Vevay':['US',38.75,-85.07],'America/Indiana/Vincennes':['US',38.68,-87.53],'America/Indiana/Winamac':['US',41.05,-86.6],'America/Inuvik':['CA',68.35,-133.72],'America/Iqaluit':['CA',63.73,-68.47],'America/Jamaica':['JM',17.97,-76.79],'America/Juneau':['US',58.3,-134.42],'America/Kentucky/Louisville':['US',38.25,-85.76],'America/Kentucky/Monticello':['US',36.83,-84.85],'America/Kralendijk':['BQ',12.15,-68.28],'America/La_Paz':['BO',-16.5,-68.15],'America/Lima':['PE',-12.05,-77.05],'America/Los_Angeles':['US',34.05,-118.24],'America/Lower_Princes':['SX',18.05,-63.05],'America/Maceio':['BR',-9.67,-35.72],'America/Managua':['NI',12.15,-86.28],'America/Manaus':['BR',-3.13,-60.02],'America/Marigot':['MF',18.07,-63.08],'America/Martinique':['MQ',14.6,-61.08],'America/Matamoros':['MX',25.83,-97.5],'America/Mazatlan':['MX',23.22,-106.42],'America/Menominee':['US',45.11,-87.61],'America/Merida':['MX',20.97,-89.62],'America/Metlakatla':['US',55.13,-131.58],'America/Mexico_City':['MX',19.4,-99.15],'America/Miquelon':['PM',47.05,-56.33],'America/Moncton':['CA',46.1,-64.78],'America/Monterrey':['MX',25.67,-100.32],'America/Montevideo':['UY',-34.91,-56.21],'America/Montserrat':['MS',16.72,-62.22],'America/Nassau':['BS',25.08,-77.35],'America/New_York':['US',40.71,-74.01],'America/Nome':['US',64.5,-165.41],'America/Noronha':['BR',-3.85,-32.42],'America/North_Dakota/Beulah':['US',47.26,-101.78],'America/North_Dakota/Center':['US',47.12,-101.3],'America/North_Dakota/New_Salem':['US',46.84,-101.41],'America/Nuuk':['GL',64.18,-51.73],'America/Ojinaga':['MX',29.57,-104.42],'America/Panama':['PA',8.97,-79.53],'America/Paramaribo':['SR',5.83,-55.17],'America/Phoenix':['US',33.45,-112.07],'America/Port-au-Prince':['HT',18.53,-72.33],'America/Port_of_Spain':['TT',10.65,-61.52],'America/Porto_Velho':['BR',-8.77,-63.9],'America/Puerto_Rico':['PR',18.47,-66.11],'America/Punta_Arenas':['CL',-53.15,-70.92],'America/Rankin_Inlet':['CA',62.82,-92.08],'America/Recife':['BR',-8.05,-34.9],'America/Regina':['CA',50.4,-104.65],'America/Resolute':['CA',74.7,-94.83],'America/Rio_Branco':['BR',-9.97,-67.8],'America/Santarem':['BR',-2.43,-54.87],'America/Santiago':['CL',-33.45,-70.67],'America/Santo_Domingo':['DO',18.47,-69.9],'America/Sao_Paulo':['BR',-23.53,-46.62],'America/Scoresbysund':['GL',70.48,-21.97],'America/Sitka':['US',57.18,-135.3],'America/St_Barthelemy':['BL',17.88,-62.85],'America/St_Johns':['CA',47.57,-52.72],'America/St_Kitts':['KN',17.3,-62.72],'America/St_Lucia':['LC',14.02,-61.0],'America/St_Thomas':['VI',18.35,-64.93],'America/St_Vincent':['VC',13.15,-61.23],'America/Swift_Current':['CA',50.28,-107.83],'America/Tegucigalpa':['HN',14.1,-87.22],'America/Thule':['GL',76.57,-68.78],'America/Tijuana':['MX',32.53,-117.02],'America/Toronto':['CA',43.65,-79.38],'America/Tortola':['VG',18.45,-64.62],'America/Vancouver':['CA',49.27,-123.12],'America/Whitehorse':['CA',60.72,-135.05],'America/Winnipeg':['CA',49.88,-97.15],'America/Yakutat':['US',59.55,-139.73],'Antarctica/Casey':['AQ',-66.28,110.52],'Antarctica/Davis':['AQ',-68.58,77.97],'Antarctica/DumontDUrville':['AQ',-66.67,140.02],'Antarctica/Macquarie':['AU',-54.5,158.95],'Antarctica/Mawson':['AQ',-67.6,62.88],'Antarctica/McMurdo':['AQ',-77.83,166.6],'Antarctica/Palmer':['AQ',-64.8,-64.1],'Antarctica/Rothera':['AQ',-67.57,-68.13],'Antarctica/Syowa':['AQ',-69.01,39.59],'Antarctica/Troll':['AQ',-72.01,2.53],'Antarctica/Vostok':['AQ',-78.4,106.9],'Arctic/Longyearbyen':['SJ',78.0,16.0],'Asia/Aden':['YE',12.75,45.2],'Asia/Almaty':['KZ',43.25,76.95],'Asia/Amman':['JO',31.95,35.93],'Asia/Anadyr':['RU',64.75,177.48],'Asia/Aqtau':['KZ',44.52,50.27],'Asia/Aqtobe':['KZ',50.28,57.17],'Asia/Ashgabat':['TM',37.95,58.38],'Asia/Atyrau':['KZ',47.12,51.93],'Asia/Baghdad':['IQ',33.35,44.42],'Asia/Bahrain':['BH',26.38,50.58],'Asia/Baku':['AZ',40.38,49.85],'Asia/Bangkok':['TH',13.75,100.52],'Asia/Barnaul':['RU',53.37,83.75],'Asia/Beirut':['LB',33.88,35.5],'Asia/Bishkek':['KG',42.9,74.6],'Asia/Brunei':['BN',4.93,114.92],'Asia/Calcutta':['IN',22.53,88.37],'Asia/Chita':['RU',52.05,113.47],'Asia/Colombo':['LK',6.93,79.85],'Asia/Damascus':['SY',33.5,36.3],'Asia/Dhaka':['BD',23.72,90.42],'Asia/Dili':['TL',-8.55,125.58],'Asia/Dubai':['AE',25.3,55.3],'Asia/Dushanbe':['TJ',38.58,68.8],'Asia/Famagusta':['CY',35.12,33.95],'Asia/Gaza':['PS',31.5,34.47],'Asia/Hebron':['PS',31.53,35.09],'Asia/Ho_Chi_Minh':['VN',10.75,106.67],'Asia/Hong_Kong':['HK',22.28,114.15],'Asia/Hovd':['MN',48.02,91.65],'Asia/Irkutsk':['RU',52.27,104.33],'Asia/Jakarta':['ID',-6.17,106.8],'Asia/Jayapura':['ID',-2.53,140.7],'Asia/Jerusalem':['IL',31.78,35.22],'Asia/Kabul':['AF',34.52,69.2],'Asia/Kamchatka':['RU',53.02,158.65],'Asia/Karachi':['PK',24.87,67.05],'Asia/Kathmandu':['NP',27.72,85.32],'Asia/Katmandu':['NP',27.72,85.32],'Asia/Khandyga':['RU',62.66,135.55],'Asia/Kolkata':['IN',22.53,88.37],'Asia/Krasnoyarsk':['RU',56.02,92.83],'Asia/Kuala_Lumpur':['MY',3.17,101.7],'Asia/Kuching':['MY',1.55,110.33],'Asia/Kuwait':['KW',29.33,47.98],'Asia/Macau':['MO',22.2,113.54],'Asia/Magadan':['RU',59.57,150.8],'Asia/Makassar':['ID',-5.12,119.4],'Asia/Manila':['PH',14.59,120.97],'Asia/Muscat':['OM',23.6,58.58],'Asia/Nicosia':['CY',35.17,33.37],'Asia/Novokuznetsk':['RU',53.75,87.12],'Asia/Novosibirsk':['RU',55.03,82.92],'Asia/Omsk':['RU',55.0,73.4],'Asia/Oral':['KZ',51.22,51.35],'Asia/Phnom_Penh':['KH',11.55,104.92],'Asia/Pontianak':['ID',-0.03,109.33],'Asia/Pyongyang':['KP',39.02,125.75],'Asia/Qatar':['QA',25.28,51.53],'Asia/Qostanay':['KZ',53.2,63.62],'Asia/Qyzylorda':['KZ',44.8,65.47],'Asia/Rangoon':['MM',16.78,96.17],'Asia/Riyadh':['SA',24.63,46.72],'Asia/Saigon':['VN',10.75,106.67],'Asia/Sakhalin':['RU',46.97,142.7],'Asia/Samarkand':['UZ',39.67,66.8],'Asia/Seoul':['KR',37.55,126.97],'Asia/Shanghai':['CN',31.23,121.47],'Asia/Singapore':['SG',1.28,103.85],'Asia/Srednekolymsk':['RU',67.47,153.72],'Asia/Taipei':['TW',25.05,121.5],'Asia/Tashkent':['UZ',41.33,69.3],'Asia/Tbilisi':['GE',41.72,44.82],'Asia/Tehran':['IR',35.67,51.43],'Asia/Thimphu':['BT',27.47,89.65],'Asia/Tokyo':['JP',35.65,139.74],'Asia/Tomsk':['RU',56.5,84.97],'Asia/Ulaanbaatar':['MN',47.92,106.88],'Asia/Urumqi':['CN',43.8,87.58],'Asia/Ust-Nera':['RU',64.56,143.23],'Asia/Vientiane':['LA',17.97,102.6],'Asia/Vladivostok':['RU',43.17,131.93],'Asia/Yakutsk':['RU',62.0,129.67],'Asia/Yangon':['MM',16.78,96.17],'Asia/Yekaterinburg':['RU',56.85,60.6],'Asia/Yerevan':['AM',40.18,44.5],'Atlantic/Azores':['PT',37.73,-25.67],'Atlantic/Bermuda':['BM',32.28,-64.77],'Atlantic/Canary':['ES',28.1,-15.4],'Atlantic/Cape_Verde':['CV',14.92,-23.52],'Atlantic/Faroe':['FO',62.02,-6.77],'Atlantic/Madeira':['PT',32.63,-16.9],'Atlantic/Reykjavik':['IS',64.15,-21.85],'Atlantic/South_Georgia':['GS',-54.27,-36.53],'Atlantic/St_Helena':['SH',-15.92,-5.7],'Atlantic/Stanley':['FK',-51.7,-57.85],'Australia/Adelaide':['AU',-34.92,138.58],'Australia/Brisbane':['AU',-27.47,153.03],'Australia/Broken_Hill':['AU',-31.95,141.45],'Australia/Darwin':['AU',-12.47,130.83],'Australia/Eucla':['AU',-31.72,128.87],'Australia/Hobart':['AU',-42.88,147.32],'Australia/Lindeman':['AU',-20.27,149.0],'Australia/Lord_Howe':['AU',-31.55,159.08],'Australia/Melbourne':['AU',-37.82,144.97],'Australia/Perth':['AU',-31.95,115.85],'Australia/Sydney':['AU',-33.87,151.22],'Europe/Amsterdam':['NL',52.37,4.9],'Europe/Andorra':['AD',42.5,1.52],'Europe/Astrakhan':['RU',46.35,48.05],'Europe/Athens':['GR',37.97,23.72],'Europe/Belgrade':['RS',44.83,20.5],'Europe/Berlin':['DE',52.5,13.37],'Europe/Bratislava':['SK',48.15,17.12],'Europe/Brussels':['BE',50.83,4.33],'Europe/Bucharest':['RO',44.43,26.1],'Europe/Budapest':['HU',47.5,19.08],'Europe/Busingen':['DE',47.7,8.68],'Europe/Chisinau':['MD',47.0,28.83],'Europe/Copenhagen':['DK',55.67,12.58],'Europe/Dublin':['IE',53.33,-6.25],'Europe/Gibraltar':['GI',36.13,-5.35],'Europe/Guernsey':['GG',49.45,-2.54],'Europe/Helsinki':['FI',60.17,24.97],'Europe/Isle_of_Man':['IM',54.15,-4.47],'Europe/Istanbul':['TR',41.02,28.97],'Europe/Jersey':['JE',49.18,-2.11],'Europe/Kaliningrad':['RU',54.72,20.5],'Europe/Kiev':['UA',50.43,30.52],'Europe/Kirov':['RU',58.6,49.65],'Europe/Kyiv':['UA',50.43,30.52],'Europe/Lisbon':['PT',38.72,-9.13],'Europe/Ljubljana':['SI',46.05,14.52],'Europe/London':['GB',51.51,-0.13],'Europe/Luxembourg':['LU',49.6,6.15],'Europe/Madrid':['ES',40.4,-3.68],'Europe/Malta':['MT',35.9,14.52],'Europe/Mariehamn':['AX',60.1,19.95],'Europe/Minsk':['BY',53.9,27.57],'Europe/Monaco':['MC',43.7,7.38],'Europe/Moscow':['RU',55.76,37.62],'Europe/Oslo':['NO',59.92,10.75],'Europe/Paris':['FR',48.87,2.33],'Europe/Podgorica':['ME',42.43,19.27],'Europe/Prague':['CZ',50.08,14.43],'Europe/Riga':['LV',56.95,24.1],'Europe/Rome':['IT',41.9,12.48],'Europe/Samara':['RU',53.2,50.15],'Europe/San_Marino':['SM',43.92,12.47],'Europe/Sarajevo':['BA',43.87,18.42],'Europe/Saratov':['RU',51.57,46.03],'Europe/Simferopol':['UA',44.95,34.1],'Europe/Skopje':['MK',41.98,21.43],'Europe/Sofia':['BG',42.68,23.32],'Europe/Stockholm':['SE',59.33,18.05],'Europe/Tallinn':['EE',59.42,24.75],'Europe/Tirane':['AL',41.33,19.83],'Europe/Ulyanovsk':['RU',54.33,48.4],'Europe/Vaduz':['LI',47.15,9.52],'Europe/Vatican':['VA',41.9,12.45],'Europe/Vienna':['AT',48.22,16.33],'Europe/Vilnius':['LT',54.68,25.32],'Europe/Volgograd':['RU',48.73,44.42],'Europe/Warsaw':['PL',52.25,21.0],'Europe/Zagreb':['HR',45.8,15.97],'Europe/Zurich':['CH',47.38,8.53],'Indian/Antananarivo':['MG',-18.92,47.52],'Indian/Chagos':['IO',-7.33,72.42],'Indian/Christmas':['CX',-10.42,105.72],'Indian/Cocos':['CC',-12.17,96.92],'Indian/Comoro':['KM',-11.68,43.27],'Indian/Kerguelen':['TF',-49.35,70.22],'Indian/Mahe':['SC',-4.67,55.47],'Indian/Maldives':['MV',4.17,73.5],'Indian/Mauritius':['MU',-20.17,57.5],'Indian/Mayotte':['YT',-12.78,45.23],'Indian/Reunion':['RE',-20.87,55.47],'Pacific/Apia':['WS',-13.83,-171.73],'Pacific/Auckland':['NZ',-36.87,174.77],'Pacific/Bougainville':['PG',-6.22,155.57],'Pacific/Chatham':['NZ',-43.95,-176.55],'Pacific/Chuuk':['FM',7.42,151.78],'Pacific/Easter':['CL',-27.15,-109.43],'Pacific/Efate':['VU',-17.67,168.42],'Pacific/Fakaofo':['TK',-9.37,-171.23],'Pacific/Fiji':['FJ',-18.13,178.42],'Pacific/Funafuti':['TV',-8.52,179.22],'Pacific/Galapagos':['EC',-0.9,-89.6],'Pacific/Gambier':['PF',-23.13,-134.95],'Pacific/Guadalcanal':['SB',-9.53,160.2],'Pacific/Guam':['GU',13.47,144.75],'Pacific/Honolulu':['US',21.31,-157.86],'Pacific/Kanton':['KI',-2.78,-171.72],'Pacific/Kiritimati':['KI',1.87,-157.33],'Pacific/Kosrae':['FM',5.32,162.98],'Pacific/Kwajalein':['MH',9.08,167.33],'Pacific/Majuro':['MH',7.15,171.2],'Pacific/Marquesas':['PF',-9.0,-139.5],'Pacific/Midway':['UM',28.22,-177.37],'Pacific/Nauru':['NR',-0.52,166.92],'Pacific/Niue':['NU',-19.02,-169.92],'Pacific/Norfolk':['NF',-29.05,167.97],'Pacific/Noumea':['NC',-22.27,166.45],'Pacific/Pago_Pago':['AS',-14.27,-170.7],'Pacific/Palau':['PW',7.33,134.48],'Pacific/Pitcairn':['PN',-25.07,-130.08],'Pacific/Pohnpei':['FM',6.97,158.22],'Pacific/Port_Moresby':['PG',-9.5,147.17],'Pacific/Rarotonga':['CK',-21.23,-159.77],'Pacific/Saipan':['MP',15.2,145.75],'Pacific/Tahiti':['PF',-17.53,-149.57],'Pacific/Tarawa':['KI',1.42,173.0],'Pacific/Tongatapu':['TO',-21.13,-175.2],'Pacific/Wake':['UM',19.28,166.62],'Pacific/Wallis':['WF',-13.3,-176.17]};
  const RAD = Math.PI / 180;
  const RAIZ = document.documentElement;
  const DEMO = RAIZ.dataset.demo === '1';
  const TIERRA = RAIZ.dataset.tierra || '/static/tierra/';
  const QUIETO = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const NUBES = new URLSearchParams(location.search).get('nubes') === '1';
  const TOKEN = (() => { try { return sessionStorage.getItem('guardiana_token') || ''; } catch (_) { return ''; } })();
  const $ = (id) => document.getElementById(id);

  // ---------- The world ----------
  const centros = {};
  P.trim().split(' | ').forEach((s) => { const [c, lon, lat] = s.trim().split(' '); if (!centros[c]) centros[c] = { lon: +lon, lat: +lat }; });
  PEQUENOS.forEach(([c, lon, lat]) => { if (!centros[c]) centros[c] = { lon, lat }; });
  const paisDelEquipo = () => {
    try { const z = Intl.DateTimeFormat().resolvedOptions().timeZone; if (ZONAS[z] && centros[ZONAS[z][0]]) return ZONAS[z][0]; } catch (_) {}
    try { const r = (navigator.languages || [navigator.language]).map((l) => (String(l).split('-')[1] || '').toUpperCase()).find((x) => centros[x]); if (r) return r; } catch (_) {}
    return 'US';
  };
  const CASA = paisDelEquipo();
  const nombrePais = (() => { let dn = null; try { dn = new Intl.DisplayNames(['es'], { type: 'region' }); } catch (_) {} return (c) => { try { return dn ? dn.of(c) : c; } catch (_) { return c; } }; })();
  const vec = (lon, lat) => { const a = lat * RAD, b = lon * RAD; return [Math.cos(a) * Math.sin(b), Math.sin(a), Math.cos(a) * Math.cos(b)]; };
  const punto = (c) => { const p = centros[c]; return p ? vec(p.lon, p.lat) : null; };
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const slerp = (a, b, t) => {
    const o = Math.acos(Math.max(-1, Math.min(1, dot(a, b))));
    if (o < 1e-4) return a.slice();
    const s = Math.sin(o), k1 = Math.sin((1 - t) * o) / s, k2 = Math.sin(t * o) / s;
    return [a[0] * k1 + b[0] * k2, a[1] * k1 + b[1] * k2, a[2] * k1 + b[2] * k2];
  };

  // ---------- The nine doors ----------
  // Five doors since 20 Oct 2026 (the owner: nine felt wide and heavy). Agreed by Claude, ChatGPT and
  // Grok: the nine functions stay, grouped, each reachable inside its door. While a door is being
  // built it says so; ZERO, the one being built now, opens as a preview.
  const PUERTAS = [
    { id: 'hoy', n: 'Hoy', s: 'En construcción', d: 'Los cortes de hoy y el historial completo, encadenado y auditable.', dentro: ['Cortes de hoy', 'Historial'], url: null, construccion: true },
    { id: 'proteccion', n: 'Protección', s: 'En construcción', d: 'Verificación de que lo instalado es lo publicado, y Defensa cuando esté probada.', dentro: ['Verificación', 'Defensa (en desarrollo)'], url: null, construccion: true },
    // ZERO is open (owner, 20 Oct 2026): the new start screen (zero.html), browsing for real in ZERO's own window.
    { id: 'zero', n: 'ZERO', s: 'Abierto', d: 'El navegador: busca donde quieras y mira lo que se corta en cada web.', dentro: ['Navegador', 'Buscadores', 'Cortes por página'], url: '/static/zero.html' },
    { id: 'casa', n: 'Casa', s: 'En construcción', d: 'Los dispositivos de tu red y la radiografía de este equipo.', dentro: ['Dispositivos', 'Radiografía'], url: null, construccion: true },
    { id: 'mensajes', n: 'Mensajes', s: 'En construcción', d: 'Mensajes y llamadas cifrados con GUARDIANA SECURE, y lo que sabe tu IA de ti.', dentro: ['Mensajes y llamadas', 'Tu IA'], url: null, construccion: true },
  ];
  const nav = $('puertas');
  const NP = PUERTAS.length;
  PUERTAS.forEach((p, i) => {
    const a = (i + 0.5) * 2 * Math.PI / NP, s = Math.sin(a), c = Math.cos(a);
    const el = document.createElement('div');
    el.className = 'puerta';
    el.style.left = `calc(50% + var(--anillo) * ${s.toFixed(4)})`;
    el.style.top = `calc(50% - var(--anillo) * ${c.toFixed(4)})`;
    el.style.transitionDelay = (1.3 + i * 0.08).toFixed(2) + 's';
    const b = document.createElement('button');
    b.type = 'button';
    b.setAttribute('aria-label', p.n + ' · ' + p.s);
    b.innerHTML = '<span class="p"></span>';
    const et = document.createElement('span');
    et.className = 'et';
    et.innerHTML = '';
    et.append(p.n);
    const sm = document.createElement('small');
    sm.textContent = p.s;
    if (p.desarrollo || p.construccion) sm.className = 'desarrollo';
    et.appendChild(sm);
    // Placed from the button's box (44 px, centred on the dot).
    if (s > 0.3) { et.style.left = '42px'; et.style.top = '9px'; }
    else if (s < -0.3) { et.style.right = '42px'; et.style.top = '9px'; et.style.textAlign = 'right'; }
    else if (c > 0) { et.style.left = '50%'; et.style.bottom = '38px'; et.style.transform = 'translateX(-50%)'; et.style.textAlign = 'center'; }
    else { et.style.left = '50%'; et.style.top = '38px'; et.style.transform = 'translateX(-50%)'; et.style.textAlign = 'center'; }
    b.addEventListener('mouseenter', () => elige(i));
    b.addEventListener('focus', () => elige(i));
    b.addEventListener('click', () => abre(i));
    // The name is part of the button: the whole door, dot and words, is one thing to press.
    b.appendChild(et);
    el.appendChild(b);
    nav.appendChild(el);
    p.el = el;
  });
  let sel = 0;
  function elige(i) {
    sel = (i + NP) % NP;
    PUERTAS.forEach((p, k) => p.el.classList.toggle('on', k === sel));
    const p = PUERTAS[sel];
    $('ficha-n').textContent = String(sel + 1).padStart(2, '0');
    $('ficha-de').textContent = '/' + String(NP).padStart(2, '0');
    $('ficha-k').textContent = p.s;
    $('ficha-t').textContent = p.n;
    $('ficha-d').textContent = p.d;
    $('ficha-a').setAttribute('aria-label', 'Abrir ' + p.n);
  }
  elige(0);
  $('ficha-a').addEventListener('click', (e) => { e.preventDefault(); abre(sel); });

  // ---------- A door opened, in the same screen ----------
  let dentro = false, limpio = false, limpioDesde = 0;
  const marco = $('marco');
  function abre(i, apila = true) {
    elige(i);
    const p = PUERTAS[sel];
    dentro = true;
    document.body.classList.add('dentro');
    // ZERO is clean (owner, 20 Oct 2026): no planet behind the browser, and the globe stops
    // drawing once it has faded out, so the web gets the whole machine.
    limpio = p.id === 'zero';
    document.body.classList.toggle('limpio', limpio);
    $('volver').hidden = false;
    $('donde').hidden = false;
    $('donde').textContent = p.n;
    // a door with its own show uses the common section (orbita-secciones.js)
    if (!p.url && window.GuardianaSecciones && window.GuardianaSecciones.abre(p.id)) {
      marco.hidden = true; marco.removeAttribute('src'); $('sin-pagina').hidden = true;
      if (apila) history.pushState({ puerta: p.id }, '', '#' + p.id);
      setTimeout(() => { if (dentro) $('volver').focus(); }, 900);
      return;
    }
    if (window.GuardianaSecciones) window.GuardianaSecciones.cierra();
    const real = p.url && !DEMO;
    marco.hidden = !real;
    $('sin-pagina').hidden = real;
    if (real) {
      marco.title = p.n;
      marco.src = p.url + (p.url.includes('?') ? '&' : '?') + 'orbita=1';
    } else {
      marco.removeAttribute('src');
      $('sp-k').textContent = p.construccion ? 'EN CONSTRUCCIÓN' : p.desarrollo ? 'EN DESARROLLO' : p.s.toUpperCase();
      $('sp-k').classList.toggle('desarrollo', !!(p.desarrollo || p.construccion));
      $('sp-t').textContent = p.n;
      $('sp-d').textContent = p.construccion ? p.d + ' Dentro: ' + (p.dentro || []).join(' · ') + '. Se abrirá cuando esté terminada.' : p.aparte ? p.d + ' ' + p.aparte : (DEMO && p.url ? p.d + ' En el programa, aquí entra la página real.' : p.d);
    }
    if (apila) history.pushState({ puerta: p.id }, '', '#' + p.id);
    setTimeout(() => { if (dentro) $('volver').focus(); }, 900);
  }
  function cierra() {
    if (!dentro) return;
    dentro = false;
    limpio = false;
    document.body.classList.remove('dentro', 'limpio');
    $('volver').hidden = true;
    $('donde').hidden = true;
    setTimeout(() => { if (!dentro) { marco.removeAttribute('src'); if (window.GuardianaSecciones) window.GuardianaSecciones.cierra(); } }, 800);
    setTimeout(() => { const b = PUERTAS[sel].el.querySelector('button'); if (!dentro && b) b.focus({ preventScroll: true }); }, 700);
  }
  const atras = () => { if (history.state && history.state.puerta) history.back(); else cierra(); };
  $('volver').addEventListener('click', atras);
  addEventListener('popstate', (e) => {
    const id = e.state && e.state.puerta;
    const i = PUERTAS.findIndex((p) => p.id === id);
    if (i >= 0) abre(i, false); else cierra();
  });
  addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !caja.hidden) { e.preventDefault(); cierraLugar(false); return; }
    if (!caja.hidden) return;
    if (e.key === 'Escape' && dentro) { e.preventDefault(); atras(); return; }
    if (dentro || e.altKey || e.ctrlKey || e.metaKey) return;
    if (e.target && /INPUT|TEXTAREA|SELECT/.test(e.target.tagName)) return;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') { e.preventDefault(); elige(sel + 1); PUERTAS[sel].el.querySelector('button').focus(); }
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') { e.preventDefault(); elige(sel - 1); PUERTAS[sel].el.querySelector('button').focus(); }
  });
  { const i = PUERTAS.findIndex((p) => '#' + p.id === location.hash); if (i >= 0) { history.replaceState({ puerta: PUERTAS[i].id }, '', location.hash); setTimeout(() => abre(i, false), 400); } }

  // ---------- Geometry shared with the CSS (--u) ----------
  let W = 0, H = 0, DPR = 1, U = 1, CX = 0, CY = 0, R = 1;
  const cvE = $('estrellas'), cvT = $('tierra'), cvR = $('rutas');
  const cx2 = cvR.getContext('2d');
  function mide() {
    W = innerWidth; H = innerHeight; DPR = Math.min(devicePixelRatio || 1, 2);
    // The ring's unit. Wide: «En directo» and the door's reading take the two lower corners, so the
    // ring stays clear of them (the lower doors' labels reach 26.4 units out and 130 px beyond).
    // Narrow: both move off the corners and the ring keeps clear of the top and bottom lines.
    const estrecho = W < 1180;
    document.body.classList.toggle('estrecho', estrecho);
    if (estrecho) U = Math.max(3, Math.min((H / 2 - 160) / 41, (H / 2 - 150) / 41, (W / 2 - 150) / 40.4));
    else U = Math.max(3, Math.min(H / 100, W * 0.0062, (W / 2 - 40 - 290 - 24 - 130) / 26.4, (H / 2 - 120) / 41));
    RAIZ.style.setProperty('--u', U.toFixed(3) + 'px');
    CX = W / 2; CY = H / 2; R = U * 31;
    for (const c of [cvE, cvT, cvR]) { c.width = Math.round(W * DPR); c.height = Math.round(H * DPR); }
    cx2.setTransform(DPR, 0, 0, DPR, 0, 0);
    pintaEstrellas();
  }
  function pintaEstrellas() {
    const g = cvE.getContext('2d');
    g.setTransform(DPR, 0, 0, DPR, 0, 0);
    g.fillStyle = '#06080C'; g.fillRect(0, 0, W, H);
    let s = 7;
    const rnd = () => { s = (s * 16807) % 2147483647; return s / 2147483647; };
    const n = Math.round(W * H / 2600);
    for (let i = 0; i < n; i++) {
      const x = rnd() * W, y = rnd() * H, m = rnd();
      const r = m > 0.985 ? 1.3 : m > 0.9 ? 0.85 : 0.5;
      g.fillStyle = `rgba(${200 + rnd() * 40 | 0},${210 + rnd() * 30 | 0},255,${(0.18 + rnd() * 0.55).toFixed(2)})`;
      g.beginPath(); g.arc(x, y, r, 0, 7); g.fill();
      if (m > 0.993) { const h = g.createRadialGradient(x, y, 0, x, y, 7); h.addColorStop(0, 'rgba(200,220,255,.35)'); h.addColorStop(1, 'rgba(200,220,255,0)'); g.fillStyle = h; g.fillRect(x - 7, y - 7, 14, 14); }
    }
  }

  // ---------- The view: where the planet looks ----------
  const vista = { lon: (centros[CASA] ? centros[CASA].lon : -40) + 18, lat: 14, vlon: 0, vlat: 0 };
  let meta = null;           // a country the person asked to see (a row of «En directo»)
  let arrastre = null, quietoHasta = 0;
  const VEL = QUIETO ? 1.2 : 2.6; // degrees per second
  // view → world: Ry(lon) · Rx(−lat)
  const aMundo = (v) => {
    const a = vista.lat * RAD, b = vista.lon * RAD, ca = Math.cos(a), sa = Math.sin(a), cb = Math.cos(b), sb = Math.sin(b);
    const y = v[1] * ca + v[2] * sa, z = -v[1] * sa + v[2] * ca;
    return [v[0] * cb + z * sb, y, -v[0] * sb + z * cb];
  };
  const aVista = (w) => {
    const a = vista.lat * RAD, b = vista.lon * RAD, ca = Math.cos(a), sa = Math.sin(a), cb = Math.cos(b), sb = Math.sin(b);
    const x = w[0] * cb - w[2] * sb, z = w[0] * sb + w[2] * cb;
    return [x, w[1] * ca - z * sa, w[1] * sa + z * ca];
  };
  // A world point (unit sphere times its height) on the screen; `ve` false when the Earth hides it.
  const pantalla = (w) => {
    const v = aVista(w);
    return { x: CX + R * v[0], y: CY - R * v[1], ve: v[2] > 0 || v[0] * v[0] + v[1] * v[1] > 1.0, z: v[2] };
  };
  cvT.addEventListener('pointerdown', (e) => {
    const dx = e.clientX - CX, dy = e.clientY - CY;
    if (dx * dx + dy * dy > R * R * 1.15) return;
    arrastre = { x: e.clientX, y: e.clientY, t: performance.now() };
    meta = null; cvT.classList.add('arrastra'); cvT.setPointerCapture(e.pointerId);
  });
  cvT.addEventListener('pointermove', (e) => {
    if (!arrastre) return;
    const now = performance.now(), dt = Math.max(8, now - arrastre.t);
    const dx = e.clientX - arrastre.x, dy = e.clientY - arrastre.y;
    const k = 57 / R;
    vista.lon -= dx * k; vista.lat = Math.max(-60, Math.min(60, vista.lat + dy * k));
    vista.vlon = -dx * k / dt * 1000; vista.vlat = dy * k / dt * 1000;
    arrastre = { x: e.clientX, y: e.clientY, t: now };
  });
  const suelta = () => { if (!arrastre) return; arrastre = null; cvT.classList.remove('arrastra'); quietoHasta = performance.now() + 2500; };
  cvT.addEventListener('pointerup', suelta); cvT.addEventListener('pointercancel', suelta);
  function mira(pais, punto) {
    const c = punto || centros[pais]; if (!c) return;
    meta = { lon: c.lon, lat: Math.max(-45, Math.min(45, c.lat)) };
    quietoHasta = performance.now() + 6000;
  }
  function avanzaVista(dt, now) {
    if (arrastre) return;
    if (meta) {
      let d = ((meta.lon - vista.lon) % 360 + 540) % 360 - 180;
      vista.lon += d * Math.min(1, dt * 2.2); vista.lat += (meta.lat - vista.lat) * Math.min(1, dt * 2.2);
      if (Math.abs(d) < 0.05 && Math.abs(meta.lat - vista.lat) < 0.05) meta = null;
      return;
    }
    vista.lon += vista.vlon * dt; vista.lat = Math.max(-60, Math.min(60, vista.lat + vista.vlat * dt));
    vista.vlon *= Math.pow(0.04, dt); vista.vlat *= Math.pow(0.04, dt);
    if (now > quietoHasta) { vista.lon += VEL * dt; vista.lat += (14 - vista.lat) * dt * 0.25; }
  }

  // ---------- The planet (WebGL 1) ----------
  const gl = cvT.getContext('webgl', { alpha: true, premultipliedAlpha: true, antialias: false });
  const ONDAS = 8;
  const ondas = []; // { w: world point, t0, corte }
  let G = null;
  if (gl) {
    const vs = 'attribute vec2 a;void main(){gl_Position=vec4(a,0.,1.);}';
    const fs = `precision highp float;
uniform vec2 uC;uniform float uR;uniform mat3 uM;uniform vec3 uL;uniform float uT,uNubOn,uAten;
uniform sampler2D uDia,uLuz,uNub,uAgua;uniform vec4 uO[${ONDAS}];uniform float uOk[${ONDAS}];
const float PI=3.14159265;
void main(){
 vec2 p=(gl_FragCoord.xy-uC)/uR;float r=length(p);vec3 atm=vec3(.47,.67,.92);
 vec3 col=vec3(0.);float al=0.;
 if(r<1.){
  vec3 n=vec3(p,sqrt(max(0.,1.-r*r)));vec3 w=uM*n;
  float lat=asin(clamp(w.y,-1.,1.));float lon=atan(w.x,w.z);
  vec2 uv=vec2(lon/(2.*PI)+.5,.5-lat/PI);
  vec3 dia=texture2D(uDia,uv).rgb;float agua=texture2D(uAgua,uv).r;
  float nub=texture2D(uNub,vec2(fract(uv.x-uT*.0016),uv.y)).r*uNubOn;nub=smoothstep(.12,.9,nub);
  vec3 luz=texture2D(uLuz,uv).rgb;
  float lam=dot(n,uL);float dl=smoothstep(-.22,.42,lam);
  vec3 sup=mix(dia*vec3(.96,1.,1.06),vec3(.97,.98,1.),nub*.88);
  vec3 c=sup*(.05+1.08*max(lam,0.))*mix(.3,1.,dl);
  c+=luz*vec3(1.,.78,.48)*1.5*(1.-dl)*(1.-nub*.8);
  vec3 h=normalize(uL+vec3(0.,0.,1.));c+=vec3(.8,.9,1.)*pow(max(dot(n,h),0.),160.)*agua*(1.-nub)*.32;
  float fr=pow(1.-n.z,2.4);c=mix(c,atm*(.18+.8*max(lam+.35,0.)),fr*.6);
  for(int i=0;i<${ONDAS};i++){vec4 o=uO[i];if(o.w>0.&&o.w<1.){
   float d=acos(clamp(dot(w,o.xyz),-1.,1.));float rd=.02+o.w*.32;float f=1.-o.w;
   float ring=exp(-pow((d-rd)/(.008+.01*o.w),2.))*f*f;float core=exp(-d*d/.0008)*f*f*f;
   vec3 oc=uOk[i]>.5?vec3(.95,.5,.42):vec3(.66,.82,.96);c+=oc*(ring*1.25+core*2.);}}
  float e=smoothstep(1.,1.-1.6/uR,r);col=c*uAten*e;al=e;
 }
 float h1=r>1.?exp(-(r-1.)/.05)*.5:0.;float h2=r>1.?exp(-(r-1.)/.011)*.55:0.;
 vec3 hc=atm*(h1+h2)*uAten;
 gl_FragColor=vec4(col+hc*(1.-al),al+(h1+h2)*.9*(1.-al));
}`;
    const sh = (t, src) => { const s = gl.createShader(t); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
    try {
      const pr = gl.createProgram();
      gl.attachShader(pr, sh(gl.VERTEX_SHADER, vs)); gl.attachShader(pr, sh(gl.FRAGMENT_SHADER, fs)); gl.linkProgram(pr);
      gl.useProgram(pr);
      const b = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, b);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
      const la = gl.getAttribLocation(pr, 'a'); gl.enableVertexAttribArray(la); gl.vertexAttribPointer(la, 2, gl.FLOAT, false, 0, 0);
      const u = {}; ['uC', 'uR', 'uM', 'uL', 'uT', 'uNubOn', 'uAten', 'uDia', 'uLuz', 'uNub', 'uAgua', 'uO', 'uOk'].forEach((k) => { u[k] = gl.getUniformLocation(pr, k); });
      G = { pr, u, tex: {}, nubOn: 0 };
      const tex = (unidad, nombre, url, si) => {
        const t = gl.createTexture(); gl.activeTexture(gl.TEXTURE0 + unidad); gl.bindTexture(gl.TEXTURE_2D, t);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([10, 14, 20, 255]));
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        gl.uniform1i(u[nombre], unidad);
        const im = new Image();
        im.onload = () => { gl.activeTexture(gl.TEXTURE0 + unidad); gl.bindTexture(gl.TEXTURE_2D, t); gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, im); if (si) si(); };
        im.src = url;
      };
      tex(0, 'uDia', TIERRA + 'dia.jpg', () => listo());
      tex(1, 'uLuz', TIERRA + 'luces.jpg');
      // The clouds are off since 19 Oct 2026 (the owner: «para que se vea más claro»); `?nubes=1`
      // brings them back to compare.
      if (NUBES) tex(2, 'uNub', TIERRA + 'nubes.jpg', () => { G.nubOn = 1; });
      else { gl.uniform1i(u.uNub, 0); }
      tex(3, 'uAgua', TIERRA + 'agua.png');
    } catch (e) { G = null; }
  }
  const L = (() => { const v = [-0.5, 0.36, 0.79], m = Math.hypot(...v); return v.map((x) => x / m); })();
  function pintaTierra(t, aten) {
    if (!G) return;
    gl.viewport(0, 0, cvT.width, cvT.height);
    gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
    const u = G.u;
    gl.uniform2f(u.uC, CX * DPR, (H - CY) * DPR);
    gl.uniform1f(u.uR, R * DPR);
    const c0 = aMundo([1, 0, 0]), c1 = aMundo([0, 1, 0]), c2 = aMundo([0, 0, 1]);
    gl.uniformMatrix3fv(u.uM, false, [...c0, ...c1, ...c2]);
    gl.uniform3f(u.uL, L[0], L[1], L[2]);
    gl.uniform1f(u.uT, t); gl.uniform1f(u.uNubOn, G.nubOn); gl.uniform1f(u.uAten, aten);
    const o = new Float32Array(ONDAS * 4), k = new Float32Array(ONDAS);
    ondas.slice(-ONDAS).forEach((x, i) => { const edad = (t - x.t0) / (x.corte ? 2.6 : 1.8); o.set([...x.w, edad], i * 4); k[i] = x.corte ? 1 : 0; });
    gl.uniform4fv(u.uO, o); gl.uniform1fv(u.uOk, k);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  // ---------- Flights, cuts and their traces ----------
  const vuelos = [], huellas = [], chispas = [], rotulos = [], destinos = new Map();
  const ROJO = '224,122,108', ACERO = '156,195,228', BLANCO = '232,236,241';
  function lanza(ev, t) {
    const de = origen(), a = punto(ev.pais);
    if (!de || !a) return;
    const ang = Math.acos(Math.max(-1, Math.min(1, dot(de, a))));
    const alt = 0.05 + 0.24 * ang / Math.PI;
    const N = 72, ruta = [];
    for (let i = 0; i <= N; i++) { const s = i / N, p = slerp(de, a, s), h = 1 + alt * Math.sin(Math.PI * s); ruta.push([p[0] * h, p[1] * h, p[2] * h]); }
    const dur = (1.5 + ang * 0.9) * (QUIETO ? 1.6 : 1);
    vuelos.push({ ev, ruta, t0: t, dur, tc: ev.corte ? 0.4 + Math.random() * 0.14 : 1, hecho: false, a });
  }
  const sobre = (ruta, s) => { const f = s * (ruta.length - 1), i = Math.min(ruta.length - 2, Math.floor(f)), k = f - i, p = ruta[i], q = ruta[i + 1]; return [p[0] + (q[0] - p[0]) * k, p[1] + (q[1] - p[1]) * k, p[2] + (q[2] - p[2]) * k]; };
  function estela(ruta, s0, s1, color, ancho, alfa) {
    trazo(ruta, s0, s1, color, ancho * 3.2, alfa * 0.18, true);
    trazo(ruta, s0, s1, color, ancho, alfa, true);
  }
  function trazo(ruta, s0, s1, color, ancho, alfa, cola) {
    const N = ruta.length - 1, i0 = Math.max(0, Math.floor(s0 * N)), i1 = Math.min(N, Math.ceil(s1 * N));
    let prev = null;
    for (let i = i0; i <= i1; i++) {
      const s = Math.max(s0, Math.min(s1, i / N));
      const q = pantalla(sobre(ruta, s));
      if (prev && prev.ve && q.ve) {
        const f = cola ? (s - s0) / Math.max(1e-3, s1 - s0) : 1;
        cx2.strokeStyle = `rgba(${color},${(alfa * f * f).toFixed(3)})`;
        cx2.lineWidth = ancho * (cola ? 0.35 + 0.65 * f : 1);
        cx2.beginPath(); cx2.moveTo(prev.x, prev.y); cx2.lineTo(q.x, q.y); cx2.stroke();
      }
      prev = q;
    }
  }
  function cabeza(q, color, r) {
    const g = cx2.createRadialGradient(q.x, q.y, 0, q.x, q.y, r * 6);
    g.addColorStop(0, `rgba(${BLANCO},.95)`); g.addColorStop(0.18, `rgba(${color},.7)`); g.addColorStop(1, `rgba(${color},0)`);
    cx2.fillStyle = g; cx2.beginPath(); cx2.arc(q.x, q.y, r * 6, 0, 7); cx2.fill();
  }
  function revienta(v, t) {
    const w = sobre(v.ruta, v.tc);
    const m = Math.hypot(...w);
    ondas.push({ w: w.map((x) => x / m), t0: t, corte: true });
    if (!QUIETO) for (let i = 0; i < 26; i++) {
      const a = Math.random() * 7, s = 30 + Math.random() * 150;
      chispas.push({ w, vx: Math.cos(a) * s, vy: Math.sin(a) * s, t0: t, vida: 0.6 + Math.random() * 0.7 });
    }
    rotulos.push({ w, t0: t, ev: v.ev });
    if (rotulos.length > 3) rotulos.shift();
    huellas.push({ ruta: v.ruta, s1: v.tc, t0: t, corte: true });
    anota(v.ev);
    late();
  }
  function llega(v, t) {
    ondas.push({ w: v.a, t0: t, corte: false });
    huellas.push({ ruta: v.ruta, s1: 1, t0: t, corte: false });
    const d = destinos.get(v.ev.pais) || { w: v.a, n: 0, t: t };
    d.n++; d.t = t; destinos.set(v.ev.pais, d);
  }
  function pintaRutas(t, dt) {
    cx2.clearRect(0, 0, W, H);
    cx2.lineCap = 'round';
    cx2.globalCompositeOperation = 'lighter';
    // the traces of the last half minute: the day's web, fading
    for (let i = huellas.length - 1; i >= 0; i--) {
      const h = huellas[i], e = (t - h.t0) / 30;
      if (e >= 1) { huellas.splice(i, 1); continue; }
      trazo(h.ruta, 0, h.s1, h.corte ? ROJO : ACERO, 1, (h.corte ? 0.22 : 0.16) * (1 - e), false);
    }
    while (huellas.length > 70) huellas.shift();
    // the countries reached: a calm point each
    for (const d of destinos.values()) {
      const q = pantalla(d.w); if (!q.ve || q.z < 0) continue;
      const r = 1.6 + Math.min(4, Math.log2(1 + d.n)), f = Math.max(0.35, 1 - (t - d.t) / 60);
      const g = cx2.createRadialGradient(q.x, q.y, 0, q.x, q.y, r * 3.5);
      g.addColorStop(0, `rgba(${ACERO},${(0.75 * f).toFixed(2)})`); g.addColorStop(1, `rgba(${ACERO},0)`);
      cx2.fillStyle = g; cx2.beginPath(); cx2.arc(q.x, q.y, r * 3.5, 0, 7); cx2.fill();
    }
    // the flights
    for (let i = vuelos.length - 1; i >= 0; i--) {
      const v = vuelos[i], s = (t - v.t0) / v.dur;
      if (v.ev.corte) {
        const sh = Math.min(s, v.tc);
        if (s < v.tc) {
          estela(v.ruta, Math.max(0, sh - 0.28), sh, ACERO, 2.4, 1);
          const q = pantalla(sobre(v.ruta, sh)); if (q.ve) cabeza(q, ACERO, 3);
        } else {
          if (!v.hecho) { v.hecho = true; revienta(v, t); }
          const e = (t - v.t0 - v.tc * v.dur) / 2.8;
          if (e >= 1) { vuelos.splice(i, 1); continue; }
          // where it wanted to go: a dotted line that fades
          cx2.setLineDash([2, 6]); trazo(v.ruta, v.tc, 1, BLANCO, 1, 0.35 * (1 - e), false); cx2.setLineDash([]);
          estela(v.ruta, Math.max(0, v.tc - 0.28 * (1 - e)), v.tc, ROJO, 2.4, 0.95 * (1 - e));
          const q = pantalla(sobre(v.ruta, v.tc));
          if (q.ve) {
            const r1 = 6 + 34 * Math.min(1, e * 2.2), r2 = 4 + 60 * e;
            cx2.strokeStyle = `rgba(${ROJO},${(0.9 * (1 - Math.min(1, e * 2.2))).toFixed(3)})`; cx2.lineWidth = 2;
            cx2.beginPath(); cx2.arc(q.x, q.y, r1, 0, 7); cx2.stroke();
            cx2.strokeStyle = `rgba(${ROJO},${(0.35 * (1 - e)).toFixed(3)})`; cx2.lineWidth = 1;
            cx2.beginPath(); cx2.arc(q.x, q.y, r2, 0, 7); cx2.stroke();
            const k = 4.5, a = 1 - e;
            cx2.strokeStyle = `rgba(${ROJO},${a.toFixed(3)})`; cx2.lineWidth = 1.8;
            cx2.beginPath(); cx2.moveTo(q.x - k, q.y - k); cx2.lineTo(q.x + k, q.y + k); cx2.moveTo(q.x + k, q.y - k); cx2.lineTo(q.x - k, q.y + k); cx2.stroke();
          }
        }
      } else {
        if (s < 1) {
          estela(v.ruta, Math.max(0, s - 0.3), s, ACERO, 2, 0.9);
          const q = pantalla(sobre(v.ruta, s)); if (q.ve) cabeza(q, ACERO, 2.4);
        } else {
          if (!v.hecho) { v.hecho = true; llega(v, t); }
          const e = (t - v.t0 - v.dur) / 0.7;
          if (e >= 1) { vuelos.splice(i, 1); continue; }
          trazo(v.ruta, Math.max(0, 1 - 0.26 * (1 - e)), 1, ACERO, 1.8, 0.85 * (1 - e), true);
        }
      }
    }
    // sparks of the cuts
    for (let i = chispas.length - 1; i >= 0; i--) {
      const c = chispas[i], e = (t - c.t0) / c.vida;
      if (e >= 1) { chispas.splice(i, 1); continue; }
      const q = pantalla(c.w); if (!q.ve) continue;
      const tt = t - c.t0, x = q.x + c.vx * tt * (1 - tt * 0.6), y = q.y + c.vy * tt * (1 - tt * 0.6) + 30 * tt * tt;
      cx2.fillStyle = `rgba(${e < 0.3 ? '255,214,190' : ROJO},${(1 - e).toFixed(3)})`;
      cx2.beginPath(); cx2.arc(x, y, 1.4 * (1 - e) + 0.4, 0, 7); cx2.fill();
    }
    // the computer: at its country, said to be approximate
    const qc = pantalla(origen() || [0, 0, 1]);
    if (qc.ve && qc.z > 0) {
      const f = (t % 2.4) / 2.4;
      cx2.strokeStyle = `rgba(${ACERO},${(0.7 * (1 - f)).toFixed(3)})`; cx2.lineWidth = 1.2;
      cx2.beginPath(); cx2.arc(qc.x, qc.y, 4 + 16 * f, 0, 7); cx2.stroke();
      cabeza(qc, ACERO, 1.8);
    }
    cx2.globalCompositeOperation = 'source-over';
    if (qc.ve && qc.z > 0) {
      cx2.font = '500 10px "IBM Plex Mono", monospace'; cx2.fillStyle = 'rgba(232,236,241,.82)';
      cx2.textAlign = 'left'; cx2.fillText(etiquetaCasa(), qc.x + 12, qc.y + 4);
    }
    // what was cut, said in words next to where it happened
    const usados = [];
    for (let i = rotulos.length - 1; i >= 0; i--) {
      const r = rotulos[i], e = t - r.t0;
      if (e > 4.2) { rotulos.splice(i, 1); continue; }
      const q = pantalla(r.w); if (!q.ve) continue;
      const a = Math.min(1, e / 0.25) * Math.min(1, (4.2 - e) / 0.7);
      let x = q.x + 26, y = q.y - 44;
      for (const u of usados) if (Math.abs(u.x - x) < 220 && Math.abs(u.y - y) < 60) y = u.y - 64;
      usados.push({ x, y });
      rotulo(q, x, y, r.ev, a);
    }
  }
  function rotulo(q, x, y, ev, a) {
    const l1 = 'CORTADO · ' + hora(ev.cuando), l2 = ev.quien, l3 = (ev.programa ? ev.programa + ' → ' : '') + nombrePais(ev.pais) + (ev.categoria ? ' · ' + ev.categoria : '');
    cx2.font = '600 13px "Hanken Grotesk", sans-serif'; const w2 = cx2.measureText(l2).width;
    cx2.font = '400 11px "Hanken Grotesk", sans-serif'; const w3 = cx2.measureText(l3).width;
    const w = Math.max(w2, w3, 110) + 22, h = 56;
    if (x + w > W - 12) x = q.x - 26 - w;
    cx2.globalAlpha = a;
    cx2.strokeStyle = `rgba(${ROJO},.6)`; cx2.lineWidth = 1;
    cx2.beginPath(); cx2.moveTo(q.x, q.y); cx2.lineTo(x < q.x ? x + w : x, y + h / 2); cx2.stroke();
    cx2.fillStyle = 'rgba(8,11,16,.84)'; cx2.strokeStyle = 'rgba(224,122,108,.45)';
    cx2.beginPath(); if (cx2.roundRect) cx2.roundRect(x, y, w, h, 10); else cx2.rect(x, y, w, h); cx2.fill(); cx2.stroke();
    cx2.textAlign = 'left';
    cx2.font = '500 9.5px "IBM Plex Mono", monospace'; cx2.fillStyle = `rgb(${ROJO})`; cx2.fillText(l1, x + 11, y + 16);
    cx2.font = '600 13px "Hanken Grotesk", sans-serif'; cx2.fillStyle = `rgb(${BLANCO})`; cx2.fillText(l2, x + 11, y + 33);
    cx2.font = '400 11px "Hanken Grotesk", sans-serif'; cx2.fillStyle = '#A9B3BE'; cx2.fillText(l3, x + 11, y + 48);
    cx2.globalAlpha = 1;
  }
  const hora = (ms) => { const d = new Date(ms || Date.now()); return String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0'); };

  // ---------- «En directo»: the list, and the ring answering ----------
  const lista = $('lista');
  function anota(ev) {
    $('espera').hidden = true;
    const li = document.createElement('li');
    const b = document.createElement('button');
    b.type = 'button';
    const q = document.createElement('span'); q.className = 'q';
    const i = document.createElement('i'); i.textContent = 'CORTADO'; q.append(i, ev.quien);
    const h = document.createElement('span'); h.className = 'h'; h.textContent = hora(ev.cuando);
    const w = document.createElement('span'); w.className = 'w'; w.textContent = (ev.programa ? ev.programa + ' → ' : '') + nombrePais(ev.pais) + (ev.categoria ? ' · ' + ev.categoria : '');
    b.append(q, h, w);
    b.setAttribute('aria-label', 'Cortado: ' + ev.quien + ', ' + w.textContent + '. Ver en el planeta');
    b.addEventListener('click', () => mira(ev.pais));
    li.appendChild(b);
    lista.insertBefore(li, lista.firstChild);
    while (lista.children.length > 4) lista.lastChild.remove();
    if (DEMO) { cortesDemo++; $('n').textContent = cortesDemo.toLocaleString('es-ES'); }
  }
  function late() {
    const p = $('pulso'); p.classList.remove('va'); void p.offsetWidth; p.classList.add('va');
    const d = PUERTAS[0].el; d.classList.remove('late'); void d.offsetWidth; d.classList.add('late');
  }

  // ---------- Where the events come from ----------
  const CATEGORIA = { publicidad: 'Publicidad', rastreador: 'Rastreador', telemetria: 'Telemetría', esperado: '', desconocido: '' };
  const cola = []; // events waiting their moment, so a take of five seconds is spread over them
  const encola = (evs, ventana) => { evs.forEach((e) => { e.at = performance.now() / 1000 + Math.random() * ventana; cola.push(e); }); cola.sort((a, b) => a.at - b.at); };
  let cortesDemo = 0;
  if (DEMO) {
    $('demo').hidden = false;
    $('n').textContent = '0';
    $('cuenta').firstChild.textContent = 'Cortes de la demostración ';
    const CORTES = [
      ['Rastreador publicitario', 'Navegador', 'US', 'Rastreador'], ['Corredor de datos', 'Navegador', 'IE', 'Publicidad'],
      ['Telemetría del televisor', 'Televisor', 'KR', 'Telemetría'], ['Medición de audiencia', 'Teléfono', 'DE', 'Rastreador'],
      ['Red publicitaria', 'Navegador', 'NL', 'Publicidad'], ['Telemetría de una app', 'Teléfono', 'SG', 'Telemetría'],
      ['Perfilado para anuncios', 'Navegador', 'GB', 'Publicidad'], ['Huella del dispositivo', 'Este PC', 'FR', 'Rastreador'],
      ['Analítica de terceros', 'Teléfono', 'JP', 'Rastreador'], ['Subasta de anuncios', 'Navegador', 'SE', 'Publicidad'],
    ];
    const PASAN = ['US', 'DE', 'NL', 'FR', 'GB', 'BR', 'JP', 'SG', 'IE', 'SE', 'CA', 'ES', 'IN', 'AU', 'ZA', 'MX', 'CL', 'AR'];
    const genera = () => {
      const corte = Math.random() < 0.42;
      if (corte) { const c = CORTES[Math.random() * CORTES.length | 0]; encola([{ corte: true, quien: c[0], programa: c[1], pais: c[2], categoria: c[3], cuando: Date.now() }], 0); }
      else encola([{ corte: false, pais: PASAN[Math.random() * PASAN.length | 0] }], 0);
      setTimeout(genera, 650 + Math.random() * 900);
    };
    setTimeout(genera, 2600);
  } else {
    let previo = null, fallos = 0;
    const valido = (p) => /^[A-Z]{2}$/.test(p || '') && centros[p];
    const evento = (d, corte) => ({ corte, pais: d.pais, quien: d.empresa || d.red || d.destino || d.programa, programa: d.programa, categoria: CATEGORIA[d.categoria] || '', cuando: d.ultima || Date.now() });
    const toma = async () => {
      try {
        const r = await fetch('/api/por-dentro', { headers: { 'X-Guardiana-Token': TOKEN, 'X-Guardiana-Lang': 'es', 'X-Guardiana-Tz': String(-new Date().getTimezoneOffset()) } });
        if (!r.ok) throw new Error(String(r.status));
        const u = await r.json();
        fallos = 0;
        const mapa = new Map(); let total = 0;
        for (const d of u.destinos || []) { mapa.set(d.programa + '|' + d.destino + '|' + d.pais, d); total += d.paradas || 0; }
        $('n').textContent = total.toLocaleString('es-ES');
        const cortes = [], pasan = [];
        if (!previo) {
          // the first take: the last minutes, so the planet opens already alive
          for (const d of mapa.values()) if (d.paradas > 0 && valido(d.pais) && u.ahora - d.ultima < 10 * 60e3) cortes.push(evento(d, true));
          for (const x of (u.recientes || []).slice(-14)) if (valido(x.pais)) pasan.push(evento(x, false));
        } else {
          for (const [k, d] of mapa) {
            if (!valido(d.pais)) continue;
            const a = previo.get(k);
            const dp = (d.paradas || 0) - (a ? a.paradas || 0 : 0), de = (d.enviados || 0) - (a ? a.enviados || 0 : 0);
            if (dp > 0) for (let i = 0; i < Math.min(dp, 3); i++) cortes.push(evento(d, true));
            else if (de > 0) pasan.push(evento(d, false));
          }
        }
        previo = mapa;
        encola(cortes.sort(() => Math.random() - 0.5).slice(0, 10).concat(pasan.sort(() => Math.random() - 0.5).slice(0, 14)), 4.6);
        if (!cortes.length && !lista.children.length) $('espera').textContent = 'Sin cortes en este momento. Cada uno aparecerá aquí y sobre el planeta.';
      } catch (e) {
        if (++fallos === 2) $('espera').textContent = 'No llegan las medidas del panel. El planeta sigue, sin dibujar nada que no se haya medido.';
      }
      setTimeout(toma, 5000);
    };
    toma();
  }


  // ---------- Where this computer is: its city (19 Oct 2026) ----------
  // Agreed by Claude, ChatGPT and Grok after the owner asked for «la ciudad, sí o sí»: the system's
  // own location, several samples, the best one, a city named only when it is within 5 km; or the
  // city the person chooses from the list that travels with the program. Always confirmed by the
  // person, kept by the service for the whole program (/api/ajustes/lugar), never an IP address,
  // never sent anywhere. Until then the computer is drawn at its country and says «aprox.».
  const PRECISION_CIUDAD = 5000;
  let LUGAR = null;
  const origen = () => (LUGAR ? vec(LUGAR.lon, LUGAR.lat) : punto(CASA));
  const etiquetaCasa = () => (LUGAR ? 'TU EQUIPO · ' + LUGAR.ciudad.toUpperCase() : 'TU EQUIPO · ' + (nombrePais(CASA) || CASA).toUpperCase() + ' (APROX.)');
  const apiLugar = async (metodo, cuerpo) => {
    if (DEMO) {
      if (metodo === 'PUT') { try { localStorage.setItem('guardiana_orbita_demo_lugar', JSON.stringify(cuerpo)); } catch (_) {} return cuerpo; }
      if (metodo === 'DELETE') { try { localStorage.removeItem('guardiana_orbita_demo_lugar'); } catch (_) {} return null; }
      try { return JSON.parse(localStorage.getItem('guardiana_orbita_demo_lugar') || 'null'); } catch (_) { return null; }
    }
    const r = await fetch('/api/ajustes/lugar', { method: metodo, headers: Object.assign({ 'X-Guardiana-Token': TOKEN }, cuerpo ? { 'Content-Type': 'application/json' } : {}), body: cuerpo ? JSON.stringify(cuerpo) : undefined });
    if (!r.ok) throw new Error(String(r.status));
    return (await r.json()).lugar || null;
  };
  let CIUDADES = null;
  const ciudades = async () => {
    if (CIUDADES) return CIUDADES;
    const r = await fetch(TIERRA + 'ciudades.tsv');
    if (!r.ok) throw new Error(String(r.status));
    const sinTildes = (s) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
    CIUDADES = (await r.text()).split('\n').filter((l) => l && l[0] !== '#').map((l) => { const [n, cc, adm, la, lo] = l.split('\t'); return { n, cc, adm: adm || '', la: +la, lo: +lo, k: sinTildes(n + ' ' + (adm || '')) }; });
    CIUDADES.sinTildes = sinTildes;
    return CIUDADES;
  };
  const cercana = (lista, lat, lon) => {
    let mejor = null, dm = Infinity;
    const cl = Math.cos(lat * RAD);
    for (const c of lista) { const dx = (c.lo - lon) * cl, dy = c.la - lat, d = dx * dx + dy * dy; if (d < dm) { dm = d; mejor = c; } }
    return mejor ? { c: mejor, km: Math.sqrt(dm) * 111.2 } : null;
  };
  const distKm = (a, b) => { const cl = Math.cos(a.lat * RAD), dx = (a.lon - b.lon) * cl, dy = a.lat - b.lat; return Math.sqrt(dx * dx + dy * dy) * 111.2; };
  const metros = (m) => (m < 1000 ? Math.max(1, Math.round(m)) + ' m' : (m / 1000).toFixed(m < 10000 ? 1 : 0).replace('.', ',') + ' km');
  function pintaLugar() {
    const b = $('lugar-btn');
    $('lugar-nombre').textContent = LUGAR ? LUGAR.ciudad : 'Situar este equipo';
    b.classList.toggle('falta', !LUGAR);
    b.setAttribute('aria-label', LUGAR ? 'Este equipo está en ' + LUGAR.ciudad + '. Cambiar' : 'Situar este equipo en su ciudad');
  }
  // The card: one calm question over the planet, in a few states.
  const caja = $('lugar');
  let candidato = null, medicion = null, antesDelLugar = null;
  const accion = (texto, clase, hacer) => { const b = document.createElement('button'); b.type = 'button'; b.className = clase; b.textContent = texto; b.addEventListener('click', hacer); $('lugar-acciones').appendChild(b); return b; };
  function cierraLugar(luego) {
    caja.hidden = true;
    if (medicion !== null) { navigator.geolocation.clearWatch(medicion); medicion = null; }
    if (luego) { try { sessionStorage.setItem('guardiana_orbita_lugar_luego', '1'); } catch (_) {} }
    if (antesDelLugar && antesDelLugar.focus) antesDelLugar.focus({ preventScroll: true });
  }
  function abreLugar(modo, extra) {
    if (caja.hidden) antesDelLugar = document.activeElement;
    caja.hidden = false;
    $('lugar-acciones').innerHTML = '';
    $('lugar-hallado').hidden = true;
    $('lugar-buscar').hidden = true;
    const t = $('lugar-t'), d = $('lugar-d');
    let primero = null;
    if (modo === 'inicio') {
      t.textContent = '¿En qué ciudad está este equipo?';
      d.textContent = 'Para dibujarlo en su sitio. Se mide con la ubicación de tu sistema o lo eliges tú, y siempre lo confirmas. Se guarda solo en este equipo; GUARDIANA no lo envía a nadie.';
      primero = accion('Detectar ubicación', 'boton principal', mideLugar);
      accion('Elegir ciudad', 'boton', () => abreLugar('buscar'));
      accion('Ahora no', 'boton texto', () => cierraLugar(true));
    } else if (modo === 'midiendo') {
      t.textContent = 'Midiendo…';
      d.textContent = 'Con la ubicación de tu sistema. Si te pregunta, permite el acceso: se toman varias medidas y se queda la mejor. Nada sale de este equipo hacia GUARDIANA.';
      primero = accion('Elegir ciudad', 'boton', () => abreLugar('buscar'));
      accion('Cancelar', 'boton texto', () => cierraLugar(false));
    } else if (modo === 'hallado') {
      candidato = extra.c;
      t.textContent = extra.titulo || '¿Es aquí?';
      d.textContent = extra.texto || 'Confírmalo y el planeta lo dibujará en su sitio.';
      $('lugar-hallado').hidden = false;
      $('lugar-ciudad').textContent = candidato.ciudad;
      $('lugar-dato').textContent = [candidato.region, nombrePais(candidato.pais)].filter(Boolean).join(' · ') + (candidato.precision != null ? ' · precisión ' + metros(candidato.precision) : ' · elegida por ti');
      primero = accion('Sí, es aquí', 'boton principal', () => confirma(candidato));
      accion('Elegir otra', 'boton', () => abreLugar('buscar'));
      accion('Ahora no', 'boton texto', () => cierraLugar(true));
    } else if (modo === 'buscar') {
      t.textContent = 'Elige la ciudad de este equipo';
      d.textContent = (extra && extra.motivo) || 'Escribe su nombre. La lista viaja con el programa: no se consulta nada en internet.';
      $('lugar-buscar').hidden = false;
      $('lugar-q').value = ''; $('lugar-res').innerHTML = '';
      primero = $('lugar-q');
      if (navigator.geolocation) accion('Detectar ubicación', 'boton', mideLugar);
      accion('Ahora no', 'boton texto', () => cierraLugar(true));
    } else if (modo === 'tengo') {
      t.textContent = 'Este equipo está en ' + LUGAR.ciudad;
      d.textContent = (LUGAR.fuente === 'sistema' ? 'Medido con la ubicación de tu sistema' + (LUGAR.precision != null ? ' (precisión ' + metros(LUGAR.precision) + ')' : '') : 'Elegido por ti') + ' y confirmado por ti. Se guarda solo en este equipo.';
      primero = accion('Cerrar', 'boton principal', () => cierraLugar(false));
      if (navigator.geolocation) accion('Medir de nuevo', 'boton', mideLugar);
      accion('Elegir otra', 'boton', () => abreLugar('buscar'));
      accion('Olvidar', 'boton texto', async () => { try { LUGAR = await apiLugar('DELETE'); } catch (_) {} LUGAR = null; pintaLugar(); cierraLugar(false); });
    }
    setTimeout(() => { if (!caja.hidden && primero) primero.focus(); }, 30);
  }
  async function confirma(c) {
    const cuerpo = { ciudad: c.ciudad, region: c.region || '', pais: c.pais, lat: c.lat, lon: c.lon, fuente: c.fuente, cuando: Date.now() };
    if (c.precision != null) cuerpo.precision = c.precision;
    try { LUGAR = (await apiLugar('PUT', cuerpo)) || cuerpo; }
    catch (_) { $('lugar-d').textContent = 'No se pudo guardar. Inténtalo de nuevo.'; return; }
    // «En vivo» reads the same place in this browser until it moves to the service too.
    try { localStorage.setItem('guardiana_pd_lugar', JSON.stringify({ lat: LUGAR.lat, lon: LUGAR.lon, ciudad: LUGAR.ciudad + (LUGAR.region ? ', ' + LUGAR.region : ''), cc: LUGAR.pais, fuente: LUGAR.fuente === 'manual' ? 'manual' : 'equipo', precision: LUGAR.precision != null ? LUGAR.precision : null, cuando: Date.now() })); } catch (_) {}
    pintaLugar();
    cierraLugar(false);
    mira(null, LUGAR);
  }
  // The system's location: several fresh samples for up to 15 s, the best one kept.
  function mideLugar(callado) {
    if (!navigator.geolocation) { if (callado !== true) abreLugar('buscar', { motivo: 'Este navegador no da la ubicación del sistema. Elige la ciudad.' }); return; }
    if (callado !== true) abreLugar('midiendo');
    if (medicion !== null) navigator.geolocation.clearWatch(medicion);
    let mejor = null, hecho = false, reloj = 0;
    const acaba = async (err) => {
      if (hecho) return; hecho = true; clearTimeout(reloj);
      if (medicion !== null) { navigator.geolocation.clearWatch(medicion); medicion = null; }
      if (!mejor) {
        if (callado === true) return;
        abreLugar('buscar', { motivo: err && err.code === 1 ? 'La ubicación está desactivada o no se ha permitido (en Windows: Configuración › Privacidad › Ubicación). Elige la ciudad, o actívala y vuelve a detectar.' : 'El sistema no ha dado una ubicación. Elige la ciudad.' });
        return;
      }
      const precision = Number.isFinite(mejor.coords.accuracy) ? mejor.coords.accuracy : Infinity;
      const lat = mejor.coords.latitude, lon = mejor.coords.longitude;
      if (precision > PRECISION_CIUDAD) {
        if (callado !== true) abreLugar('buscar', { motivo: 'La ubicación del sistema solo da una zona de ' + metros(precision) + ' (sin Wi‑Fi suele pasar): no basta para nombrar la ciudad. Elígela tú.' });
        return;
      }
      let m = null;
      try { m = cercana(await ciudades(), lat, lon); } catch (_) {}
      if (!m) { if (callado !== true) abreLugar('buscar'); return; }
      const c = { ciudad: m.c.n, region: m.c.adm, pais: m.c.cc, lat: Math.round(lat * 1000) / 1000, lon: Math.round(lon * 1000) / 1000, fuente: 'sistema', precision: Math.round(precision) };
      if (callado === true) {
        // a laptop that travelled: ask only when the new place is clearly another city
        if (LUGAR && distKm(LUGAR, c) > 30 && caja.hidden) abreLugar('hallado', { c, titulo: 'Parece que este equipo está ahora en ' + c.ciudad, texto: 'La ubicación de tu sistema lo sitúa allí. ¿Lo cambiamos?' });
        return;
      }
      const lejos = m.km > 12;
      abreLugar('hallado', { c, texto: lejos ? 'Tu punto está a ' + Math.round(m.km) + ' km de ' + m.c.n + ', la población más cercana que conoce el programa. Si tu municipio es otro, elígelo.' : 'Confírmalo y el planeta lo dibujará en su sitio.' });
    };
    medicion = navigator.geolocation.watchPosition((pos) => {
      if (!mejor || (Number.isFinite(pos.coords.accuracy) && pos.coords.accuracy < mejor.coords.accuracy)) mejor = pos;
      if (mejor && mejor.coords.accuracy <= 100) acaba();
    }, acaba, { enableHighAccuracy: true, timeout: 20000, maximumAge: 0 });
    reloj = setTimeout(() => acaba(), 15000);
  }
  // The search, over the list that travels with the program.
  $('lugar-q').addEventListener('input', async () => {
    const res = $('lugar-res');
    let lista;
    try { lista = await ciudades(); } catch (_) { res.innerHTML = '<p>No se pudo abrir la lista de ciudades.</p>'; return; }
    const q = lista.sinTildes($('lugar-q').value.trim());
    res.innerHTML = '';
    if (q.length < 2) return;
    const empieza = [], contiene = [];
    for (const c of lista) { if (c.k.startsWith(q)) empieza.push(c); else if (c.k.includes(q)) contiene.push(c); if (empieza.length >= 8) break; }
    const ver = empieza.concat(contiene).slice(0, 8);
    if (!ver.length) { res.innerHTML = '<p>No está en la lista. Prueba con la ciudad o el municipio más cercano.</p>'; return; }
    for (const c of ver) {
      const b = document.createElement('button'); b.type = 'button'; b.setAttribute('role', 'option');
      const n = document.createElement('span'); n.textContent = c.n + (c.adm ? ' · ' + c.adm : '');
      const s = document.createElement('small'); s.textContent = nombrePais(c.cc);
      b.append(n, s);
      b.addEventListener('click', () => abreLugar('hallado', { c: { ciudad: c.n, region: c.adm, pais: c.cc, lat: c.la, lon: c.lo, fuente: 'manual', precision: null } }));
      res.appendChild(b);
    }
  });
  $('lugar-q').addEventListener('keydown', (e) => { if (e.key === 'Enter') { const b = $('lugar-res').querySelector('button'); if (b) { e.preventDefault(); b.click(); } } });
  $('lugar-btn').addEventListener('click', () => (LUGAR ? abreLugar('tengo') : abreLugar('inicio')));
  caja.addEventListener('click', (e) => { if (e.target === caja) cierraLugar(false); });
  (async () => {
    try { LUGAR = await apiLugar('GET'); } catch (_) { LUGAR = null; }
    pintaLugar();
    if (LUGAR) {
      mira(null, LUGAR);
      if (LUGAR.fuente === 'sistema') { try { navigator.permissions.query({ name: 'geolocation' }).then((p) => { if (p.state === 'granted') mideLugar(true); }).catch(() => {}); } catch (_) {} }
      return;
    }
    let luego = false;
    try { luego = sessionStorage.getItem('guardiana_orbita_lugar_luego') === '1'; } catch (_) {}
    if (luego) return;
    // What «En vivo» measured or was told in this browser, offered for confirmation first.
    let previo = null;
    try { previo = JSON.parse(localStorage.getItem('guardiana_pd_lugar') || 'null'); } catch (_) {}
    setTimeout(() => {
      if (!caja.hidden || dentro) return;
      if (previo && previo.ciudad && previo.cc && Number.isFinite(previo.lat) && (previo.fuente === 'manual' || (Number.isFinite(previo.precision) && previo.precision <= PRECISION_CIUDAD))) {
        const [ciudad, region] = String(previo.ciudad).split(', ');
        abreLugar('hallado', { c: { ciudad, region: region || '', pais: previo.cc, lat: previo.lat, lon: previo.lon, fuente: previo.fuente === 'manual' ? 'manual' : 'sistema', precision: previo.fuente === 'manual' ? null : previo.precision } });
      } else abreLugar('inicio');
    }, 3600);
  })();

  // ---------- The loop ----------
  let t0 = performance.now(), ultimo = t0, impar = false, empezado = false;
  function listo() { if (empezado) return; empezado = true; requestAnimationFrame(() => document.body.classList.add('listo')); }
  setTimeout(listo, 2500);
  function cuadro(now) {
    requestAnimationFrame(cuadro);
    const dt = Math.min(0.05, (now - ultimo) / 1000); ultimo = now;
    const t = (now - t0) / 1000;
    impar = !impar;
    if (document.hidden) return;
    if (!limpio) limpioDesde = now;
    else if (now - limpioDesde > 1200) return; // faded out: nothing to draw
    if (dentro && impar) return; // behind glass, half the frames are enough
    while (cola.length && cola[0].at <= now / 1000) lanza(cola.shift(), t);
    while (ondas.length && t - ondas[0].t0 > 3) ondas.shift();
    avanzaVista(dentro ? dt * 2 : dt, now);
    pintaTierra(t, dentro ? 0.9 : 1);
    pintaRutas(t, dt);
  }
  mide();
  addEventListener('resize', mide);
  requestAnimationFrame(cuadro);
})();
