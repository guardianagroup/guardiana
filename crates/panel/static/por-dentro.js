// GUARDIANA por dentro (1.0.22). The owner asked for a view where you see «cómo trabaja GUARDIANA
// dentro del ordenador»: the antivirus, the browser and the programs that talk most, their data
// going out to countries and companies, what GUARDIANA lets pass and what it stops, live, «un
// espectáculo visual» someone can keep open for hours. Designed with Grok's help (12–13 Oct 2026).
//
// No third-party code (brief §8), and nothing from the internet: WebGL2 written here, the Earth's
// pictures (NASA, public domain) served by the panel itself, the world's countries from Natural
// Earth (public domain) in a 1.5° grid below. The light on the globe is the sun's of this very
// moment, computed here. Every figure is a measurement of this computer (`/api/por-dentro`) and
// nothing else is ever drawn: when there is no measurement, the page says why, in words.
(() => {
  'use strict';
  // ---------- The world: Natural Earth 110 m (public domain) in a 1.5° grid ----------
  const P = `FJ 11.6 -16.4 | TZ 34.8 -6.3 | EH -12.1 24.3 | CA -101.6 57.7 | US -99.1 39.5 | KZ 67.3 48.2 | UZ 63.2 41.7 | PG 144.3 -6.6 | ID 114 -0.3 | AR -65.1 -35.2 | CL -71.7 -37.3 | CD 23.6 -2.8 | SO 45.7 4.8 | KE 37.8 0.6 | SD 29.9 16 | TD 18.6 15.3 | HT -73.6 18.6 | DO -70.5 18.9 | RU 124.6 61.2 | BS -77.9 24.5 | FK -59.4 -51.7 | NO 13.4 62.9 | GL -41.5 74.8 | TF 69.5 -49.3 | TL 126 -8.8 | ZA 25.1 -29 | LS 28.2 -29.6 | MX -102.6 23.9 | UY -56 -32.8 | BR -53.1 -10.8 | BO -64.6 -16.7 | PE -74.4 -9.2 | CO -73.1 3.9 | PA -80.1 8.5 | CR -84.2 10 | NI -85 12.8 | HN -86.6 14.8 | SV -88.9 13.7 | GT -90.4 15.7 | BZ -88.7 17.2 | VE -66.2 7.2 | GY -59 4.8 | SR -55.9 4.1 | FR 2.3 46.6 | EC -78.4 -1.5 | PR -66.5 18.2 | JM -77.3 18.1 | CU -79 21.6 | ZW 29.8 -18.9 | BW 23.8 -22.1 | NA 17.2 -22.1 | SN -14.5 14.4 | ML -3.5 17.3 | MR -10.3 20.2 | BJ 2.3 9.6 | NE 9.3 17.3 | NG 8 9.5 | CM 12.6 5.7 | TG 1 8.4 | GH -1.2 7.9 | CI -5.6 7.6 | GN -11.1 10.4 | GW -15.1 12 | LR -9.4 6.4 | SL -11.8 8.5 | BF -1.8 12.3 | CF 20.4 6.5 | CG 15.1 -0.8 | GA 11.7 -0.6 | GQ 10.4 1.6 | ZM 27.7 -13.4 | MW 34.2 -13.2 | MZ 35.5 -17.2 | SZ 31.4 -26.5 | AO 17.5 -12.3 | BI 29.9 -3.4 | IL 34.8 31.5 | LB 35.9 33.9 | MG 46.7 -19.4 | PS 35.3 31.9 | GM -15.4 13.5 | TN 9.5 34.2 | DZ 2.6 28.2 | JO 36.8 31.2 | AE 54.2 23.9 | QA 51.2 25.3 | KW 47.6 29.3 | IQ 43.8 33 | OM 56.1 20.6 | VU 166.9 -15.2 | KH 104.9 12.7 | TH 101 15 | LA 103.7 18.4 | MM 96.5 21 | VN 105.8 20 | KP 127.2 40.1 | KR 127.8 36.4 | MN 102.9 46.8 | IN 79.6 22.9 | BD 90.3 23.8 | BT 90.5 27.4 | NP 84 28.2 | PK 69.4 30 | AF 66.1 33.9 | TJ 71 38.6 | KG 74.6 41.5 | TM 59.3 39.1 | IR 54.3 32.5 | SY 38.5 35 | AM 45 40.2 | SE 16.6 62.8 | BY 28 53.5 | UA 31.2 49.1 | PL 19.3 52.1 | AT 14.1 47.6 | HU 19.4 47.2 | MD 28.4 47.2 | RO 24.9 45.9 | LT 23.9 55.3 | LV 24.8 56.8 | EE 25.8 58.6 | DE 10.3 51.1 | BG 25.2 42.8 | GR 22.6 39.3 | TR 35.4 39 | AL 20 41.1 | HR 17.5 43.7 | CH 8.1 46.8 | LU 6 49.8 | BE 4.6 50.7 | NL 5.5 52.3 | PT -8.1 39.6 | ES -3.6 40.3 | IE -8 53.2 | NC 165.5 -21.3 | SB 159.1 -7.9 | NZ 170.5 -44 | AU 134.4 -25.6 | LK 80.7 7.7 | CN 103.9 36.6 | TW 121 23.7 | IT 12.2 43.5 | DK 9.3 56.2 | GB -2.7 53.9 | IS -18.8 65.1 | AZ 47.7 40.3 | GE 43.5 42.2 | PH 121.5 15.8 | MY 114.7 3.5 | BN 114.9 4.7 | SI 14.9 46.1 | FI 26.2 64.5 | SK 19.5 48.7 | CZ 15.3 49.8 | ER 38.7 15.4 | JP 136.9 36 | PY -58.4 -23.2 | YE 47.5 15.9 | SA 44.5 24.1 | AQ 31.2 -77.3 | CY 33.6 35.3 | CY 33 34.9 | MA -8.4 29.9 | EG 29.8 26.5 | LY 18 27 | ET 39.6 8.7 | DJ 42.5 11.8 | SO 46.2 9.8 | UG 32.4 1.3 | RW 29.9 -2 | BA 17.8 44.2 | MK 21.7 41.6 | RS 20.8 44.2 | ME 19.3 42.8 | XK 20.9 42.6 | TT -61.3 10.4 | SS 30.2 7.3`;
  const R = `0 94 23 9 0 196 4 17 0 2 23 27 0 3 23 3 0 185 4 6 0 3 4 8 0 2 23 33 0 24 22 6 0 13 19 2 0 28 19 5 0 99 4 1 0 4 4 3 0 1 4 2 0 1 4 4 0 1 4 7 0 4 23 34 0 21 22 5 0 54 19 3 0 90 4 2 0 19 4 6 0 7 23 33 0 22 22 3 0 56 19 2 0 92 4 4 0 1 4 2 0 3 4 3 0 3 4 2 0 4 4 1 0 15 23 27 0 52 19 4 0 19 19 14 0 16 19 4 0 61 4 3 0 42 23 24 0 51 19 2 0 19 19 15 0 84 4 4 0 1 4 3 0 1 4 1 0 1 4 2 0 2 4 3 0 1 4 2 0 2 4 6 0 17 23 22 0 50 19 2 0 9 19 2 0 6 19 32 0 7 19 2 0 27 19 6 0 7 5 1 0 22 4 1 0 3 4 8 0 6 4 2 0 2 4 12 0 61 19 19 0 3 19 3 0 7 19 3 0 1 19 37 0 2 19 13 0 30 5 15 0 6 4 4 0 2 4 1 0 3 4 11 0 1 4 1 0 1 4 2 0 5 4 1 0 4 4 7 0 9 23 20 0 29 22 4 152 2 22 2 19 1 0 18 19 1 0 4 19 3 0 1 19 57 0 7 19 5 0 4 19 6 0 1 5 17 4 18 0 4 4 2 0 4 4 2 0 1 4 4 0 2 4 2 0 6 4 4 0 58 19 6 0 7 19 3 0 2 19 3 0 2 19 1 0 8 19 1 0 3 19 1 0 73 19 1 0 2 19 2 0 4 5 14 4 39 0 6 4 8 0 54 19 7 0 7 19 3 0 18 19 1 0 71 19 3 0 2 19 3 0 1 5 17 4 36 0 1 4 1 0 4 4 7 0 2 4 1 0 55 19 7 0 3 19 4 0 106 5 13 4 34 0 2 4 2 0 1 4 2 0 5 4 5 0 9 23 7 0 13 145 2 0 18 22 2 111 5 0 2 152 5 19 99 0 11 5 16 4 32 0 6 4 1 0 3 4 3 0 3 4 2 0 11 23 5 0 32 22 4 111 4 0 2 152 7 19 88 0 1 19 8 0 12 5 11 0 1 5 4 4 31 0 11 4 6 0 15 23 2 0 32 22 5 111 4 0 2 152 5 19 85 0 5 19 5 0 18 5 5 0 10 5 1 4 1 5 1 4 27 0 11 4 6 0 2 4 2 0 46 22 4 111 4 0 4 121 3 19 77 0 2 19 1 0 2 19 2 0 4 19 2 0 26 5 3 0 13 4 27 0 11 4 10 0 37 144 1 0 11 111 3 0 5 120 1 121 1 19 75 0 12 19 3 0 25 5 1 0 18 5 1 4 29 0 7 4 10 0 37 144 2 0 7 143 2 0 1 111 3 0 3 120 1 119 2 120 2 19 73 0 12 19 5 0 21 5 1 0 22 4 32 0 2 4 15 0 33 144 1 0 1 144 2 0 7 122 1 0 4 114 1 0 1 19 2 119 2 112 4 19 23 6 3 19 43 0 14 19 4 0 43 4 1 0 2 4 31 0 2 4 16 0 31 134 2 0 1 144 3 0 3 131 2 122 5 114 6 112 6 19 19 6 11 19 29 140 2 19 12 0 9 19 3 0 48 4 31 0 1 4 15 0 35 144 4 0 2 131 1 122 6 114 6 113 2 112 2 113 3 19 17 6 13 19 27 140 4 19 11 0 9 19 1 0 50 4 41 0 2 4 2 0 36 144 2 0 3 44 2 130 1 122 4 154 4 114 4 113 9 19 7 6 24 19 4 98 3 19 2 98 4 19 1 98 1 19 9 140 5 19 9 0 1 19 1 0 60 4 2 5 19 4 17 0 1 4 2 0 4 4 3 0 33 44 1 0 1 44 6 122 4 154 1 115 1 153 4 113 11 19 5 6 27 140 1 98 18 140 10 19 6 0 64 5 26 4 11 5 1 4 2 0 7 4 1 0 33 44 6 128 2 115 5 116 4 118 3 117 2 113 5 19 7 6 23 140 1 6 1 140 3 98 20 140 9 19 4 0 64 5 27 4 9 5 2 4 5 0 39 44 6 142 4 151 1 127 3 173 1 118 5 113 1 0 2 19 1 0 2 19 7 0 3 6 20 140 5 98 17 140 12 19 3 0 65 5 28 4 4 5 5 0 2 4 1 0 42 44 6 142 3 0 2 127 1 171 2 173 2 118 4 0 7 19 5 0 3 6 3 7 4 6 13 140 10 98 10 140 13 19 4 0 3 156 2 0 61 5 28 4 2 5 6 0 41 133 7 44 1 0 5 142 2 0 3 174 1 175 1 173 1 123 4 0 9 147 1 19 3 0 3 6 2 7 7 6 5 106 4 140 11 98 9 140 14 19 1 0 5 156 3 0 61 5 35 0 42 132 2 133 5 0 8 142 2 0 2 126 1 172 1 124 3 125 1 0 2 125 4 0 2 125 2 147 2 146 2 0 3 107 4 7 4 6 2 7 1 106 5 140 32 96 2 0 71 5 34 0 43 132 1 133 5 0 10 142 1 0 2 126 1 124 1 0 3 125 12 110 1 146 2 0 3 107 6 7 4 106 3 140 31 0 1 140 1 0 1 96 2 0 8 156 2 0 63 5 32 0 44 132 1 133 5 0 10 142 1 0 3 124 2 0 2 125 12 108 3 0 3 107 8 7 2 105 1 104 1 105 2 140 29 0 4 96 2 97 1 0 7 156 1 0 65 5 30 0 47 133 3 0 3 83 1 0 1 83 2 82 1 0 11 125 6 109 4 88 2 108 10 107 3 104 6 103 2 140 30 0 3 97 2 0 5 156 3 0 65 5 30 0 47 163 2 0 1 83 7 82 1 0 9 124 2 0 4 161 1 0 1 109 4 88 3 108 10 104 7 103 4 140 28 0 4 97 2 0 2 156 6 0 67 5 27 0 47 163 4 83 6 82 2 0 17 109 2 88 4 108 10 104 7 103 2 99 4 140 27 0 8 156 3 0 71 28 2 5 22 0 48 163 5 83 7 82 1 165 3 0 3 165 2 0 8 80 1 84 2 88 6 108 9 104 5 103 4 99 2 140 29 0 6 156 1 0 75 28 1 0 1 28 5 5 16 0 48 163 3 83 9 82 1 165 5 0 1 165 4 164 6 77 1 84 1 159 3 88 4 108 9 104 3 103 5 99 4 140 28 0 82 28 1 0 1 28 8 5 4 0 2 5 1 0 5 5 1 0 47 163 2 83 12 165 10 164 6 159 7 88 1 87 1 0 2 108 7 103 8 99 5 102 2 140 25 0 82 28 2 0 1 28 8 5 1 0 10 5 1 0 45 163 3 83 12 165 11 164 5 0 2 159 9 0 2 108 7 103 5 99 8 102 4 101 2 99 4 140 16 0 84 28 1 0 1 28 7 5 1 0 10 5 2 0 1 20 1 0 41 163 2 3 2 54 1 83 11 165 11 164 6 0 1 159 9 0 4 89 1 108 3 103 6 99 16 94 3 140 14 0 85 28 1 0 2 28 7 0 55 163 2 54 4 53 1 83 10 165 10 164 6 0 2 159 9 0 2 85 1 89 1 0 7 103 2 99 12 100 3 99 1 94 2 140 14 0 90 28 6 0 54 163 2 54 5 53 2 83 10 56 1 165 1 16 1 165 6 164 7 0 2 159 9 85 2 89 2 0 7 99 13 100 3 94 4 140 12 0 2 141 1 0 89 28 5 0 12 48 1 0 41 163 1 3 1 54 5 53 4 83 6 56 4 16 3 165 4 15 8 0 1 159 11 89 3 0 6 99 13 0 2 100 1 94 5 140 1 95 4 140 3 0 95 28 5 0 5 28 2 0 6 48 3 0 38 54 7 53 5 83 3 56 7 16 4 165 2 15 8 0 2 159 10 89 2 0 10 99 9 0 4 94 4 92 1 93 3 95 1 0 100 28 5 0 3 28 3 0 10 18 2 0 35 54 7 53 7 56 7 16 6 15 9 0 2 159 6 158 2 89 3 0 11 99 7 0 7 94 2 92 2 93 3 95 1 0 1 140 2 0 99 28 6 39 2 0 48 54 7 53 7 56 7 16 6 15 9 155 1 0 2 159 1 158 2 159 1 158 3 89 2 0 12 99 6 0 8 94 2 92 5 93 1 0 9 148 2 0 93 28 1 0 2 28 1 39 2 37 3 0 45 52 2 54 5 53 6 56 7 16 7 15 8 155 2 0 2 158 7 0 14 99 5 0 9 94 1 0 1 94 1 92 4 93 2 0 8 148 1 0 98 39 1 38 1 37 2 36 2 0 44 52 3 53 7 66 1 56 9 16 6 15 9 166 3 155 1 0 1 158 4 0 17 99 3 0 12 94 1 92 3 91 1 93 1 91 1 95 1 0 7 148 1 0 101 36 2 0 45 52 3 53 5 66 4 56 2 57 6 58 1 16 5 15 9 166 4 155 1 158 1 0 20 99 3 0 13 92 2 91 4 95 1 0 9 148 1 0 100 36 1 0 7 33 1 0 1 41 1 0 36 62 4 53 2 66 5 55 1 57 7 58 1 16 5 15 8 166 5 167 1 0 3 168 1 13 1 0 17 99 2 0 16 91 2 95 1 0 9 148 1 0 1 148 1 0 100 35 1 0 5 33 1 41 1 0 1 41 6 0 32 62 1 65 1 62 1 61 4 60 2 59 1 55 1 57 7 16 5 67 2 15 1 177 1 15 2 177 3 166 6 168 4 13 1 0 17 99 2 0 17 95 1 0 114 34 1 0 2 33 4 41 8 0 31 65 2 64 1 61 4 60 2 59 1 55 1 57 6 58 2 16 2 67 4 177 6 166 9 168 1 13 1 0 20 139 1 0 11 92 2 0 15 148 2 0 104 33 5 41 6 42 2 0 31 64 2 61 4 60 2 59 1 55 1 57 6 58 2 67 7 177 6 166 8 13 2 0 20 139 1 0 13 92 1 0 15 148 1 0 104 33 7 41 4 42 3 43 2 44 1 0 29 64 1 61 3 60 2 0 3 57 3 58 4 67 8 177 5 14 1 166 6 13 2 0 32 9 1 0 2 149 2 0 8 149 2 0 110 33 6 41 2 30 3 42 1 43 3 44 1 30 1 0 40 58 4 67 2 12 8 177 2 169 1 14 2 166 2 14 1 13 4 0 32 9 2 0 1 149 2 0 7 149 1 9 1 0 110 33 7 41 3 30 2 42 2 43 1 30 1 44 1 30 1 0 40 70 2 69 1 58 2 68 1 12 9 169 2 14 4 13 4 0 34 9 2 0 1 149 1 0 5 149 3 9 2 0 108 45 1 33 6 30 13 0 39 69 3 68 3 12 8 169 3 14 4 13 2 0 37 9 3 0 4 9 6 0 1 9 3 0 2 9 1 0 100 45 4 33 4 30 15 0 37 69 4 68 2 12 8 169 3 14 4 13 1 0 39 9 2 0 4 9 5 0 7 9 1 0 2 9 1 0 97 45 3 32 2 33 3 30 16 0 36 69 2 68 3 12 8 170 2 2 3 14 3 0 40 9 3 0 3 9 5 0 1 9 2 0 8 9 1 0 1 9 2 0 93 32 1 45 1 32 5 30 21 0 33 69 1 68 3 12 9 2 5 14 2 0 41 9 3 0 5 9 1 0 4 9 1 0 2 9 1 0 4 9 5 8 2 0 90 32 5 30 25 0 32 12 12 2 6 0 43 9 2 0 21 9 2 8 3 0 89 32 5 30 26 0 31 75 3 12 9 2 6 0 44 9 2 0 2 9 1 0 14 9 1 0 2 9 2 8 4 0 6 136 1 0 82 32 4 30 26 0 32 75 6 12 5 2 6 0 48 9 3 0 15 9 2 8 2 0 1 8 2 0 7 136 1 0 81 32 4 30 24 0 33 75 6 12 4 71 3 72 1 2 4 0 55 9 1 0 15 8 2 0 88 32 6 31 2 30 19 0 34 75 7 71 1 12 2 71 3 72 1 2 3 73 1 0 162 32 5 31 4 30 17 0 34 75 7 71 3 12 1 71 2 72 1 73 4 0 60 138 4 0 4 138 1 0 93 32 5 31 6 30 14 0 34 75 7 71 6 73 1 72 2 73 3 0 5 79 1 0 51 138 1 0 1 138 5 0 3 138 2 0 94 32 4 31 6 30 14 0 34 75 7 71 5 73 3 72 1 73 3 0 4 79 3 0 49 138 8 0 3 138 3 0 95 32 2 31 7 30 13 0 34 51 1 75 6 71 3 49 4 73 3 0 4 79 4 0 49 138 11 0 1 138 3 0 96 11 1 31 8 30 12 0 34 51 6 50 3 49 5 73 2 0 5 79 4 0 48 138 17 0 95 11 1 31 5 157 2 30 12 0 36 51 5 50 4 49 4 73 1 0 7 79 2 0 47 138 20 0 10 135 1 0 83 11 2 31 3 157 3 30 12 0 36 51 5 50 5 49 2 73 3 0 5 79 3 0 45 138 23 0 10 135 1 0 82 11 2 10 4 157 4 30 8 0 39 51 3 50 5 26 3 73 3 0 5 79 3 0 44 138 25 0 92 11 1 10 7 157 3 30 4 0 42 51 3 50 4 26 4 73 2 0 6 79 2 0 45 138 26 0 91 11 1 10 7 157 3 30 4 0 42 51 3 26 1 50 1 26 6 73 1 0 54 138 26 0 91 11 1 10 9 30 5 0 42 51 3 26 9 0 54 138 26 0 90 11 1 10 9 30 5 0 44 26 7 27 2 26 1 0 56 138 25 0 90 11 1 10 8 29 2 30 3 0 46 26 8 0 57 138 25 0 90 11 1 10 8 29 3 30 1 0 47 26 7 0 58 138 8 0 4 138 13 0 90 11 1 10 8 29 3 0 48 26 5 0 60 138 6 0 7 138 1 0 1 138 9 0 91 11 1 10 9 0 130 138 8 0 15 137 1 0 75 11 2 10 9 0 131 138 7 0 16 137 1 0 74 11 2 10 8 0 133 138 2 0 1 138 1 0 18 137 3 0 72 11 1 10 7 0 157 137 2 0 73 11 1 10 5 0 140 138 2 0 17 137 1 0 74 11 1 10 5 0 140 138 2 0 15 137 2 0 74 11 2 10 4 0 156 137 2 0 76 11 2 10 3 0 156 137 3 0 76 11 2 10 4 0 234 11 2 10 3 0 235 11 1 10 3 0 236 11 2 10 2 0 5 21 2 0 230 11 3 10 1 0 237 11 2 10 3 0 255 0 148`;
  const G = { nlat: 94, nlon: 240, paso: 1.5, lat0: 83.25, lon0: -179.25 };
  const RAD = Math.PI / 180;
  const listaPaises = P.trim().split(' | ').map((s) => { const [c, lon, lat] = s.trim().split(' '); return { c, lon: +lon, lat: +lat }; });
  const centros = {};
  listaPaises.forEach((p) => { if (!centros[p.c]) centros[p.c] = p; });
  // Countries too small for the 110 m map (no land cell): their centre only.
  [['SG', 103.8, 1.35], ['HK', 114.2, 22.3], ['BH', 50.55, 26.07], ['MT', 14.4, 35.9], ['MU', 57.55, -20.25], ['AD', 1.6, 42.5], ['MC', 7.42, 43.74], ['LI', 9.55, 47.16], ['SM', 12.46, 43.94], ['BB', -59.55, 13.17], ['MV', 73.5, 4.2], ['GI', -5.35, 36.14], ['MO', 113.55, 22.17], ['BM', -64.75, 32.3], ['KY', -81.25, 19.3], ['IM', -4.5, 54.2], ['JE', -2.13, 49.21], ['GG', -2.58, 49.45], ['VG', -64.6, 18.42], ['SC', 55.45, -4.6], ['AG', -61.8, 17.07], ['LC', -60.98, 13.9], ['GD', -61.68, 12.1], ['KN', -62.75, 17.33], ['VC', -61.2, 13.25], ['DM', -61.35, 15.42], ['CW', -68.95, 12.17], ['AW', -69.97, 12.52], ['ST', 6.6, 0.3], ['CV', -23.6, 15.1], ['KM', 43.3, -11.7], ['TO', -175.2, -21.2], ['WS', -172.1, -13.75], ['KI', 173.0, 1.4], ['MH', 171.2, 7.1], ['FM', 158.2, 6.9], ['PW', 134.5, 7.5], ['TV', 179.2, -8.5], ['NR', 166.93, -0.53]]
    .forEach(([c, lon, lat]) => { if (!centros[c]) { centros[c] = { c, lon, lat }; listaPaises.push({ c, lon, lat }); } });
  const celdas = new Uint8Array(G.nlat * G.nlon);
  { const n = R.trim().split(/\s+/).map(Number); let k = 0; for (let i = 0; i < n.length; i += 2) { celdas.fill(n[i], k, k + n[i + 1]); k += n[i + 1]; } }
  const paisEn = (lon, lat) => {
    const i = Math.round((G.lat0 - lat) / G.paso), j = Math.round((lon - G.lon0) / G.paso);
    if (i < 0 || i >= G.nlat) return null;
    const v = celdas[i * G.nlon + (((j % G.nlon) + G.nlon) % G.nlon)];
    return v ? listaPaises[v - 1].c : null;
  };
  // Where the computer is, without asking anyone and without letting anyone pretend otherwise
  // (the owner, 13 Oct 2026: «que nadie pueda hacer el engaño»): the time zone the system says,
  // then the region of its languages. Nothing on the page changes it, and nothing is stored.
  // Every time zone of the IANA database (zone.tab): its country and the coordinates of its city.
  const ZONAS = {'Africa/Abidjan':['CI',5.32,-4.03],'Africa/Accra':['GH',5.55,-0.22],'Africa/Addis_Ababa':['ET',9.03,38.7],'Africa/Algiers':['DZ',36.78,3.05],'Africa/Asmara':['ER',15.33,38.88],'Africa/Bamako':['ML',12.65,-8.0],'Africa/Bangui':['CF',4.37,18.58],'Africa/Banjul':['GM',13.47,-16.65],'Africa/Bissau':['GW',11.85,-15.58],'Africa/Blantyre':['MW',-15.78,35.0],'Africa/Brazzaville':['CG',-4.27,15.28],'Africa/Bujumbura':['BI',-3.38,29.37],'Africa/Cairo':['EG',30.05,31.25],'Africa/Casablanca':['MA',33.65,-7.58],'Africa/Ceuta':['ES',35.88,-5.32],'Africa/Conakry':['GN',9.52,-13.72],'Africa/Dakar':['SN',14.67,-17.43],'Africa/Dar_es_Salaam':['TZ',-6.8,39.28],'Africa/Djibouti':['DJ',11.6,43.15],'Africa/Douala':['CM',4.05,9.7],'Africa/El_Aaiun':['EH',27.15,-13.2],'Africa/Freetown':['SL',8.5,-13.25],'Africa/Gaborone':['BW',-24.65,25.92],'Africa/Harare':['ZW',-17.83,31.05],'Africa/Johannesburg':['ZA',-26.25,28.0],'Africa/Juba':['SS',4.85,31.62],'Africa/Kampala':['UG',0.32,32.42],'Africa/Khartoum':['SD',15.6,32.53],'Africa/Kigali':['RW',-1.95,30.07],'Africa/Kinshasa':['CD',-4.3,15.3],'Africa/Lagos':['NG',6.45,3.4],'Africa/Libreville':['GA',0.38,9.45],'Africa/Lome':['TG',6.13,1.22],'Africa/Luanda':['AO',-8.8,13.23],'Africa/Lubumbashi':['CD',-11.67,27.47],'Africa/Lusaka':['ZM',-15.42,28.28],'Africa/Malabo':['GQ',3.75,8.78],'Africa/Maputo':['MZ',-25.97,32.58],'Africa/Maseru':['LS',-29.47,27.5],'Africa/Mbabane':['SZ',-26.3,31.1],'Africa/Mogadishu':['SO',2.07,45.37],'Africa/Monrovia':['LR',6.3,-10.78],'Africa/Nairobi':['KE',-1.28,36.82],'Africa/Ndjamena':['TD',12.12,15.05],'Africa/Niamey':['NE',13.52,2.12],'Africa/Nouakchott':['MR',18.1,-15.95],'Africa/Ouagadougou':['BF',12.37,-1.52],'Africa/Porto-Novo':['BJ',6.48,2.62],'Africa/Sao_Tome':['ST',0.33,6.73],'Africa/Tripoli':['LY',32.9,13.18],'Africa/Tunis':['TN',36.8,10.18],'Africa/Windhoek':['NA',-22.57,17.1],'America/Adak':['US',51.88,-176.66],'America/Anchorage':['US',61.22,-149.9],'America/Anguilla':['AI',18.2,-63.07],'America/Antigua':['AG',17.05,-61.8],'America/Araguaina':['BR',-7.2,-48.2],'America/Argentina/Buenos_Aires':['AR',-34.6,-58.45],'America/Argentina/Catamarca':['AR',-28.47,-65.78],'America/Argentina/Cordoba':['AR',-31.4,-64.18],'America/Argentina/Jujuy':['AR',-24.18,-65.3],'America/Argentina/La_Rioja':['AR',-29.43,-66.85],'America/Argentina/Mendoza':['AR',-32.88,-68.82],'America/Argentina/Rio_Gallegos':['AR',-51.63,-69.22],'America/Argentina/Salta':['AR',-24.78,-65.42],'America/Argentina/San_Juan':['AR',-31.53,-68.52],'America/Argentina/San_Luis':['AR',-33.32,-66.35],'America/Argentina/Tucuman':['AR',-26.82,-65.22],'America/Argentina/Ushuaia':['AR',-54.8,-68.3],'America/Aruba':['AW',12.5,-69.97],'America/Asuncion':['PY',-25.27,-57.67],'America/Atikokan':['CA',48.76,-91.62],'America/Bahia':['BR',-12.98,-38.52],'America/Bahia_Banderas':['MX',20.8,-105.25],'America/Barbados':['BB',13.1,-59.62],'America/Belem':['BR',-1.45,-48.48],'America/Belize':['BZ',17.5,-88.2],'America/Blanc-Sablon':['CA',51.42,-57.12],'America/Boa_Vista':['BR',2.82,-60.67],'America/Bogota':['CO',4.6,-74.08],'America/Boise':['US',43.61,-116.2],'America/Buenos_Aires':['AR',-34.6,-58.45],'America/Cambridge_Bay':['CA',69.11,-105.05],'America/Campo_Grande':['BR',-20.45,-54.62],'America/Cancun':['MX',21.08,-86.77],'America/Caracas':['VE',10.5,-66.93],'America/Cayenne':['GF',4.93,-52.33],'America/Cayman':['KY',19.3,-81.38],'America/Chicago':['US',41.85,-87.65],'America/Chihuahua':['MX',28.63,-106.08],'America/Ciudad_Juarez':['MX',31.73,-106.48],'America/Costa_Rica':['CR',9.93,-84.08],'America/Coyhaique':['CL',-45.57,-72.07],'America/Creston':['CA',49.1,-116.52],'America/Cuiaba':['BR',-15.58,-56.08],'America/Curacao':['CW',12.18,-69.0],'America/Danmarkshavn':['GL',76.77,-18.67],'America/Dawson':['CA',64.07,-139.42],'America/Dawson_Creek':['CA',55.77,-120.23],'America/Denver':['US',39.74,-104.98],'America/Detroit':['US',42.33,-83.05],'America/Dominica':['DM',15.3,-61.4],'America/Edmonton':['CA',53.55,-113.47],'America/Eirunepe':['BR',-6.67,-69.87],'America/El_Salvador':['SV',13.7,-89.2],'America/Fort_Nelson':['CA',58.8,-122.7],'America/Fortaleza':['BR',-3.72,-38.5],'America/Glace_Bay':['CA',46.2,-59.95],'America/Godthab':['GL',64.18,-51.73],'America/Goose_Bay':['CA',53.33,-60.42],'America/Grand_Turk':['TC',21.47,-71.13],'America/Grenada':['GD',12.05,-61.75],'America/Guadeloupe':['GP',16.23,-61.53],'America/Guatemala':['GT',14.63,-90.52],'America/Guayaquil':['EC',-2.17,-79.83],'America/Guyana':['GY',6.8,-58.17],'America/Halifax':['CA',44.65,-63.6],'America/Havana':['CU',23.13,-82.37],'America/Hermosillo':['MX',29.07,-110.97],'America/Indiana/Indianapolis':['US',39.77,-86.16],'America/Indiana/Knox':['US',41.3,-86.62],'America/Indiana/Marengo':['US',38.38,-86.34],'America/Indiana/Petersburg':['US',38.49,-87.28],'America/Indiana/Tell_City':['US',37.95,-86.76],'America/Indiana/Vevay':['US',38.75,-85.07],'America/Indiana/Vincennes':['US',38.68,-87.53],'America/Indiana/Winamac':['US',41.05,-86.6],'America/Inuvik':['CA',68.35,-133.72],'America/Iqaluit':['CA',63.73,-68.47],'America/Jamaica':['JM',17.97,-76.79],'America/Juneau':['US',58.3,-134.42],'America/Kentucky/Louisville':['US',38.25,-85.76],'America/Kentucky/Monticello':['US',36.83,-84.85],'America/Kralendijk':['BQ',12.15,-68.28],'America/La_Paz':['BO',-16.5,-68.15],'America/Lima':['PE',-12.05,-77.05],'America/Los_Angeles':['US',34.05,-118.24],'America/Lower_Princes':['SX',18.05,-63.05],'America/Maceio':['BR',-9.67,-35.72],'America/Managua':['NI',12.15,-86.28],'America/Manaus':['BR',-3.13,-60.02],'America/Marigot':['MF',18.07,-63.08],'America/Martinique':['MQ',14.6,-61.08],'America/Matamoros':['MX',25.83,-97.5],'America/Mazatlan':['MX',23.22,-106.42],'America/Menominee':['US',45.11,-87.61],'America/Merida':['MX',20.97,-89.62],'America/Metlakatla':['US',55.13,-131.58],'America/Mexico_City':['MX',19.4,-99.15],'America/Miquelon':['PM',47.05,-56.33],'America/Moncton':['CA',46.1,-64.78],'America/Monterrey':['MX',25.67,-100.32],'America/Montevideo':['UY',-34.91,-56.21],'America/Montserrat':['MS',16.72,-62.22],'America/Nassau':['BS',25.08,-77.35],'America/New_York':['US',40.71,-74.01],'America/Nome':['US',64.5,-165.41],'America/Noronha':['BR',-3.85,-32.42],'America/North_Dakota/Beulah':['US',47.26,-101.78],'America/North_Dakota/Center':['US',47.12,-101.3],'America/North_Dakota/New_Salem':['US',46.84,-101.41],'America/Nuuk':['GL',64.18,-51.73],'America/Ojinaga':['MX',29.57,-104.42],'America/Panama':['PA',8.97,-79.53],'America/Paramaribo':['SR',5.83,-55.17],'America/Phoenix':['US',33.45,-112.07],'America/Port-au-Prince':['HT',18.53,-72.33],'America/Port_of_Spain':['TT',10.65,-61.52],'America/Porto_Velho':['BR',-8.77,-63.9],'America/Puerto_Rico':['PR',18.47,-66.11],'America/Punta_Arenas':['CL',-53.15,-70.92],'America/Rankin_Inlet':['CA',62.82,-92.08],'America/Recife':['BR',-8.05,-34.9],'America/Regina':['CA',50.4,-104.65],'America/Resolute':['CA',74.7,-94.83],'America/Rio_Branco':['BR',-9.97,-67.8],'America/Santarem':['BR',-2.43,-54.87],'America/Santiago':['CL',-33.45,-70.67],'America/Santo_Domingo':['DO',18.47,-69.9],'America/Sao_Paulo':['BR',-23.53,-46.62],'America/Scoresbysund':['GL',70.48,-21.97],'America/Sitka':['US',57.18,-135.3],'America/St_Barthelemy':['BL',17.88,-62.85],'America/St_Johns':['CA',47.57,-52.72],'America/St_Kitts':['KN',17.3,-62.72],'America/St_Lucia':['LC',14.02,-61.0],'America/St_Thomas':['VI',18.35,-64.93],'America/St_Vincent':['VC',13.15,-61.23],'America/Swift_Current':['CA',50.28,-107.83],'America/Tegucigalpa':['HN',14.1,-87.22],'America/Thule':['GL',76.57,-68.78],'America/Tijuana':['MX',32.53,-117.02],'America/Toronto':['CA',43.65,-79.38],'America/Tortola':['VG',18.45,-64.62],'America/Vancouver':['CA',49.27,-123.12],'America/Whitehorse':['CA',60.72,-135.05],'America/Winnipeg':['CA',49.88,-97.15],'America/Yakutat':['US',59.55,-139.73],'Antarctica/Casey':['AQ',-66.28,110.52],'Antarctica/Davis':['AQ',-68.58,77.97],'Antarctica/DumontDUrville':['AQ',-66.67,140.02],'Antarctica/Macquarie':['AU',-54.5,158.95],'Antarctica/Mawson':['AQ',-67.6,62.88],'Antarctica/McMurdo':['AQ',-77.83,166.6],'Antarctica/Palmer':['AQ',-64.8,-64.1],'Antarctica/Rothera':['AQ',-67.57,-68.13],'Antarctica/Syowa':['AQ',-69.01,39.59],'Antarctica/Troll':['AQ',-72.01,2.53],'Antarctica/Vostok':['AQ',-78.4,106.9],'Arctic/Longyearbyen':['SJ',78.0,16.0],'Asia/Aden':['YE',12.75,45.2],'Asia/Almaty':['KZ',43.25,76.95],'Asia/Amman':['JO',31.95,35.93],'Asia/Anadyr':['RU',64.75,177.48],'Asia/Aqtau':['KZ',44.52,50.27],'Asia/Aqtobe':['KZ',50.28,57.17],'Asia/Ashgabat':['TM',37.95,58.38],'Asia/Atyrau':['KZ',47.12,51.93],'Asia/Baghdad':['IQ',33.35,44.42],'Asia/Bahrain':['BH',26.38,50.58],'Asia/Baku':['AZ',40.38,49.85],'Asia/Bangkok':['TH',13.75,100.52],'Asia/Barnaul':['RU',53.37,83.75],'Asia/Beirut':['LB',33.88,35.5],'Asia/Bishkek':['KG',42.9,74.6],'Asia/Brunei':['BN',4.93,114.92],'Asia/Calcutta':['IN',22.53,88.37],'Asia/Chita':['RU',52.05,113.47],'Asia/Colombo':['LK',6.93,79.85],'Asia/Damascus':['SY',33.5,36.3],'Asia/Dhaka':['BD',23.72,90.42],'Asia/Dili':['TL',-8.55,125.58],'Asia/Dubai':['AE',25.3,55.3],'Asia/Dushanbe':['TJ',38.58,68.8],'Asia/Famagusta':['CY',35.12,33.95],'Asia/Gaza':['PS',31.5,34.47],'Asia/Hebron':['PS',31.53,35.09],'Asia/Ho_Chi_Minh':['VN',10.75,106.67],'Asia/Hong_Kong':['HK',22.28,114.15],'Asia/Hovd':['MN',48.02,91.65],'Asia/Irkutsk':['RU',52.27,104.33],'Asia/Jakarta':['ID',-6.17,106.8],'Asia/Jayapura':['ID',-2.53,140.7],'Asia/Jerusalem':['IL',31.78,35.22],'Asia/Kabul':['AF',34.52,69.2],'Asia/Kamchatka':['RU',53.02,158.65],'Asia/Karachi':['PK',24.87,67.05],'Asia/Kathmandu':['NP',27.72,85.32],'Asia/Katmandu':['NP',27.72,85.32],'Asia/Khandyga':['RU',62.66,135.55],'Asia/Kolkata':['IN',22.53,88.37],'Asia/Krasnoyarsk':['RU',56.02,92.83],'Asia/Kuala_Lumpur':['MY',3.17,101.7],'Asia/Kuching':['MY',1.55,110.33],'Asia/Kuwait':['KW',29.33,47.98],'Asia/Macau':['MO',22.2,113.54],'Asia/Magadan':['RU',59.57,150.8],'Asia/Makassar':['ID',-5.12,119.4],'Asia/Manila':['PH',14.59,120.97],'Asia/Muscat':['OM',23.6,58.58],'Asia/Nicosia':['CY',35.17,33.37],'Asia/Novokuznetsk':['RU',53.75,87.12],'Asia/Novosibirsk':['RU',55.03,82.92],'Asia/Omsk':['RU',55.0,73.4],'Asia/Oral':['KZ',51.22,51.35],'Asia/Phnom_Penh':['KH',11.55,104.92],'Asia/Pontianak':['ID',-0.03,109.33],'Asia/Pyongyang':['KP',39.02,125.75],'Asia/Qatar':['QA',25.28,51.53],'Asia/Qostanay':['KZ',53.2,63.62],'Asia/Qyzylorda':['KZ',44.8,65.47],'Asia/Rangoon':['MM',16.78,96.17],'Asia/Riyadh':['SA',24.63,46.72],'Asia/Saigon':['VN',10.75,106.67],'Asia/Sakhalin':['RU',46.97,142.7],'Asia/Samarkand':['UZ',39.67,66.8],'Asia/Seoul':['KR',37.55,126.97],'Asia/Shanghai':['CN',31.23,121.47],'Asia/Singapore':['SG',1.28,103.85],'Asia/Srednekolymsk':['RU',67.47,153.72],'Asia/Taipei':['TW',25.05,121.5],'Asia/Tashkent':['UZ',41.33,69.3],'Asia/Tbilisi':['GE',41.72,44.82],'Asia/Tehran':['IR',35.67,51.43],'Asia/Thimphu':['BT',27.47,89.65],'Asia/Tokyo':['JP',35.65,139.74],'Asia/Tomsk':['RU',56.5,84.97],'Asia/Ulaanbaatar':['MN',47.92,106.88],'Asia/Urumqi':['CN',43.8,87.58],'Asia/Ust-Nera':['RU',64.56,143.23],'Asia/Vientiane':['LA',17.97,102.6],'Asia/Vladivostok':['RU',43.17,131.93],'Asia/Yakutsk':['RU',62.0,129.67],'Asia/Yangon':['MM',16.78,96.17],'Asia/Yekaterinburg':['RU',56.85,60.6],'Asia/Yerevan':['AM',40.18,44.5],'Atlantic/Azores':['PT',37.73,-25.67],'Atlantic/Bermuda':['BM',32.28,-64.77],'Atlantic/Canary':['ES',28.1,-15.4],'Atlantic/Cape_Verde':['CV',14.92,-23.52],'Atlantic/Faroe':['FO',62.02,-6.77],'Atlantic/Madeira':['PT',32.63,-16.9],'Atlantic/Reykjavik':['IS',64.15,-21.85],'Atlantic/South_Georgia':['GS',-54.27,-36.53],'Atlantic/St_Helena':['SH',-15.92,-5.7],'Atlantic/Stanley':['FK',-51.7,-57.85],'Australia/Adelaide':['AU',-34.92,138.58],'Australia/Brisbane':['AU',-27.47,153.03],'Australia/Broken_Hill':['AU',-31.95,141.45],'Australia/Darwin':['AU',-12.47,130.83],'Australia/Eucla':['AU',-31.72,128.87],'Australia/Hobart':['AU',-42.88,147.32],'Australia/Lindeman':['AU',-20.27,149.0],'Australia/Lord_Howe':['AU',-31.55,159.08],'Australia/Melbourne':['AU',-37.82,144.97],'Australia/Perth':['AU',-31.95,115.85],'Australia/Sydney':['AU',-33.87,151.22],'Europe/Amsterdam':['NL',52.37,4.9],'Europe/Andorra':['AD',42.5,1.52],'Europe/Astrakhan':['RU',46.35,48.05],'Europe/Athens':['GR',37.97,23.72],'Europe/Belgrade':['RS',44.83,20.5],'Europe/Berlin':['DE',52.5,13.37],'Europe/Bratislava':['SK',48.15,17.12],'Europe/Brussels':['BE',50.83,4.33],'Europe/Bucharest':['RO',44.43,26.1],'Europe/Budapest':['HU',47.5,19.08],'Europe/Busingen':['DE',47.7,8.68],'Europe/Chisinau':['MD',47.0,28.83],'Europe/Copenhagen':['DK',55.67,12.58],'Europe/Dublin':['IE',53.33,-6.25],'Europe/Gibraltar':['GI',36.13,-5.35],'Europe/Guernsey':['GG',49.45,-2.54],'Europe/Helsinki':['FI',60.17,24.97],'Europe/Isle_of_Man':['IM',54.15,-4.47],'Europe/Istanbul':['TR',41.02,28.97],'Europe/Jersey':['JE',49.18,-2.11],'Europe/Kaliningrad':['RU',54.72,20.5],'Europe/Kiev':['UA',50.43,30.52],'Europe/Kirov':['RU',58.6,49.65],'Europe/Kyiv':['UA',50.43,30.52],'Europe/Lisbon':['PT',38.72,-9.13],'Europe/Ljubljana':['SI',46.05,14.52],'Europe/London':['GB',51.51,-0.13],'Europe/Luxembourg':['LU',49.6,6.15],'Europe/Madrid':['ES',40.4,-3.68],'Europe/Malta':['MT',35.9,14.52],'Europe/Mariehamn':['AX',60.1,19.95],'Europe/Minsk':['BY',53.9,27.57],'Europe/Monaco':['MC',43.7,7.38],'Europe/Moscow':['RU',55.76,37.62],'Europe/Oslo':['NO',59.92,10.75],'Europe/Paris':['FR',48.87,2.33],'Europe/Podgorica':['ME',42.43,19.27],'Europe/Prague':['CZ',50.08,14.43],'Europe/Riga':['LV',56.95,24.1],'Europe/Rome':['IT',41.9,12.48],'Europe/Samara':['RU',53.2,50.15],'Europe/San_Marino':['SM',43.92,12.47],'Europe/Sarajevo':['BA',43.87,18.42],'Europe/Saratov':['RU',51.57,46.03],'Europe/Simferopol':['UA',44.95,34.1],'Europe/Skopje':['MK',41.98,21.43],'Europe/Sofia':['BG',42.68,23.32],'Europe/Stockholm':['SE',59.33,18.05],'Europe/Tallinn':['EE',59.42,24.75],'Europe/Tirane':['AL',41.33,19.83],'Europe/Ulyanovsk':['RU',54.33,48.4],'Europe/Vaduz':['LI',47.15,9.52],'Europe/Vatican':['VA',41.9,12.45],'Europe/Vienna':['AT',48.22,16.33],'Europe/Vilnius':['LT',54.68,25.32],'Europe/Volgograd':['RU',48.73,44.42],'Europe/Warsaw':['PL',52.25,21.0],'Europe/Zagreb':['HR',45.8,15.97],'Europe/Zurich':['CH',47.38,8.53],'Indian/Antananarivo':['MG',-18.92,47.52],'Indian/Chagos':['IO',-7.33,72.42],'Indian/Christmas':['CX',-10.42,105.72],'Indian/Cocos':['CC',-12.17,96.92],'Indian/Comoro':['KM',-11.68,43.27],'Indian/Kerguelen':['TF',-49.35,70.22],'Indian/Mahe':['SC',-4.67,55.47],'Indian/Maldives':['MV',4.17,73.5],'Indian/Mauritius':['MU',-20.17,57.5],'Indian/Mayotte':['YT',-12.78,45.23],'Indian/Reunion':['RE',-20.87,55.47],'Pacific/Apia':['WS',-13.83,-171.73],'Pacific/Auckland':['NZ',-36.87,174.77],'Pacific/Bougainville':['PG',-6.22,155.57],'Pacific/Chatham':['NZ',-43.95,-176.55],'Pacific/Chuuk':['FM',7.42,151.78],'Pacific/Easter':['CL',-27.15,-109.43],'Pacific/Efate':['VU',-17.67,168.42],'Pacific/Fakaofo':['TK',-9.37,-171.23],'Pacific/Fiji':['FJ',-18.13,178.42],'Pacific/Funafuti':['TV',-8.52,179.22],'Pacific/Galapagos':['EC',-0.9,-89.6],'Pacific/Gambier':['PF',-23.13,-134.95],'Pacific/Guadalcanal':['SB',-9.53,160.2],'Pacific/Guam':['GU',13.47,144.75],'Pacific/Honolulu':['US',21.31,-157.86],'Pacific/Kanton':['KI',-2.78,-171.72],'Pacific/Kiritimati':['KI',1.87,-157.33],'Pacific/Kosrae':['FM',5.32,162.98],'Pacific/Kwajalein':['MH',9.08,167.33],'Pacific/Majuro':['MH',7.15,171.2],'Pacific/Marquesas':['PF',-9.0,-139.5],'Pacific/Midway':['UM',28.22,-177.37],'Pacific/Nauru':['NR',-0.52,166.92],'Pacific/Niue':['NU',-19.02,-169.92],'Pacific/Norfolk':['NF',-29.05,167.97],'Pacific/Noumea':['NC',-22.27,166.45],'Pacific/Pago_Pago':['AS',-14.27,-170.7],'Pacific/Palau':['PW',7.33,134.48],'Pacific/Pitcairn':['PN',-25.07,-130.08],'Pacific/Pohnpei':['FM',6.97,158.22],'Pacific/Port_Moresby':['PG',-9.5,147.17],'Pacific/Rarotonga':['CK',-21.23,-159.77],'Pacific/Saipan':['MP',15.2,145.75],'Pacific/Tahiti':['PF',-17.53,-149.57],'Pacific/Tarawa':['KI',1.42,173.0],'Pacific/Tongatapu':['TO',-21.13,-175.2],'Pacific/Wake':['UM',19.28,166.62],'Pacific/Wallis':['WF',-13.3,-176.17]};
  const paisDelEquipo = () => {
    try { const z = Intl.DateTimeFormat().resolvedOptions().timeZone; if (ZONAS[z] && centros[ZONAS[z][0]]) return ZONAS[z][0]; } catch (_) {}
    try { const r = (navigator.languages || [navigator.language]).map((l) => (String(l).split('-')[1] || '').toUpperCase()).find((x) => centros[x]); if (r) return r; } catch (_) {}
    return 'US';
  };
  // Where on the Earth the computer is drawn. Its time zone names a city (Bogotá, Madrid, São
  // Paulo…) whose coordinates the time zone database carries; without one, the country's centre.
  const zonaDelEquipo = () => { try { const z = Intl.DateTimeFormat().resolvedOptions().timeZone; return ZONAS[z] ? { z, cc: ZONAS[z][0], lat: ZONAS[z][1], lon: ZONAS[z][2] } : null; } catch (_) { return null; } };
  const lugarDe = (pais) => {
    const z = zonaDelEquipo();
    if (z && z.cc === pais) return { lon: z.lon, lat: z.lat, fuente: 'zona', ciudad: z.z.split('/').pop().replace(/_/g, ' ') };
    const c = centros[pais] || centros.US;
    return { lon: c.lon, lat: c.lat, fuente: 'pais' };
  };
  const UE = new Set(['AT', 'BE', 'BG', 'HR', 'CY', 'CZ', 'DK', 'EE', 'FI', 'FR', 'DE', 'GR', 'HU', 'IE', 'IT', 'LV', 'LT', 'LU', 'MT', 'NL', 'PL', 'PT', 'RO', 'SK', 'SI', 'ES', 'SE']);

  window.GuardianaUniverso = { iniciar };

  async function iniciar(ctx) {
    const { api, esc, LOC } = ctx;
    const t = ctx.t, tn = ctx.tn;
    const $ = (id) => document.getElementById(id);
    const cv = $('cv'), cx = cv.getContext('2d');
    const stage = $('stage'), lienzo = $('lienzo'), tip = $('tip');
    let nombrePais = (c) => c;
    try { const dn = new Intl.DisplayNames([LOC], { type: 'region' }); nombrePais = (c) => { try { return dn.of(c) || c; } catch (_) { return c; } }; } catch (_) {}
    const bytes = (n) => { const u = ['B', 'kB', 'MB', 'GB', 'TB']; let i = 0; n = Number(n || 0); while (n >= 1000 && i < u.length - 1) { n /= 1000; i++; } return (i ? n.toFixed(n < 10 ? 1 : 0) : Math.round(n)) + ' ' + u[i]; };
    const num = (n) => Number(n || 0).toLocaleString(LOC);
    const pct = (a, b) => (b ? Math.round(a / b * 100) : 0);
    const hora = (ms) => new Date(ms).toLocaleTimeString(LOC, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    const hace = (ms) => { const s = Math.max(0, Math.round(ms / 1000)); return s < 60 ? t('pd_hace_s').replace('{n}', s) : s < 3600 ? t('pd_hace_min').replace('{n}', Math.floor(s / 60)) : t('pd_hace_h').replace('{n}', Math.floor(s / 3600)).replace('{m}', String(Math.floor(s % 3600 / 60)).padStart(2, '0')); };
    const reducido = matchMedia('(prefers-reduced-motion: reduce)').matches;

    // ---------- Model: programs and destinations ----------
    // A destination is (program, name, country, network): the name the program asked for that
    // server, where the server is, and whose network it is. `sub`/`baj` add up the window.
    const PROD = [], DEST = [];
    const porClave = new Map();
    const COLORES = { antivirus: ['#8fdcff', '#cfa8ff', '#7fb2ff', '#b6c8ff'], navegador: ['#9df2c8', '#c6f68d', '#7ff0e0'], programa: ['#ffd1a6', '#f5b0d0', '#d4c4ff', '#a6e3e9', '#f3e08a', '#ffb3a6', '#c9d6ea', '#b7f0a8'] };
    const prod = (id) => PROD.find((p) => p.id === id) || PROD[0];
    // Stopped: a rule of this name, the whole program cut, or a wide rule (a category) that stopped
    // every lookup of a name that never left.
    const bloqueado = (d) => d.cortado || (prod(d.p) && prod(d.p).cortado) || !!d.ancho;
    const dibujable = (d) => !!(d.lugar || centros[d.pais]);
    // Where a destination is drawn (1.0.24): at the region or city its operator publishes, when it
    // does (exact), or around its country's centre (only the country is known), a small spiral
    // apart from the others in the same spot so each one can be seen and clicked.
    const sitio = (d) => {
      const clave = d.lugar ? `${d.lugar.lat},${d.lugar.lon}` : d.pais;
      const grupo = VISIBLES.filter((x) => (x.lugar ? `${x.lugar.lat},${x.lugar.lon}` : x.pais) === clave), k = Math.max(0, grupo.indexOf(d));
      const base = d.lugar ? { lon: d.lugar.lon, lat: d.lugar.lat } : centros[d.pais], paso = d.lugar ? 0.18 : 1.1;
      const ga = k * 2.399963, rr = grupo.length > 1 ? paso * Math.sqrt(k + 0.5) : 0;
      d.d = d.lugar ? 'exacto' : 'pais';
      return { lon: base.lon + Math.cos(ga) * rr / Math.max(0.35, Math.cos(base.lat * RAD)), lat: base.lat + Math.sin(ga) * rr };
    };
    // What the person's computer sent, in 10-second buckets: 30 of them are the deck's «last five
    // minutes», whatever the rhythm of the data (Windows is read every 10 s).
    const CUBO_MS = 10000, N_CUBOS = 30, CADA_MEDIDA = 10;
    const st = {
      vista: 'destinos', proy: 'globo', filtro: 'todo', yo: paisDelEquipo(), W: 0, Wc: 0, H: 0, dpr: 1, t0: performance.now(),
      dRot: 0, dTilt: 0, part: [], estallidos: [], hover: null, raton: null, dianas: [],
      serie: new Array(N_CUBOS).fill(0), serieP: {}, cuboT0: Date.now(), eventos: [], ahora: [0, 0, 0], intervalo: 5,
      datos: null, abierto: Date.now(), primeraCarga: true, ultimoGesto: performance.now(), tocado: false, vuelo: null, inercia: null, interactuando: false,
    };

    // A replay on the website (13 Oct 2026) draws the recorded computer where it was, by the time
    // zone the recording carries, never where the person watching is.
    if (ctx.zona && ZONAS[ctx.zona] && centros[ZONAS[ctx.zona][0]]) {
      const z = ZONAS[ctx.zona];
      st.yo = z[0];
      st.lugar = { lon: z[2], lat: z[1], fuente: 'zona', ciudad: ctx.zona.split('/').pop().replace(/_/g, ' ') };
    } else st.lugar = lugarDe(st.yo);
    // In a replay nothing can be cut: it would show a result that never happened.
    const soloMirar = !!ctx.reproduccion;
    const avanzaCubos = () => {
      const n = Math.min(N_CUBOS, Math.floor((Date.now() - st.cuboT0) / CUBO_MS));
      if (n <= 0) return;
      for (let k = 0; k < n; k++) { st.serie.push(0); st.serie.shift(); Object.values(st.serieP).forEach((v) => { v.push(0); v.shift(); }); }
      st.cuboT0 = n === N_CUBOS ? Date.now() : st.cuboT0 + n * CUBO_MS;
    };

    // ---------- Stars (fixed, made once) ----------
    let semilla = 7;
    const azar = () => { semilla = (semilla * 16807) % 2147483647; return semilla / 2147483647; };
    const gauss = () => { let u = 0, v = 0; while (!u) u = azar(); while (!v) v = azar(); return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v); };
    const estrellas = Array.from({ length: 320 }, () => ({ x: azar(), y: azar(), r: azar() * 1.2 + 0.2, f: azar() * 6.28 }));
    let paisesUsados = [];
    const tono = (c) => { let h = 0; for (const ch of c) h = (h * 31 + ch.charCodeAt(0)) % 360; return h; };

    // «Todo · Datos · Antivirus» (13 Oct 2026): the drawing shows everything, or only what the
    // browser and the other programs send, or only what the antivirus sends — the same split as
    // the two telemetries of «Datos en vivo».
    const enFiltro = (p) => st.filtro === 'todo' || (st.filtro === 'av') === (!!p && p.tipo === 'antivirus');
    // Every measured destination is drawn. A busy computer may have hundreds, but hiding the
    // quiet ones would make a newly opened page or program disappear from the map.
    let VISIBLES = [];
    const recalcula = () => {
      const orden = DEST.filter((d) => dibujable(d) && enFiltro(prod(d.p))).sort((a, b) => (b.sub + b.baj) - (a.sub + a.baj) || b.bloq - a.bloq);
      VISIBLES = orden;
      VISIBLES.forEach((d, i) => { d.i = i; });
      paisesUsados = [...new Set(VISIBLES.map((d) => d.pais))];
      PROD.forEach((p, k) => { if (!st.serieP[p.id]) st.serieP[p.id] = new Array(N_CUBOS).fill(0); p.k = k; });
    };
    const nuevoProducto = (p) => {
      const tipo = p.tipo || 'programa', usados = PROD.filter((x) => x.tipo === tipo).length, pal = COLORES[tipo] || COLORES.programa;
      const o = { id: p.id, nombre: p.id, tipo, procesos: p.procesos || [], firma: p.firma || '', cortable: !!p.cortable, cortado: !!p.cortado, color: pal[usados % pal.length], sub: p.enviados || 0, baj: p.recibidos || 0 };
      PROD.push(o); return o;
    };
    const claveDe = (d) => [d.programa, d.destino, d.pais, d.red].join('|');
    const nombreDe = (d) => d.destino || (d.red && d.red[0] !== '@' ? t('pd_sin_nombre_red').replace('{red}', d.red) : t('pd_sin_nombre'));

    // ---------- Nothing to draw: said in words, never filled with anything invented ----------
    // The page only draws measurements (the owner, 13 Oct 2026: no example data anywhere). Each
    // empty case says what is happening: no session (the panel's own «sin sesión» card, which
    // app.js shows), the first measurements on their way, nothing arriving after a while, an error
    // reading them, or a system that does not measure the bytes of each program.
    const ESPERA_MS = 30000;
    const VACIOS = {
      error: () => ({ titulo: t('pd_vacio_titulo'), texto: t('pd_error_lectura') }),
      midiendo: () => ({ titulo: t('pd_midiendo_titulo'), texto: t('pd_midiendo'), midiendo: true }),
      espera: () => ({ titulo: t('pd_vacio_titulo'), texto: t('pd_vacio') }),
      sin_bytes: () => ({ titulo: t('pd_sin_bytes_titulo'), texto: t('pd_sin_bytes'), enlace: true }),
    };
    let vacioAhora = null;
    const vacio = (cual) => {
      if (cual === vacioAhora) return;
      vacioAhora = cual;
      $('vacio').hidden = !cual;
      if (!cual) return;
      const v = VACIOS[cual]();
      $('vacio-titulo').textContent = v.titulo;
      $('vacio-titulo').classList.toggle('midiendo', !!v.midiendo);
      $('vacio-texto').textContent = v.texto;
      $('vacio-enlace').hidden = !v.enlace;
    };

    // ---------- Measurements: the panel's answer, and what changed since the last one ----------
    const filasVistas = new Map();
    let ultimaCarga = 0, primeraLectura = 0;
    const cargar = async () => {
      let r;
      try { r = await api('/api/por-dentro'); } catch (e) {
        // Without anything measured yet, the reason is said; with a picture already drawn, it
        // stays as it was and the next reading tries again. Without a session, the «sin sesión»
        // card says it already, and once is enough.
        if (!st.datos) vacio(String(e && e.message || '').includes('sesión') ? null : 'error');
        return;
      }
      if (!primeraLectura) primeraLectura = Date.now();
      const ahora = performance.now(), intervalo = ultimaCarga ? Math.max(1, (ahora - ultimaCarga) / 1000) : 5;
      ultimaCarga = ahora; st.intervalo = intervalo; st.datos = r;
      pintaVpn(r.vpn);
      r.programas.forEach((p) => {
        let o = PROD.find((x) => x.id === p.id);
        if (!o) o = nuevoProducto(p);
        Object.assign(o, { tipo: p.tipo, procesos: p.procesos, firma: p.firma, cortable: p.cortable, cortado: p.cortado, sub: p.enviados, baj: p.recibidos, act: p.actividad || null });
      });
      const rec = [], vistos = new Set();
      r.destinos.forEach((x) => {
        const k = claveDe(x); vistos.add(k);
        let d = porClave.get(k);
        const esNuevo = !d;
        if (!d) { d = { k, p: x.programa, ruta: [], tg: 0.3, x: 0, y: 0, vis: true, ema: 0, bloq: x.paradas }; porClave.set(k, d); DEST.push(d); }
        const antes = d.bloq || 0;
        Object.assign(d, { host: nombreDe(x), destino: x.destino, pais: x.pais, red: x.red, asn: x.asn, proto: x.proto, puerto: x.puerto, vias: x.vias, sub: x.enviados, baj: x.recibidos, paq: x.paquetes, noche: x.noche, primera: x.primera, ultima: x.ultima, doc: x.documentado, uso: x.uso, consultas: x.consultas, bloq: x.paradas, regla: x.regla ?? null, cortado: x.regla !== null && x.regla !== undefined, sede: x.sede, empresa: x.empresa || '', lugar: x.lugar || null });
        d.ancho = !d.cortado && d.sede && d.bloq > 0;
        if (!st.primeraCarga && esNuevo) {
          d.visto = Date.now();
          evento(prod(d.p).nombre, d.host, t('pd_ev_nuevo').replace('{pais}', d.pais || '—').replace('{red}', d.red || '—'), 'n');
          if (prod(d.p).tipo === 'antivirus') avisa(t('pd_aviso_nuevo').replace('{prog}', prod(d.p).nombre).replace('{nombre}', d.host).replace('{pais}', d.pais ? nombrePais(d.pais) : '—'));
        }
        if (!st.primeraCarga && x.paradas > antes) rec.push({ d, env: 0, recb: 0, paq: 0, parada: x.paradas - antes });
      });
      for (let i = DEST.length - 1; i >= 0; i--) if (!vistos.has(DEST[i].k)) { porClave.delete(DEST[i].k); DEST.splice(i, 1); }
      r.recientes.forEach((f) => {
        const kf = [f.minuto, f.programa, f.destino, f.pais, f.red, f.proto, f.puerto].join('|');
        const prev = filasVistas.get(kf);
        filasVistas.set(kf, { e: f.enviados, r: f.recibidos, p: f.paquetes, m: f.minuto });
        let de, dr, dp;
        if (prev) { de = f.enviados - prev.e; dr = f.recibidos - prev.r; dp = f.paquetes - prev.p; } else if (st.primeraCarga && f.minuto < r.ahora - 70000) {
          // Opening the page: the latest minutes go to the log at once, without particles.
          const d0 = porClave.get([f.programa, f.destino, f.pais, f.red].join('|'));
          if (d0 && f.enviados > 0) st.eventos.push({ h: ctx.reloj ? ctx.reloj() : hora(f.minuto), prog: prod(d0.p).nombre, host: d0.host, que: `${bytes(f.enviados)} ↑ · ${bytes(f.recibidos)} ↓ · ${f.pais || '—'} · ${f.red || '—'}`, cls: 'o', av: prod(d0.p).tipo === 'antivirus' });
          return;
        } else { de = f.enviados; dr = f.recibidos; dp = f.paquetes; }
        if (de <= 0 && dr <= 0) return;
        const d = porClave.get([f.programa, f.destino, f.pais, f.red].join('|'));
        if (d) rec.push({ d, env: Math.max(0, de), recb: Math.max(0, dr), paq: Math.max(0, dp), parada: 0 });
      });
      for (const [k, v] of filasVistas) if (v.m < r.ahora - 5 * 60000) filasVistas.delete(k);
      recalcula();
      aplicaDeltas(rec, Math.max(intervalo, CADA_MEDIDA));
      st.primeraCarga = false;
      // Off Windows the bytes of each program are not measured: nothing will come, and that is
      // what the page says, instead of «midiendo» for ever.
      vacio(DEST.length ? null : r.mide_bytes === false ? 'sin_bytes' : Date.now() - primeraLectura < ESPERA_MS ? 'midiendo' : 'espera');
      $('vigilancia-off').hidden = !(r.vigilando === false && r.mide_bytes !== false);
      pintaHud();
    };
    // What arrived in this beat becomes particles along each route, spread over the beat so the
    // flow looks continuous, and lines in the log. The totals are the panel's, not added here.
    const aplicaDeltas = (rec, segundos) => {
      graba(rec);
      let env = 0, recb = 0, paq = 0;
      const porProd = {}, grupo = { datos: [0, 0], av: [0, 0] };
      DEST.forEach((d) => { d.ema = (d.ema || 0) * 0.7; });
      rec.forEach(({ d, env: e, recb: b, paq: q, parada }) => {
        const p = prod(d.p);
        if (parada) {
          for (let k = 0; k < Math.min(3, parada); k++) nuevo(d, 'out', true, 2000, azar() * segundos * 900);
          evento(p.nombre, d.host, t('pd_ev_parado'), 'c');
          return;
        }
        env += e; recb += b; paq += q; porProd[d.p] = (porProd[d.p] || 0) + e;
        const g = grupo[p.tipo === 'antivirus' ? 'av' : 'datos']; g[0] += e; g[1] += b;
        d.ema += e / segundos * 0.3; d.ult = performance.now();
        const n = Math.min(4, 1 + Math.floor(Math.log10(1 + e / 3000)));
        for (let k = 0; k < n; k++) nuevo(d, 'out', false, e, azar() * segundos * 900);
        if (b > 0) nuevo(d, 'in', false, b, 300 + azar() * segundos * 800);
        evento(p.nombre, d.host, `${bytes(e)} ↑ · ${bytes(b)} ↓ · ${d.pais || '—'} · ${d.red || '—'}${d.asn ? ' AS' + d.asn : ''}${d.proto ? ' · ' + d.proto + '/' + d.puerto : ''}`, 'o');
      });
      avanzaCubos();
      st.serie[N_CUBOS - 1] += env;
      Object.keys(st.serieP).forEach((k) => { st.serieP[k][N_CUBOS - 1] += porProd[k] || 0; });
      // «Now» is the latest measurement: a beat that brought nothing new keeps it until the next
      // reading of Windows is due, so the figure does not jump between double and zero.
      const hayDatos = env > 0 || recb > 0, ahoraMs = performance.now();
      if (hayDatos || !st.ultimoDato || ahoraMs - st.ultimoDato > (CADA_MEDIDA + 2) * 1000) {
        st.ahora = [env / segundos, recb / segundos, paq / segundos];
        st.ahoraG = { datos: grupo.datos.map((x) => x / segundos), av: grupo.av.map((x) => x / segundos) };
      }
      if (hayDatos) st.ultimoDato = ahoraMs;
      pintaHud();
    };
    const nuevo = (d, dir, bloq, tam, retraso) => {
      setTimeout(() => {
        if (st.part.length > 280 || !VISIBLES.includes(d)) return;
        st.part.push({ d, dir, bloq, t: dir === 'out' ? 0 : 1, v: (reducido ? 0.16 : 0.34) + azar() * 0.16, r: Math.min(3.2, 1.1 + Math.log10(1 + tam) * 0.42) });
      }, retraso || 0);
    };
    const evento = (prog, host, que, cls) => {
      const pp = PROD.find((x) => x.nombre === prog);
      st.eventos.unshift({ h: ctx.reloj ? ctx.reloj() : hora(Date.now()), prog, host, que, cls, av: !!pp && pp.tipo === 'antivirus' });
      if (st.eventos.length > 40) st.eventos.length = 40;
    };

    // ---------- Size ----------
    let fondoListo = false;
    const ajusta = () => {
      const r = lienzo.getBoundingClientRect();
      st.dpr = Math.min(window.devicePixelRatio || 1, 3);
      st.Wc = Math.max(1, r.width); st.W = st.Wc; st.H = Math.max(1, r.height);
      cv.width = Math.round(st.Wc * st.dpr); cv.height = Math.round(st.H * st.dpr);
      fondoListo = false;
    };
    new ResizeObserver(ajusta).observe(lienzo);

    // ---------- Geometry ----------
    const bez2 = (a, c, b, u) => ({ x: (1 - u) * (1 - u) * a.x + 2 * (1 - u) * u * c.x + u * u * b.x, y: (1 - u) * (1 - u) * a.y + 2 * (1 - u) * u * c.y + u * u * b.y });
    const N = 48;
    // A route is a list of sampled points; `tg` is the fraction where it crosses GUARDIANA's gate.
    const tramo = (seg, n) => { const out = []; for (let i = 0; i <= n; i++) out.push(seg(i / n)); return out; };
    const punto = (ruta, u) => { if (!ruta.length) return { x: 0, y: 0, v: false }; const f = Math.max(0, Math.min(1, u)) * (ruta.length - 1); const i = Math.floor(f); const a = ruta[i], b = ruta[Math.min(i + 1, ruta.length - 1)]; const k = f - i; return { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k, v: a.v !== false && b.v !== false }; };
    const tiempo = () => (performance.now() - st.t0) / 1000;
    const prodsVisibles = () => PROD.filter((p) => VISIBLES.some((d) => d.p === p.id));
    const lado = (d) => { const p = prod(d.p); return ((p ? p.k : 0) % 3) - 1; };
    // Each watched program inside the ring, turning slowly.
    const posProductos = (c, r) => { const ps = prodsVisibles(); return ps.map((p, k) => { const a = tiempo() * (reducido ? 0.03 : 0.12) + k * 2 * Math.PI / Math.max(1, ps.length); return { p, x: c.x + Math.cos(a) * r, y: c.y + Math.sin(a) * r }; }); };
    const posDe = (prods, d, c) => prods.find((q) => q.p.id === d.p) || { x: c.x, y: c.y };

    // =====================================================================================
    // The Earth in WebGL2 (globe and map are the same mesh: the map is the globe unrolled).
    // Texturas de la NASA (Blue Marble de día, luces de ciudades de noche) y la luz del sol
    // de este mismo momento: donde ahora es de noche, en la pantalla también lo es.
    // =====================================================================================
    const glc = $('gl');
    const T = { ok: false, m: 0, mObj: 0, zoomG: 1, zoomM: 1, px: 0, py: 0, dist: 5, sol: [0, 0, 1], solLL: [0, 0], tanH: Math.tan(15 * RAD), M: null, PV: null, C: [0, 0, 5], asp: 1 };
    const gl = glc.getContext('webgl2', { antialias: true, alpha: false, powerPreference: 'default' });

    // --- matrices (columna a columna, como las quiere WebGL) ---
    const mul = (a, b) => { const o = new Float32Array(16); for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) { let s = 0; for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k]; o[c * 4 + r] = s; } return o; };
    const persp = (asp) => { const t = 1 / T.tanH, n = 0.1, f = 60, o = new Float32Array(16); o[0] = t / asp; o[5] = t; o[10] = (f + n) / (n - f); o[11] = -1; o[14] = 2 * f * n / (n - f); return o; };
    const trasl = (x, y, z) => new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1]);
    const rotX = (t) => { const c = Math.cos(t), s = Math.sin(t); return new Float32Array([1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]); };
    const rotY = (t) => { const c = Math.cos(t), s = Math.sin(t); return new Float32Array([c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]); };
    const aplica = (m, v) => [m[0] * v[0] + m[4] * v[1] + m[8] * v[2] + m[12], m[1] * v[0] + m[5] * v[1] + m[9] * v[2] + m[13], m[2] * v[0] + m[6] * v[1] + m[10] * v[2] + m[14], m[3] * v[0] + m[7] * v[1] + m[11] * v[2] + m[15]];

    // --- el sol de ahora: punto subsolar (declinación y ecuación del tiempo) ---
    const solAhora = () => {
      const d = new Date(), ini = Date.UTC(d.getUTCFullYear(), 0, 0), N = (d - ini) / 86400000;
      const h = d.getUTCHours() + d.getUTCMinutes() / 60 + d.getUTCSeconds() / 3600;
      const dec = -23.44 * Math.cos(2 * Math.PI / 365 * (N + 10));
      const B = 2 * Math.PI * (N - 81) / 364, eot = 9.87 * Math.sin(2 * B) - 7.53 * Math.cos(B) - 1.5 * Math.sin(B);
      let lon = -15 * (h - 12 + eot / 60); lon = ((lon + 540) % 360) - 180;
      T.solLL = [lon, dec];
      const cl = Math.cos(dec * RAD);
      T.sol = [cl * Math.sin(lon * RAD), Math.sin(dec * RAD), cl * Math.cos(lon * RAD)];
    };
    solAhora(); setInterval(solAhora, 30000);
    const esDeDia = (lon, lat) => { const cl = Math.cos(lat * RAD); return cl * Math.sin(lon * RAD) * T.sol[0] + Math.sin(lat * RAD) * T.sol[1] + cl * Math.cos(lon * RAD) * T.sol[2] > 0; };

    // --- shaders ---
    const VS_TIERRA = `#version 300 es
    in vec2 aLL;
    uniform mat4 uPV, uM; uniform float uMorph;
    out vec2 vUv; out vec3 vN; out vec3 vW;
    void main() {
      float lon = aLL.x, lat = aLL.y;
      vec3 s = vec3(cos(lat) * sin(lon), sin(lat), cos(lat) * cos(lon));
      vec3 p = mix(s, vec3(lon, lat, 0.0), uMorph);
      vec4 w = uM * vec4(p, 1.0);
      vN = s; vW = w.xyz;
      vUv = vec2(lon / 6.2831853 + 0.5, 0.5 - lat / 3.1415927);
      gl_Position = uPV * w;
    }`;
    const FS_TIERRA = `#version 300 es
    precision highp float;
    uniform sampler2D uDia, uLuces, uAgua;
    // De cerca (15 Oct 2026): hasta cuatro teselas de 45° a 2048 px, cuatro veces más finas que
    // la Tierra entera. Se muestrean siempre (así el mipmap no se rompe en los bordes) y solo
    // cuentan donde el píxel cae dentro de su rectángulo.
    uniform sampler2D uDet0, uDet1, uDet2, uDet3, uLuz0, uLuz1, uLuz2, uLuz3;
    uniform vec4 uRect0, uRect1, uRect2, uRect3; uniform vec4 uHay;
    uniform vec3 uSol, uCam; uniform mat3 uM3; uniform float uMorph;
    in vec2 vUv; in vec3 vN; in vec3 vW;
    out vec4 o;
    vec3 aces(vec3 x) { return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
    vec2 enTesela(vec4 r) { vec2 t = clamp((vUv - r.xy) / (r.zw - r.xy), 0.0, 1.0); return (8.0 + t * 2032.0) / 2048.0; }
    float dentro(vec4 r, float hay) { return hay * step(r.x, vUv.x) * step(vUv.x, r.z) * step(r.y, vUv.y) * step(vUv.y, r.w); }
    void main() {
      vec3 n = normalize(vN);
      float mu = dot(n, uSol);
      vec3 diaT = texture(uDia, vUv).rgb;
      float luz = texture(uLuces, vUv).r;
      float d0 = dentro(uRect0, uHay.x), d1 = dentro(uRect1, uHay.y), d2 = dentro(uRect2, uHay.z), d3 = dentro(uRect3, uHay.w);
      vec2 q0 = enTesela(uRect0), q1 = enTesela(uRect1), q2 = enTesela(uRect2), q3 = enTesela(uRect3);
      diaT = mix(diaT, texture(uDet0, q0).rgb, d0); luz = mix(luz, texture(uLuz0, q0).r, d0);
      diaT = mix(diaT, texture(uDet1, q1).rgb, d1); luz = mix(luz, texture(uLuz1, q1).r, d1);
      diaT = mix(diaT, texture(uDet2, q2).rgb, d2); luz = mix(luz, texture(uLuz2, q2).r, d2);
      diaT = mix(diaT, texture(uDet3, q3).rgb, d3); luz = mix(luz, texture(uLuz3, q3).r, d3);
      vec3 dia = pow(diaT, vec3(2.2));
      float agua = texture(uAgua, vUv).r;
      float lado = smoothstep(-0.08, 0.22, mu);
      // Luz del sol con una noche suave (the owner, 13 Oct 2026: «se ve muy oscuro… que se vea el
      // día y la noche, pero más suave»): the night side keeps its land and seas, a little bluer,
      // and the day side gets the full sun. The line between them is soft, as dusk is.
      float diaF = smoothstep(-0.14, 0.24, mu);
      vec3 noct = dia * vec3(0.30, 0.36, 0.50);
      vec3 col = mix(noct, dia * (0.62 + 0.68 * max(mu, 0.0)), diaF);
      // Crepúsculo: banda cálida a lo largo del terminador.
      col += dia * vec3(1.0, 0.45, 0.2) * 0.22 * exp(-pow((mu - 0.03) * 10.0, 2.0));
      // Luces de las ciudades, solo donde es de noche.
      float noche = 1.0 - smoothstep(-0.20, 0.05, mu);
      col += vec3(1.0, 0.70, 0.38) * pow(luz, 1.25) * 1.5 * noche;
      vec3 nw = normalize(uM3 * n);
      vec3 V = normalize(uCam - vW);
      vec3 Lw = normalize(uM3 * uSol);
      float g = 1.0 - uMorph;
      // Brillo del sol en el océano (solo en el agua y solo en el globo).
      vec3 H = normalize(Lw + V);
      float nh = max(dot(nw, H), 0.0);
      float spec = (pow(nh, 900.0) * 0.32 + pow(nh, 90.0) * 0.05 + pow(nh, 12.0) * 0.012) * agua * lado * g;
      col += vec3(1.0, 0.86, 0.66) * spec;
      col += vec3(0.02, 0.06, 0.13) * agua * lado * 0.6;
      // Atmósfera vista de canto (Fresnel), azul de día, apagada de noche.
      float fres = pow(1.0 - max(dot(nw, V), 0.0), 2.6) * g;
      vec3 atm = vec3(0.28, 0.55, 1.0) * fres * (0.08 + 0.92 * smoothstep(-0.3, 0.45, mu));
      col = col * (1.0 - fres * 0.35) + atm * 0.85;
      // En el mapa plano, un leve sombreado hacia el terminador mantiene el relieve del día.
      col = aces(col * 1.3);
      o = vec4(pow(col, vec3(1.0 / 2.2)), 1.0);
    }`;
    const VS_PANTALLA = `#version 300 es
    in vec2 aP; out vec2 vP;
    void main() { vP = aP; gl_Position = vec4(aP, 0.0, 1.0); }`;
    const FS_FONDO = `#version 300 es
    precision highp float;
    uniform sampler2D uCielo; uniform vec2 uRes; uniform float uDesp;
    uniform vec2 uCentro; uniform float uRadio, uHalo; uniform vec2 uSolP;
    in vec2 vP; out vec4 o;
    void main() {
      vec2 uv = vec2(gl_FragCoord.x / uRes.y * 0.275 + uDesp, gl_FragCoord.y / uRes.y * 0.55 + 0.22);
      vec3 c = texture(uCielo, uv).rgb;
      // Halo de la atmósfera alrededor del globo, más intenso del lado del sol.
      vec2 d = gl_FragCoord.xy - uCentro;
      float r = length(d) / uRadio;
      if (r > 1.0) {
        float lado = 0.25 + 0.75 * smoothstep(-0.7, 0.8, dot(normalize(d), uSolP));
        float h = exp(-(r - 1.0) * 14.0) * 0.9 + exp(-(r - 1.0) * 3.5) * 0.18;
        c += vec3(0.30, 0.58, 1.0) * h * lado * uHalo;
      }
      o = vec4(c, 1.0);
    }`;
    const compila = (tipo, src) => { const s = gl.createShader(tipo); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
    const programa = (vs, fs) => { const p = gl.createProgram(); gl.attachShader(p, compila(gl.VERTEX_SHADER, vs)); gl.attachShader(p, compila(gl.FRAGMENT_SHADER, fs)); gl.linkProgram(p); if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p)); const u = {}; const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS); for (let i = 0; i < n; i++) { const a = gl.getActiveUniform(p, i); u[a.name] = gl.getUniformLocation(p, a.name); } return { p, u }; };

    // Cielo: estrellas y una Vía Láctea tenue, generadas una vez.
    const cielo = () => {
      const c = document.createElement('canvas'); c.width = 2048; c.height = 1024; const g = c.getContext('2d');
      g.fillStyle = '#02040a'; g.fillRect(0, 0, 2048, 1024);
      // Cada mancha y cada estrella se pinta también una vuelta a izquierda y derecha: el cielo encaja sin costura.
      for (let i = 0; i < 1400; i++) { const x = azar() * 2048, y = 512 + gauss() * 120 + Math.sin(x / 2048 * Math.PI * 2) * 140; const r = 30 + azar() * 90;
        for (const dx of [-2048, 0, 2048]) { const gr = g.createRadialGradient(x + dx, y, 0, x + dx, y, r); gr.addColorStop(0, 'rgba(120,140,190,0.035)'); gr.addColorStop(1, 'rgba(120,140,190,0)'); g.fillStyle = gr; g.fillRect(x + dx - r, y - r, 2 * r, 2 * r); } }
      for (let i = 0; i < 5200; i++) { const x = azar() * 2048, y = azar() < 0.45 ? 512 + gauss() * 150 + Math.sin(x / 2048 * Math.PI * 2) * 140 : azar() * 1024; const b = Math.pow(azar(), 6); const t = azar(); g.fillStyle = `rgba(${200 + 55 * t | 0},${210 + 30 * t | 0},${255 - 40 * (1 - t) | 0},${0.25 + b * 0.75})`; const s = 0.6 + b * 1.6; g.fillRect(x, y, s, s); }
      return c;
    };
    const textura = (fuente, mip, fija) => {
      const t = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, t);
      if (fuente) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, fuente);
      else gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([8, 20, 40, 255]));
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, fija ? gl.CLAMP_TO_EDGE : gl.REPEAT); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      if (fuente && mip) { gl.generateMipmap(gl.TEXTURE_2D); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR); const an = gl.getExtension('EXT_texture_filter_anisotropic'); if (an) gl.texParameterf(gl.TEXTURE_2D, an.TEXTURE_MAX_ANISOTROPY_EXT, Math.min(16, gl.getParameter(an.MAX_TEXTURE_MAX_ANISOTROPY_EXT))); }
      else gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      return t;
    };
    const DET = { cols: 8, filas: 4, hay: false, ruta: '', cache: new Map(), max: 20 };
    let PT, PF, vaoT, vaoF, nIdx, tex = {};
    if (gl) {
      try {
        PT = programa(VS_TIERRA, FS_TIERRA); PF = programa(VS_PANTALLA, FS_FONDO);
        // Malla: 360 × 180 pasos de longitud y latitud.
        const NL = 360, NA = 180, v = new Float32Array((NL + 1) * (NA + 1) * 2); let k = 0;
        for (let j = 0; j <= NA; j++) for (let i = 0; i <= NL; i++) { v[k++] = -Math.PI + 2 * Math.PI * i / NL; v[k++] = Math.PI / 2 - Math.PI * j / NA; }
        const ix = new Uint32Array(NL * NA * 6); k = 0;
        for (let j = 0; j < NA; j++) for (let i = 0; i < NL; i++) { const a = j * (NL + 1) + i, b = a + NL + 1; ix.set([a, b, a + 1, a + 1, b, b + 1], k); k += 6; }
        nIdx = ix.length;
        vaoT = gl.createVertexArray(); gl.bindVertexArray(vaoT);
        gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer()); gl.bufferData(gl.ARRAY_BUFFER, v, gl.STATIC_DRAW);
        const la = gl.getAttribLocation(PT.p, 'aLL'); gl.enableVertexAttribArray(la); gl.vertexAttribPointer(la, 2, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, gl.createBuffer()); gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, ix, gl.STATIC_DRAW);
        vaoF = gl.createVertexArray(); gl.bindVertexArray(vaoF);
        gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer()); gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
        const lp = gl.getAttribLocation(PF.p, 'aP'); gl.enableVertexAttribArray(lp); gl.vertexAttribPointer(lp, 2, gl.FLOAT, false, 0, 0);
        gl.bindVertexArray(null);
        tex = { dia: textura(null), luces: textura(null), agua: textura(null), cielo: textura(cielo(), true) };
        const carga = (clave, url) => { const im = new Image(); im.onload = () => { gl.deleteTexture(tex[clave]); tex[clave] = textura(im, true); }; im.src = url; };
        const ruta = ctx.tierra || '/static/tierra/';
          carga('dia', ruta + 'dia.jpg'); carga('luces', ruta + 'luces.jpg'); carga('agua', ruta + 'agua.png');
        DET.ruta = ruta + 't/';
        // Are the close-up tiles there? (A build without them still shows the whole Earth.)
        fetch(DET.ruta + 'tiles.json', { cache: 'force-cache' }).then((r) => (r.ok ? r.json() : null)).then((j) => { if (j && j.cols === DET.cols && j.filas === DET.filas) DET.hay = true; }).catch(() => {});
        T.ok = true;
      } catch (e) { console.error(e); T.ok = false; }
    }
    // --- De cerca: las teselas de 45° (build/tierra.py), cargadas solo bajo la vista ---
    // The whole Earth is 11 px per degree; a tile is 45. Below ~14 px per degree on screen the
    // whole Earth is enough; above it, the four tiles nearest the centre of the view take over,
    // one at a time as each arrives. Up to twenty stay in memory; the oldest unused ones go.
    const tesela = (c, r) => {
      const k = c + '-' + r; let e = DET.cache.get(k);
      if (e) { e.uso = tiempo(); return e; }
      e = { c, r, dia: null, luces: null, uso: tiempo(), rect: [c / DET.cols, r / DET.filas, (c + 1) / DET.cols, (r + 1) / DET.filas] };
      DET.cache.set(k, e);
      for (const q of ['dia', 'luces']) { const im = new Image(); im.onload = () => { if (DET.cache.get(k) === e) e[q] = textura(im, true, true); }; im.src = DET.ruta + q + '-' + c + '-' + r + '.jpg'; }
      if (DET.cache.size > DET.max) {
        const viejas = [...DET.cache.values()].sort((a, b) => a.uso - b.uso).slice(0, DET.cache.size - DET.max);
        viejas.forEach((v) => { if (v.dia) gl.deleteTexture(v.dia); if (v.luces) gl.deleteTexture(v.luces); DET.cache.delete(v.c + '-' + v.r); });
      }
      return e;
    };
    const teselasVisibles = () => {
      if (!DET.hay || T.m > 0.2 && T.m < 0.8 || pxPorGrado() < 8) return [];
      const c = lonLatBajo(st.W / 2, st.H / 2);
      if (!c) return [];
      const fx = (c[0] + 180) / 45, fy = (90 - c[1]) / 45;
      const tc = Math.min(DET.cols - 1, Math.max(0, Math.floor(fx))), tr = Math.min(DET.filas - 1, Math.max(0, Math.floor(fy)));
      const nc = (tc + (fx - tc < 0.5 ? -1 : 1) + DET.cols) % DET.cols, nr = tr + (fy - tr < 0.5 ? -1 : 1);
      const lista = [[tc, tr], [nc, tr]];
      if (nr >= 0 && nr < DET.filas) lista.push([tc, nr], [nc, nr]);
      return lista.map(([a, b]) => tesela(a, b)).filter((e) => e.dia && e.luces);
    };

    // Cámara y modelo de este cuadro. m = 0 globo, 1 mapa.
    const camara = () => {
      const W = st.Wc, H = st.H, asp = W / H, aspU = st.W / H; T.asp = asp;
      const yo = st.lugar;
      // The globe sways a little by itself until the person takes it in hand (13 Oct 2026).
      const osc = reducido || st.tocado ? 0 : Math.sin(tiempo() * 0.05) * 0.35;
      T.osc = osc;
      const rot = -yo.lon * RAD + osc + st.dRot, tilt = Math.max(-1.4, Math.min(1.4, yo.lat * RAD * 0.8 + st.dTilt));
      const g = 1 - T.m;
      const dGlobo = 1 / Math.sin(Math.atan(0.84 * T.tanH * Math.min(1, aspU)));
      const dMapa = Math.max((Math.PI / 2) / (T.tanH * 0.94), Math.PI / (T.tanH * aspU * 0.985));
      // Zoom (the owner, 13 Oct 2026: «que se pueda hacer zoom… hasta un punto máximo»): on the
      // globe it brings the camera closer to the surface (the height above it shrinks), on the map
      // it narrows the view; each has its own limits (ZOOM).
      T.dGlobo = dGlobo; T.dMapa = dMapa;
      const dist = (1 + (dGlobo - 1) / T.zoomG) * g + (dMapa / T.zoomM) * T.m;
      // Con el panel de datos abierto, el mundo se centra en el espacio que queda libre.
      const desplaza = ((W - st.W) / 2) * (2 * dist * T.tanH / H);
      const px = T.px * T.m + desplaza, py = T.py * T.m;
      T.M = mul(rotX(tilt * g), rotY(rot * g));
      T.PV = mul(persp(asp), trasl(-px, -py, -dist));
      T.C = [px, py, dist]; T.dist = dist; T.rot = rot * g; T.tilt = tilt * g; T.tiltSin = tilt;
    };
    // Un punto local (marco de la Tierra) a pantalla, y si se ve (no está detrás del globo).
    const aPantalla = (q) => {
      const w = aplica(T.M, q), c = aplica(T.PV, [w[0], w[1], w[2]]);
      const x = (c[0] / c[3] + 1) / 2 * st.Wc, y = (1 - c[1] / c[3]) / 2 * st.H;
      let v = c[3] > 0;
      if (v && T.m < 0.5) {
        const C = T.C, D = [w[0] - C[0], w[1] - C[1], w[2] - C[2]], L = Math.hypot(D[0], D[1], D[2]);
        D[0] /= L; D[1] /= L; D[2] /= L;
        const b = C[0] * D[0] + C[1] * D[1] + C[2] * D[2], cc = C[0] * C[0] + C[1] * C[1] + C[2] * C[2] - 1, disc = b * b - cc;
        if (disc > 0) { const t1 = -b - Math.sqrt(disc); if (t1 > 0 && t1 < L - 0.012) v = false; }
      }
      return { x, y, v, w };
    };
    // Un lugar de la Tierra a una altura (fracción del radio), mezclando globo y mapa.
    const lugar = (lon, lat, alt = 0, alza = 0) => {
      const l = lon * RAD, a = lat * RAD, cl = Math.cos(a), h = 1 + alt;
      const s = [cl * Math.sin(l) * h, Math.sin(a) * h, cl * Math.cos(l) * h], p = [l, a + alza, alt * 0.35];
      const m = T.m; return [s[0] + (p[0] - s[0]) * m, s[1] + (p[1] - s[1]) * m, s[2] + (p[2] - s[2]) * m];
    };
    const pintaTierra = () => {
      if (!T.ok) return false;
      // Más calidad (the owner, 16 Oct 2026): the Earth is drawn at 1.5× the screen at least and the
      // browser scales it down, which smooths coasts and city lights like supersampling.
      const k = Math.min(3, Math.max(st.dpr, 1.5));
      const W = Math.round(st.Wc * k), H = Math.round(st.H * k);
      if (glc.width !== W || glc.height !== H) { glc.width = W; glc.height = H; }
      gl.viewport(0, 0, W, H);
      // Fondo y halo.
      const o = aPantalla([0, 0, 0]);
      const rpx = Math.tan(Math.asin(1 / T.dist)) / T.tanH * (st.H / 2);
      const Lw = aplica(T.M, T.sol), ls = Math.hypot(Lw[0], Lw[1]) || 1;
      gl.disable(gl.DEPTH_TEST); gl.useProgram(PF.p); gl.bindVertexArray(vaoF);
      gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, tex.cielo); gl.uniform1i(PF.u.uCielo, 0);
      gl.uniform2f(PF.u.uRes, W, H); gl.uniform1f(PF.u.uDesp, -T.rot * 0.04);
      gl.uniform2f(PF.u.uCentro, o.x * k, H - o.y * k); gl.uniform1f(PF.u.uRadio, rpx * k);
      gl.uniform1f(PF.u.uHalo, Math.max(0, 1 - T.m * 1.6)); gl.uniform2f(PF.u.uSolP, Lw[0] / ls, Lw[1] / ls);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      // La Tierra.
      gl.enable(gl.DEPTH_TEST); gl.clear(gl.DEPTH_BUFFER_BIT);
      gl.useProgram(PT.p); gl.bindVertexArray(vaoT);
      gl.uniformMatrix4fv(PT.u.uPV, false, T.PV); gl.uniformMatrix4fv(PT.u.uM, false, T.M);
      gl.uniformMatrix3fv(PT.u.uM3, false, new Float32Array([T.M[0], T.M[1], T.M[2], T.M[4], T.M[5], T.M[6], T.M[8], T.M[9], T.M[10]]));
      gl.uniform1f(PT.u.uMorph, T.m); gl.uniform3fv(PT.u.uSol, T.sol); gl.uniform3fv(PT.u.uCam, T.C);
      [['uDia', tex.dia], ['uLuces', tex.luces], ['uAgua', tex.agua]].forEach(([n, t], i) => { gl.activeTexture(gl.TEXTURE1 + i); gl.bindTexture(gl.TEXTURE_2D, t); gl.uniform1i(PT.u[n], 1 + i); });
      const det = teselasVisibles(), hay = [0, 0, 0, 0];
      for (let i = 0; i < 4; i++) {
        const e = det[i];
        gl.activeTexture(gl.TEXTURE4 + i); gl.bindTexture(gl.TEXTURE_2D, e ? e.dia : tex.dia); gl.uniform1i(PT.u['uDet' + i], 4 + i);
        gl.activeTexture(gl.TEXTURE8 + i); gl.bindTexture(gl.TEXTURE_2D, e ? e.luces : tex.luces); gl.uniform1i(PT.u['uLuz' + i], 8 + i);
        gl.uniform4fv(PT.u['uRect' + i], e ? e.rect : [0, 0, 0, 0]); hay[i] = e ? 1 : 0;
      }
      gl.uniform4fv(PT.u.uHay, hay);
      gl.drawElements(gl.TRIANGLES, nIdx, gl.UNSIGNED_INT, 0);
      gl.bindVertexArray(null);
      return true;
    };

    // Two hex digits of opacity, to append to a #rrggbb colour.
    const alfa = (a) => Math.round(Math.max(0, Math.min(1, a)) * 255).toString(16).padStart(2, '0');
    // Brillos ya dibujados (uno por color), para que las partículas parezcan luz y no puntos.
    const brillos = {};
    const brillo = (col) => {
      if (brillos[col]) return brillos[col];
      const c = document.createElement('canvas'); c.width = c.height = 64; const g = c.getContext('2d');
      const gr = g.createRadialGradient(32, 32, 0, 32, 32, 32);
      gr.addColorStop(0, 'rgba(255,255,255,.95)'); gr.addColorStop(0.07, col); gr.addColorStop(0.3, col + '66'); gr.addColorStop(1, col + '00');
      g.fillStyle = gr; g.fillRect(0, 0, 64, 64); return (brillos[col] = c);
    };

    // --- Globo y mapa: rutas, columnas de luz, escudo y rótulos sobre la Tierra ---
    let GRUPOS = [];
    const baseLugar = (d) => {
      const b = d.lugar ? { lon: d.lugar.lon, lat: d.lugar.lat } : centros[d.pais];
      d.d = d.lugar ? 'exacto' : 'pais';
      return b;
    };
    // One line per city. A state is not a line: Dallas and Houston each keep their own,
    // however far the view is. Without a published city, the line is the region, or the country.
    const sitioDe = (d) => {
      const ciudad = d.lugar && d.lugar.ciudad ? d.lugar.ciudad.trim() : '';
      const region = d.lugar && d.lugar.region ? d.lugar.region.trim() : '';
      const bruto = ((d.lugar && d.lugar.pais) || d.pais || '').trim();
      const pais = bruto && bruto[0] !== '@' ? bruto : '';
      return { ciudad, region, pais };
    };
    const agrupa = () => {
      const map = new Map();
      VISIBLES.forEach((d) => {
        const b = baseLugar(d);
        if (!b) return;
        d.lon = b.lon; d.lat = b.lat;
        const s = sitioDe(d);
        const key = (s.ciudad ? 'c|' + s.pais + '|' + s.region + '|' + s.ciudad : s.region ? 'r|' + s.pais + '|' + s.region : 'p|' + s.pais).toLowerCase();
        let g = map.get(key);
        if (!g) { g = { ds: [], key, ciudad: s.ciudad, region: s.region, pais: s.pais }; map.set(key, g); }
        g.ds.push(d);
      });
      const grupos = [...map.values()];
      grupos.forEach((g) => {
        g.ds.sort((a, c) => (c.sub + c.baj) - (a.sub + a.baj) || c.bloq - a.bloq);
        g.top = g.ds[0];
        g.lon = g.ds.reduce((a, d) => a + d.lon, 0) / g.ds.length;
        g.lat = g.ds.reduce((a, d) => a + d.lat, 0) / g.ds.length;
      });
      return grupos;
    };
    const aplicaRuta = (g) => { g.ds.forEach((d) => { d.ruta = g.ruta; d.tg = g.tg; d.vis = g.vis; d.x = g.x; d.y = g.y; d.base = g.base; }); };
    const layoutTierra = () => {
      camara();
      const yo = st.lugar, cen = aPantalla(lugar(yo.lon, yo.lat, 0.002));
      const R0 = 26;
      const prods = posProductos(cen, 11);
      GRUPOS = agrupa();
      GRUPOS.forEach((g) => {
        const d = g.top, lon1 = g.lon, lat1 = g.lat;
        const a = [Math.cos(yo.lat * RAD) * Math.sin(yo.lon * RAD), Math.sin(yo.lat * RAD), Math.cos(yo.lat * RAD) * Math.cos(yo.lon * RAD)];
        const b = [Math.cos(lat1 * RAD) * Math.sin(lon1 * RAD), Math.sin(lat1 * RAD), Math.cos(lat1 * RAD) * Math.cos(lon1 * RAD)];
        const om = Math.acos(Math.max(-1, Math.min(1, a[0] * b[0] + a[1] * b[1] + a[2] * b[2]))) || 1e-6;
        const alt = 0.012 + 0.055 * om / Math.PI;
        const dl = Math.hypot(lon1 - yo.lon, lat1 - yo.lat) * RAD, alzaMax = 0.028 + 0.04 * dl / Math.PI;
        const arco = [];
        for (let i = 0; i <= N; i++) {
          const u = i / N, s1 = Math.sin((1 - u) * om) / Math.sin(om), s2 = Math.sin(u * om) / Math.sin(om), h = 1 + alt * Math.sin(Math.PI * u);
          const sx = (a[0] * s1 + b[0] * s2) * h, sy = (a[1] * s1 + b[1] * s2) * h, sz = (a[2] * s1 + b[2] * s2) * h;
          const pl = [(yo.lon + (lon1 - yo.lon) * u) * RAD, (yo.lat + (lat1 - yo.lat) * u) * RAD + alzaMax * Math.sin(Math.PI * u), 0.02];
          const m = T.m;
          arco.push(aPantalla([sx + (pl[0] - sx) * m, sy + (pl[1] - sy) * m, sz + (pl[2] - sz) * m]));
        }
        const pr = posDe(prods, d, cen);
        let ig = arco.findIndex((q) => Math.hypot(q.x - cen.x, q.y - cen.y) > R0);
        if (ig < 1) ig = 1;
        const gte = arco[ig];
        g.ruta = tramo((u) => ({ x: pr.x + (gte.x - pr.x) * u, y: pr.y + (gte.y - pr.y) * u, v: cen.v }), 5).concat(arco.slice(ig + 1));
        g.tg = 5 / (g.ruta.length - 1);
        const fin = g.ruta[g.ruta.length - 1];
        g.x = fin.x; g.y = fin.y; g.vis = fin.v;
        g.base = aPantalla(lugar(lon1, lat1, 0.002));
        aplicaRuta(g);
      });
      return { cen, R0, prods, yo };
    };
    // Un círculo pequeño sobre la superficie (radio en grados), ya en pantalla.
    const circuloSuelo = (lon, lat, rg, n = 40) => {
      const out = [], a0 = lat * RAD, l0 = lon * RAD, r = rg * RAD;
      for (let i = 0; i <= n; i++) {
        const th = i / n * Math.PI * 2;
        const la = Math.asin(Math.sin(a0) * Math.cos(r) + Math.cos(a0) * Math.sin(r) * Math.cos(th));
        const lo = l0 + Math.atan2(Math.sin(th) * Math.sin(r) * Math.cos(a0), Math.cos(r) - Math.sin(a0) * Math.sin(la));
        out.push(aPantalla(lugar(lo / RAD, la / RAD, 0.003)));
      }
      return out;
    };
    const trazoPuntos = (pts, col, ancho, guion, desp) => {
      cx.strokeStyle = col; cx.lineWidth = ancho; cx.setLineDash(guion || []); cx.lineDashOffset = desp || 0;
      cx.beginPath(); let ab = false; pts.forEach((q) => { if (!q.v) { ab = false; return; } if (!ab) { cx.moveTo(q.x, q.y); ab = true; } else cx.lineTo(q.x, q.y); }); cx.stroke(); cx.setLineDash([]); cx.lineDashOffset = 0;
    };
    const dibujaTierra = (dt) => {
      const L = layoutTierra(), tt = tiempo();
      cx.globalCompositeOperation = 'lighter';
      // Arcos: un velo ancho y un hilo fino, más vivo en el centro del recorrido.
      GRUPOS.forEach((g) => {
        const d = g.top, bl = bloqueado(d), col = bl ? '#ff6b7a' : prod(d.p).color;
        if (bl) {
          trazo(g.ruta, 0, g.tg, 'rgba(255,107,122,.75)', 0.7);
          trazo(g.ruta, g.tg, 1, 'rgba(160,175,200,.22)', 0.45, [1.2, 3.2]);
          return;
        }
        trazo(g.ruta, 0, 1, col + '24', 1.05);
        trazo(g.ruta, 0, 1, col + 'c8', 0.45);
      });
      pelotitas(tt);
      // Los puntos de llegada (15 Oct 2026): a pin, not a glow. The owner: «la gente tiene que
      // saber que tiene que tocar ahí… el fondo es negro y hay puntos de luz». A dark disc cuts
      // it out of the city lights, a white ring says «a control», the dot inside carries the
      // program's colour, and a slow breathing ring (one every 3 s, out of step per place) draws
      // the eye without blinking. Up close, the place is named next to it.
      cx.globalCompositeOperation = 'source-over';
      const puestos = [], ppg = pxPorGrado(), conNombre = ppg >= 7;
      const nombrados = conNombre ? new Set(GRUPOS.filter((g) => g.base.v).sort((a, b) => (b.top.sub + b.top.baj) - (a.top.sub + a.top.baj)).slice(0, ppg >= 20 ? 30 : 16)) : null;
      GRUPOS.forEach((g) => {
        if (!g.base.v) return;
        const d = g.top, bl = bloqueado(d), col = bl ? '#ff6b7a' : prod(d.p).color;
        const sobre = st.hover && (st.hover.ref === g || st.hover.ref === d);
        const x = g.base.x, y = g.base.y, r = sobre ? 8.5 : 6.5;
        if (!reducido) {
          const u = ((tt / 3.2 + ((g.lon * 7 + g.lat * 3) % 1 + 1)) % 1);
          cx.strokeStyle = (sobre ? '#ffffff' : col) + alfa(0.55 * (1 - u) * (1 - u)); cx.lineWidth = 1.2;
          cx.beginPath(); cx.arc(x, y, r + 2 + u * 12, 0, 7); cx.stroke();
        }
        cx.fillStyle = 'rgba(2,6,14,.82)'; cx.beginPath(); cx.arc(x, y, r + 2.4, 0, 7); cx.fill();
        cx.strokeStyle = bl ? '#ff6b7a' : sobre ? '#ffffff' : 'rgba(255,255,255,.92)'; cx.lineWidth = sobre ? 2 : 1.5;
        cx.beginPath(); cx.arc(x, y, r, 0, 7); cx.stroke();
        cx.fillStyle = col; cx.beginPath(); cx.arc(x, y, bl ? 2 : 2.4, 0, 7); cx.fill();
        if (bl) { cx.strokeStyle = '#ffd9df'; cx.lineWidth = 1.2; cx.beginPath(); cx.moveTo(x - 2.2, y - 2.2); cx.lineTo(x + 2.2, y + 2.2); cx.moveTo(x + 2.2, y - 2.2); cx.lineTo(x - 2.2, y + 2.2); cx.stroke(); }
        if (nombrados && nombrados.has(g) && !sobre) rotulo(x + r + 6, y + 8, g.ciudad || g.region || (g.pais ? nombrePais(g.pais) : d.host), bl ? '#ff8d99' : '#e9f1ff', puestos);
        st.dianas.push({ x, y, r: 18, tipo: 'lugar', ref: g });
      });
      cx.globalCompositeOperation = 'lighter';
      // El escudo de GUARDIANA sobre tu ordenador.
      if (L.cen.v) {
        [2.2, 3.6, 5.2].forEach((r, k) => trazoPuntos(circuloSuelo(L.yo.lon, L.yo.lat, r + (k === 2 ? Math.sin(tt * 1.6) * 0.3 : 0), 64), `rgba(63,224,197,${[0.9, 0.55, 0.3][k]})`, k ? 1 : 1.6, k === 1 ? [6, 5] : null, k === 1 ? -tt * 14 : 0));
        const cima = aPantalla(lugar(L.yo.lon, L.yo.lat, 0.07, 0.06));
        const gr = cx.createLinearGradient(L.cen.x, L.cen.y, cima.x, cima.y); gr.addColorStop(0, 'rgba(63,224,197,.9)'); gr.addColorStop(1, 'rgba(63,224,197,0)');
        cx.strokeStyle = gr; cx.lineWidth = 3; cx.beginPath(); cx.moveTo(L.cen.x, L.cen.y); cx.lineTo(cima.x, cima.y); cx.stroke();
        cx.drawImage(brillo('#3fe0c5'), L.cen.x - 16, L.cen.y - 16, 32, 32);
        st.dianas.push({ x: L.cen.x, y: L.cen.y, r: 16, tipo: 'yo' });
        L.prods.forEach((q) => { cx.drawImage(brillo(q.p.color), q.x - 7, q.y - 7, 14, 14); st.dianas.push({ x: q.x, y: q.y, r: 12, tipo: 'prod', ref: q.p }); });
      }
      particulas(dt);
      cx.globalCompositeOperation = 'source-over';
    };
    const rotulo = (x, y, txt, col, puestos) => {
      // A city name you can read at a glance: sans, semibold, on a near-opaque pill.
      cx.font = '600 12px "IBM Plex Sans", "Segoe UI", system-ui, sans-serif';
      const w = cx.measureText(txt).width + 16, h = 21;
      let bx = Math.min(st.W - w - 6, Math.max(6, x)), by = y - h + 4;
      if (puestos) { let n = 0; while (n++ < 12 && puestos.some((b) => bx < b.x + b.w && bx + w > b.x && by < b.y + b.h && by + h > b.y)) by += h + 2; puestos.push({ x: bx, y: by, w, h }); }
      cx.fillStyle = 'rgba(3,8,18,.88)'; cx.beginPath(); if (cx.roundRect) cx.roundRect(bx, by, w, h, 5); else cx.rect(bx, by, w, h); cx.fill();
      cx.strokeStyle = 'rgba(255,255,255,.14)'; cx.lineWidth = 1; cx.stroke();
      cx.fillStyle = col; cx.fillRect(bx + 4, by + 5, 2, h - 10);
      cx.fillStyle = col; cx.textBaseline = 'middle'; cx.fillText(txt, bx + 10, by + h / 2 + 0.5); cx.textBaseline = 'alphabetic';
    };
    // Up close, name the measured place. Keep the host as a fallback: the map never invents a
    // city or state when the operator did not publish one.
    const rotuloLugar = (d) => d.lugar && d.lugar.ciudad
      ? [d.lugar.ciudad, d.lugar.region].filter(Boolean).join(' · ')
      : d.host;
    const rotuloYo = () => [nombrePais(st.yo), st.lugar && st.lugar.fuente !== 'zona' && st.lugar.ciudad].filter(Boolean).join(' · ');
    // How many screen pixels one degree of the Earth takes now, at the centre of the view.
    const pxPorGrado = () => (!T.ok ? escalaPlano().s : T.m < 0.5 ? st.H / (2 * Math.max(0.05, T.dist - 1) * T.tanH) * RAD : st.H / (2 * T.dist * T.tanH) * RAD);
    // Qué país hay bajo el ratón en el globo o en el mapa (rayo desde la cámara).
    const lonLatBajo = (x, y) => {
      if (!T.M || (T.m > 0.2 && T.m < 0.8)) return null;
      const nx = (2 * x / st.Wc - 1) * T.tanH * T.asp, ny = (1 - 2 * y / st.H) * T.tanH, l = Math.hypot(nx, ny, 1);
      const D = [nx / l, ny / l, -1 / l], C = T.C;
      if (T.m < 0.5) {
        const b = C[0] * D[0] + C[1] * D[1] + C[2] * D[2], cc = C[0] ** 2 + C[1] ** 2 + C[2] ** 2 - 1, disc = b * b - cc;
        if (disc < 0) return null;
        const t = -b - Math.sqrt(disc), w = [C[0] + D[0] * t, C[1] + D[1] * t, C[2] + D[2] * t], M = T.M;
        const q = [M[0] * w[0] + M[1] * w[1] + M[2] * w[2], M[4] * w[0] + M[5] * w[1] + M[6] * w[2], M[8] * w[0] + M[9] * w[1] + M[10] * w[2]];
        return [Math.atan2(q[0], q[2]) / RAD, Math.asin(Math.max(-1, Math.min(1, q[1]))) / RAD];
      }
      const t = -C[2] / D[2], px = C[0] + D[0] * t, py = C[1] + D[1] * t;
      if (Math.abs(px) > Math.PI || Math.abs(py) > Math.PI / 2) return null;
      return [px / RAD, py / RAD];
    };
    // --- Destinos without WebGL: the same world, flat, drawn with the 2D canvas ---
    // The land is the Natural Earth grid above, one dot per 1.5° cell, made once per size.
    // z, px, py: the zoom and the pan of the flat map (13 Oct 2026), in screen pixels.
    const PLANO = { lat0: 83, lat1: -58, cache: null, clave: '', z: 1, px: 0, py: 0 };
    const basePlano = (s) => ({ ox: (st.W - 360 * s) / 2, oy: Math.max(8, (st.H - (PLANO.lat0 - PLANO.lat1) * s) / 2 - 12) });
    const escalaPlano = () => {
      const s = Math.max(0.5, Math.min(st.W / 360, (st.H - 70) / (PLANO.lat0 - PLANO.lat1))) * PLANO.z, b = basePlano(s);
      return { s, ox: b.ox + PLANO.px, oy: b.oy + PLANO.py };
    };
    const aPlano = (lon, lat) => { const e = escalaPlano(); return { x: e.ox + (lon + 180) * e.s, y: e.oy + (PLANO.lat0 - lat) * e.s, v: true }; };
    const tierraPlana = () => {
      const clave = [st.W, st.H, st.dpr, PLANO.z, Math.round(PLANO.px), Math.round(PLANO.py)].join('x');
      if (PLANO.cache && PLANO.clave === clave) return PLANO.cache;
      const c = document.createElement('canvas'); c.width = Math.round(st.W * st.dpr); c.height = Math.round(st.H * st.dpr);
      const g = c.getContext('2d'); g.setTransform(st.dpr, 0, 0, st.dpr, 0, 0);
      const e = escalaPlano(), tam = Math.max(1, e.s * G.paso * 0.62);
      g.fillStyle = 'rgba(120,160,200,.34)';
      for (let i = 0; i < G.nlat; i++) {
        const lat = G.lat0 - i * G.paso; if (lat > PLANO.lat0 || lat < PLANO.lat1) continue;
        const y = aPlano(0, lat).y; if (y < -tam || y > st.H + tam) continue;
        for (let j = 0; j < G.nlon; j++) if (celdas[i * G.nlon + j]) { const q = aPlano(G.lon0 + j * G.paso, lat); if (q.x > -tam && q.x < st.W + tam) g.fillRect(q.x - tam / 2, q.y - tam / 2, tam, tam); }
      }
      PLANO.cache = c; PLANO.clave = clave; return c;
    };
    const layoutPlano = () => {
      const yo = st.lugar, cen = aPlano(yo.lon, yo.lat), R0 = 18;
      const prods = posProductos(cen, 8);
      GRUPOS = agrupa();
      GRUPOS.forEach((g) => {
        const d = g.top;
        const b = aPlano(g.lon, g.lat);
        const dx = b.x - cen.x, dy = b.y - cen.y, dist = Math.hypot(dx, dy) || 1;
        const gate = { x: cen.x + dx / dist * R0, y: cen.y + dy / dist * R0 };
        const c1 = { x: (gate.x + b.x) / 2, y: (gate.y + b.y) / 2 - Math.min(28, dist * 0.08) };
        const pr = posDe(prods, d, cen);
        g.ruta = tramo((u) => ({ x: pr.x + (gate.x - pr.x) * u, y: pr.y + (gate.y - pr.y) * u }), 5).concat(tramo((u) => bez2(gate, c1, b, u), N - 5).slice(1));
        g.tg = 5 / N; g.x = b.x; g.y = b.y; g.vis = true; g.base = b;
        aplicaRuta(g);
      });
      return { cen, R0, prods };
    };
    const dibujaPlano = (dt) => {
      cx.drawImage(tierraPlana(), 0, 0, st.W, st.H);
      const L = layoutPlano();
      tentaculos();
      pelotitas(tiempo());
      escudo(L.cen, L.R0);
      st.dianas.push({ x: L.cen.x, y: L.cen.y, r: 10, tipo: 'yo' });
      L.prods.forEach(nodoProducto);
      particulas(dt);
      GRUPOS.forEach((g) => {
        const d = g.top;
        if (!g.vis) return;
        cx.drawImage(brillo(colorDe(d)), g.x - 5.2, g.y - 5.2, 10.4, 10.4);
        st.dianas.push({ x: g.x, y: g.y, r: 16, tipo: 'lugar', ref: g });
      });
    };


    // ---------- Drawing ----------
    const enTierra = () => st.vista === 'destinos' && T.ok;
    const fondo = () => {
      if (enTierra()) { cx.clearRect(0, 0, st.Wc, st.H); return; }
      cx.fillStyle = '#04070d'; cx.fillRect(0, 0, st.Wc, st.H);
      const tt = tiempo();
      estrellas.forEach((e) => { const a = 0.25 + 0.25 * Math.sin(tt * 0.8 + e.f); cx.fillStyle = `rgba(190,215,240,${reducido ? 0.35 : a})`; cx.fillRect(e.x * st.W, e.y * st.H, e.r, e.r); });
    };
    const trazo = (ruta, desde, hasta, color, ancho, guion) => {
      if (!ruta.length) return;
      const n = ruta.length - 1, i0 = Math.round(desde * n), i1 = Math.round(hasta * n);
      cx.lineCap = 'round'; cx.lineJoin = 'round';
      cx.strokeStyle = color; cx.lineWidth = ancho; cx.setLineDash(guion || []);
      cx.beginPath(); let abierto = false;
      for (let i = i0; i <= i1; i++) { const q = ruta[i]; if (q.v === false) { abierto = false; continue; } if (!abierto) { cx.moveTo(q.x, q.y); abierto = true; } else cx.lineTo(q.x, q.y); }
      cx.stroke(); cx.setLineDash([]);
    };
    const escudo = (c, r) => {
      const tt = tiempo(), cerrado = PROD.some((p) => p.cortado) || DEST.some((d) => d.cortado);
      const g = cx.createRadialGradient(c.x, c.y, r * 0.2, c.x, c.y, r * 1.6);
      g.addColorStop(0, 'rgba(63,224,197,.10)'); g.addColorStop(0.62, 'rgba(63,224,197,.05)'); g.addColorStop(1, 'rgba(63,224,197,0)');
      cx.fillStyle = g; cx.beginPath(); cx.arc(c.x, c.y, r * 1.6, 0, 7); cx.fill();
      cx.strokeStyle = 'rgba(63,224,197,.85)'; cx.lineWidth = 1.6;
      cx.setLineDash([r * 0.18, r * 0.06]); cx.lineDashOffset = -tt * 12;
      cx.beginPath(); cx.arc(c.x, c.y, r, 0, 7); cx.stroke(); cx.setLineDash([]); cx.lineDashOffset = 0;
      cx.strokeStyle = `rgba(63,224,197,${0.18 + 0.12 * Math.sin(tt * 2)})`; cx.lineWidth = 6; cx.beginPath(); cx.arc(c.x, c.y, r + 5, 0, 7); cx.stroke();
      if (cerrado) { cx.strokeStyle = 'rgba(255,107,122,.25)'; cx.lineWidth = 1; cx.beginPath(); cx.arc(c.x, c.y, r + 11, 0, 7); cx.stroke(); }
    };
    const ordenador = (c, s) => {
      const g = cx.createRadialGradient(c.x, c.y, 0, c.x, c.y, s * 2.4);
      g.addColorStop(0, 'rgba(255,120,140,.55)'); g.addColorStop(0.35, 'rgba(255,90,120,.15)'); g.addColorStop(1, 'rgba(255,90,120,0)');
      cx.fillStyle = g; cx.beginPath(); cx.arc(c.x, c.y, s * 2.4, 0, 7); cx.fill();
      cx.strokeStyle = '#ffd9df'; cx.lineWidth = 1.4; cx.strokeRect(c.x - s * 0.7, c.y - s * 0.5, s * 1.4, s * 0.9);
      cx.beginPath(); cx.moveTo(c.x - s * 0.3, c.y + s * 0.62); cx.lineTo(c.x + s * 0.3, c.y + s * 0.62); cx.moveTo(c.x, c.y + s * 0.4); cx.lineTo(c.x, c.y + s * 0.62); cx.stroke();
      cx.fillStyle = '#ff6f86'; cx.beginPath(); cx.arc(c.x, c.y - s * 0.05, s * 0.16, 0, 7); cx.fill();
    };
    const nodoProducto = (q) => {
      const p = q.p;
      cx.fillStyle = p.color; cx.beginPath(); cx.arc(q.x, q.y, 5.5, 0, 7); cx.fill();
      cx.strokeStyle = p.cortado ? '#ff6b7a' : 'rgba(255,255,255,.6)'; cx.lineWidth = p.cortado ? 2 : 1; cx.beginPath(); cx.arc(q.x, q.y, 9, 0, 7); cx.stroke();
      st.dianas.push({ x: q.x, y: q.y, r: 12, tipo: 'prod', ref: p });
    };
    // The colour of a destination: cut, documented by the maker, not documented, or not an
    // antivirus (the question does not apply).
    const colorDe = (d) => (bloqueado(d) ? '#ff6b7a' : d.doc === false ? '#ffd166' : '#ffb547');
    const etiqueta = (d, x, y, alinea) => {
      const txt = d.host.length > 30 ? d.host.slice(0, 29) + '…' : d.host;
      cx.font = '500 10.5px "IBM Plex Mono", monospace';
      const w = cx.measureText(txt).width + 12, h = 17;
      const bx = alinea === 'izq' ? x - w - 10 : x + 10, by = y - h / 2;
      const bl = bloqueado(d);
      cx.fillStyle = 'rgba(6,12,22,.82)'; cx.fillRect(bx, by, w, h);
      cx.strokeStyle = bl ? '#ff6b7a' : d.doc === false ? '#ffd166' : 'rgba(255,181,71,.7)'; cx.lineWidth = 1;
      cx.setLineDash(d.doc === false && !bl ? [3, 2] : []); cx.strokeRect(bx + 0.5, by + 0.5, w - 1, h - 1); cx.setLineDash([]);
      cx.fillStyle = bl ? '#ff8d99' : '#f3e3c8'; cx.textBaseline = 'middle'; cx.fillText(txt, bx + 6, y + 0.5); cx.textBaseline = 'alphabetic';
    };
    const nodoDestino = (d, conEtiqueta, alinea) => {
      if (!d.vis) return;
      const tam = 3 + Math.min(6, Math.log10(1 + d.sub) * 0.9);
      cx.drawImage(brillo(colorDe(d)), d.x - tam * 1.6, d.y - tam * 1.6, tam * 3.2, tam * 3.2);
      cx.fillStyle = colorDe(d); cx.fillRect(d.x - tam / 2, d.y - tam / 2, tam, tam);
      if (conEtiqueta) etiqueta(d, d.x, d.y, alinea);
      st.dianas.push({ x: d.x, y: d.y, r: 14, tipo: 'dest', ref: d });
    };
    const puerta = (q, roto) => {
      cx.save(); cx.translate(q.x, q.y); cx.rotate(roto || 0);
      cx.fillStyle = '#ff6b7a'; cx.fillRect(-2, -9, 4, 18);
      cx.restore();
    };
    const tentaculos = () => {
      GRUPOS.forEach((g) => {
        const d = g.top;
        if (!g.ruta || !g.ruta.length) return;
        const bl = bloqueado(d), base = prod(d.p).color;
        if (bl) {
          trazo(g.ruta, 0, g.tg, 'rgba(255,107,122,.75)', 0.7);
          trazo(g.ruta, g.tg, 1, 'rgba(160,175,200,.22)', 0.45, [1.2, 3.2]);
          const q = punto(g.ruta, g.tg), h = punto(g.ruta, g.tg + 0.01);
          if (q.v) puerta(q, Math.atan2(h.y - q.y, h.x - q.x));
        } else {
          trazo(g.ruta, 0, 1, base + '22', 1.05);
          trazo(g.ruta, 0, 1, base + 'c0', 0.45);
        }
      });
    };
    // A small ball travels each line and comes back. One per place, out of step with the others.
    const pelotitas = (tt) => {
      if (reducido) return;
      const prev = cx.globalCompositeOperation;
      cx.globalCompositeOperation = 'lighter';
      GRUPOS.forEach((g) => {
        if (!g.ruta || g.ruta.length < 2) return;
        // A cut place has no ball going to it (the owner, 16 Oct 2026: «cuando corto un nombre la
        // línea deja de aparecer pero sigue la pelotita yendo y viniendo»).
        if (bloqueado(g.top)) return;
        const d = g.top, col = prod(d.p).color;
        const fase = ((g.lon * 13 + g.lat * 7) % 1 + 1) % 1;
        const u = Math.abs((tt / 2.6 + fase) % 2 - 1);
        const p = punto(g.ruta, u);
        if (!p.v) return;
        const s = 3.4;
        cx.drawImage(brillo(col), p.x - s, p.y - s, s * 2, s * 2);
      });
      cx.globalCompositeOperation = prev;
    };
    const particulas = (dt) => {
      st.part = st.part.filter((q) => {
        q.t += (q.dir === 'out' ? 1 : -1) * q.v * dt;
        const d = q.d;
        if (!d.ruta.length || !d.vis) return false;
        if (q.bloq && q.t >= d.tg) { const g = punto(d.ruta, d.tg); st.estallidos.push({ x: g.x, y: g.y, a: 0, v: g.v }); return false; }
        if (q.t > 1 || q.t < 0) return false;
        const p = punto(d.ruta, q.t);
        if (!p.v) return true;
        const col = q.bloq ? '#ff6b7a' : q.dir === 'out' ? '#ffb547' : '#6fb6ff';
        // A trail: a few steps back along the route, fainter each time.
        const atras = q.dir === 'out' ? -1 : 1, prev = cx.globalCompositeOperation;
        cx.globalCompositeOperation = 'lighter';
        for (let k = 7; k >= 1; k--) { const pk = punto(d.ruta, q.t + atras * k * 0.009); if (!pk.v) continue; const s = q.r * (1.9 - k * 0.18); cx.globalAlpha = 0.22 * (1 - k / 8); cx.drawImage(brillo(col), pk.x - s, pk.y - s, 2 * s, 2 * s); }
        cx.globalAlpha = 0.8; const s = q.r * 2.3; cx.drawImage(brillo(col), p.x - s, p.y - s, 2 * s, 2 * s); cx.globalAlpha = 1;
        cx.globalCompositeOperation = prev;
        return true;
      });
      st.estallidos = st.estallidos.filter((e) => {
        e.a += dt * 1.6; if (e.a > 1) return false; if (!e.v) return true;
        cx.strokeStyle = `rgba(255,107,122,${0.6 * (1 - e.a)})`; cx.lineWidth = 1.2; cx.beginPath(); cx.arc(e.x, e.y, 3 + e.a * 9, 0, 7); cx.stroke();
        return true;
      });
    };
    // Los nombres no se pintan en el mapa: la ficha sale al pasar el cursor por el punto.

    // ---------- Programas: the activity explorer (13 Oct 2026) ----------
    // The owner found the Sankey hard to read; Grok and ChatGPT, asked as a design agency, both
    // said the same: the question is «which program sends, where to, and what is cut», and that is
    // read best as a list. One row per program (antivirus first, in a section of their own), its
    // two main destinations, what it sent and received, what was stopped and its last five minutes;
    // a click unfolds every destination, and a click on one opens its card. HTML, not canvas: it
    // is text the person reads, selects and reaches with the keyboard.
    const expl = $('explorador'), filasExpl = $('expl-filas');
    let ordenExpl = 'enviado', buscaExpl = '', abiertoExpl = null, verTodosExpl = false;
    const coincide = (txt) => !buscaExpl || String(txt || '').toLowerCase().includes(buscaExpl);
    const filaDestExpl = (d) => {
      const bl = bloqueado(d), nuevo = d.visto && Date.now() - d.visto < 180000;
      const chips = (bl ? `<span class="chip c">${esc(d.cortado ? t('pd_chip_cortado') : t('pd_chip_parado'))}</span>` : '') + (d.doc === false ? `<span class="chip nd">${esc(t('pd_chip_nodoc'))}</span>` : '') + (nuevo ? `<span class="chip">${esc(t('pd_chip_nuevo'))}</span>` : '');
      return `<div class="ex-dest enlace-fila" data-dest="${esc(d.k)}" tabindex="0" role="button">
        <span class="ex-host"><span class="mono">${esc(d.host)}</span>${d.red && d.red[0] !== '@' ? `<span class="t">${esc(t('pd_ex_red').replace('{red}', d.red))}${d.asn ? ' · AS' + d.asn : ''}</span>` : ''}</span>
        <span>${esc(d.empresa || '—')}</span>
        <span>${esc(d.pais && d.pais[0] !== '@' ? nombrePais(d.pais) : '—')}</span>
        <span class="num o">${d.sub ? '↑ ' + esc(bytes(d.sub)) : '—'}</span>
        <span class="num i">${d.baj ? '↓ ' + esc(bytes(d.baj)) : '—'}</span>
        <span class="ex-chips">${d.bloq ? `<span class="c">${esc(tn('pd_parados', d.bloq))}</span> ` : ''}${chips}</span>
      </div>`;
    };
    const filaProgExpl = (p, mx) => {
      const ds = DEST.filter((d) => d.p === p.id).sort((a, b) => b.sub - a.sub || b.bloq - a.bloq);
      const vis = buscaExpl && !coincide(p.nombre) && !p.procesos.some(coincide) ? ds.filter((d) => coincide(d.host) || coincide(d.empresa) || coincide(d.red)) : ds;
      const parados = ds.reduce((a, d) => a + (d.bloq || 0), 0), nodoc = ds.filter((d) => d.doc === false).length;
      const activo = ds.some((d) => d.ult && performance.now() - d.ult < 15000 && !bloqueado(d));
      const abierto = abiertoExpl === p.id;
      const top = ds.filter((d) => d.sub > 0).slice(0, 2);
      const resto = ds.length - top.length;
      const tipo = esc(tipoTexto(p)) + (p.act && p.act.cpu !== null && p.act.cpu !== undefined ? ' · ' + esc(t('pd_av_cpu')) + ' ' + esc(pct1(p.act.cpu)) + ' · ' + esc(bytes(p.act.memoria)) : '') + (p.firma ? ' · ' + esc(t('pd_firmado').replace('{firma}', p.firma)) : '');
      let h = `<div class="ex-prog${abierto ? ' abierto' : ''}${p.cortado ? ' cortado' : ''}" data-c="${p.color}">
        <button type="button" class="ex-fila" data-expl="${esc(p.id)}" aria-expanded="${abierto}">
          <span class="ex-nombre"><span class="pto${activo ? ' vivo' : ''}" aria-hidden="true"></span><span><b>${esc(p.nombre)}</b><span class="t">${tipo}</span></span></span>
          <span class="ex-destinos">${top.map((d) => `<span class="mono">${esc(d.host)}</span>`).join('')}${resto > 0 ? `<span class="t">${esc(tn('pd_y_mas', resto))}</span>` : ''}${!ds.length ? `<span class="t">${esc(t('pd_ex_sin_destinos'))}</span>` : ''}</span>
          <span class="ex-env"><span class="num o">${esc(bytes(p.sub))}</span><span class="barra-env"><i data-w="${Math.max(p.sub ? 2 : 0, p.sub / mx * 100)}"></i></span></span>
          <span class="num i">${esc(bytes(p.baj))}</span>
          <span class="ex-cortes">${p.cortado ? `<span class="chip c">${esc(t('pd_chip_cortado'))}</span>` : parados ? `<span class="c">${esc(tn('pd_parados', parados))}</span>` : '<span class="t">—</span>'}${nodoc ? `<span class="nd">${esc(tn('pd_ex_nodoc', nodoc))}</span>` : ''}</span>
          <span class="ex-5m"><canvas data-serie="${esc(p.id)}"></canvas></span>
          <span class="ex-flecha" aria-hidden="true">›</span>
        </button>`;
      if (abierto) {
        const lista = verTodosExpl ? vis : vis.slice(0, 6);
        h += `<div class="ex-tabla"><div class="ex-cab-dest t"><span>${esc(t('pd_ex_col_nombre'))}</span><span>${esc(t('pd_ex_col_empresa'))}</span><span>${esc(t('pd_ex_col_pais'))}</span><span>${esc(t('pd_ex_col_env'))}</span><span>${esc(t('pd_ex_col_rec'))}</span><span>${esc(t('pd_ex_col_estado'))}</span></div>
          ${lista.map(filaDestExpl).join('') || `<p class="t">${esc(t('pd_ex_sin_destinos'))}</p>`}
          ${vis.length > lista.length ? `<button type="button" class="btn-hud ex-todos" data-todos="1">${esc(t('pd_ex_ver_todos').replace('{n}', vis.length))}</button>` : ''}
          <div class="ex-acciones">${!soloMirar && p.cortable ? `<button type="button" class="btn-hud${p.cortado ? '' : ' peligro'}" data-cortar-prog="${esc(p.id)}">${esc(t(p.cortado ? 'pd_btn_dejar_prog' : 'pd_btn_cortar_prog'))}</button>` : ''}${p.tipo === 'antivirus' && !soloMirar ? `<a href="/antivirus">${esc(t('pd_ver_antivirus'))} →</a>` : ''}</div>
        </div>`;
      }
      return h + '</div>';
    };
    const ordenaExpl = (ps) => ps.sort(ordenExpl === 'recibido' ? (a, b) => b.baj - a.baj : ordenExpl === 'ahora' ? (a, b) => (st.serieP[b.id] || []).reduce((x, y) => x + y, 0) - (st.serieP[a.id] || []).reduce((x, y) => x + y, 0) : ordenExpl === 'nombre' ? (a, b) => a.nombre.localeCompare(b.nombre, LOC) : (a, b) => b.sub - a.sub);
    const pintaExplorador = () => {
      if (st.vista !== 'programas') return;
      const ps = PROD.filter(enFiltro).filter((p) => !buscaExpl || coincide(p.nombre) || p.procesos.some(coincide) || DEST.some((d) => d.p === p.id && (coincide(d.host) || coincide(d.empresa) || coincide(d.red))));
      const mx = Math.max(1, ...ps.map((p) => p.sub));
      const av = ordenaExpl(ps.filter(esAV)), otros = ordenaExpl(ps.filter((p) => !esAV(p)));
      const seccion = (titulo, lista) => (lista.length ? `<div class="ex-seccion">${esc(titulo)} <span>${lista.length}</span></div>` + lista.map((p) => filaProgExpl(p, mx)).join('') : '');
      let h = seccion(t('pd_ex_sec_av'), av) + seccion(t('pd_ex_sec_otros'), otros);
      if (!h) h = `<p class="ex-vacio t">${esc(buscaExpl ? t('pd_ex_nada_busca') : t('pd_ex_nada'))}</p>`;
      if (filasExpl.innerHTML !== h) { filasExpl.innerHTML = h; estilos(filasExpl); }
      // The heading: who sent most, and when the last measurement came.
      const mayor = PROD.filter(enFiltro).sort((a, b) => b.sub - a.sub)[0];
      $('expl-mayor').textContent = mayor && mayor.sub ? t('pd_ex_mayor').replace('{prog}', mayor.nombre).replace('{x}', bytes(mayor.sub)) : '';
      $('expl-ultima').textContent = ultimaCarga ? t('pd_ex_ultima').replace('{h}', ctx.reloj ? ctx.reloj() : hora(Date.now() - (performance.now() - ultimaCarga))) : '';
      // Five minutes per program: 30 bars, one per 10 seconds, all on the same scale.
      const todas = PROD.map((p) => Math.max(0, ...(st.serieP[p.id] || [0]))), techo = Math.max(1, ...todas);
      filasExpl.querySelectorAll('canvas[data-serie]').forEach((c) => {
        const p = PROD.find((x) => x.id === c.dataset.serie); if (!p) return;
        const { g, w, h: hh } = prepara(c); if (w < 20) return;
        const serie = st.serieP[p.id] || [], n = serie.length || 1, bw = w / n;
        g.fillStyle = 'rgba(118,144,168,.16)'; g.fillRect(0, hh - 1, w, 1);
        g.fillStyle = p.cortado ? '#ff6b7a' : '#3fe0c5';
        serie.forEach((v, i) => { if (v > 0) { const a = Math.max(2, Math.sqrt(v / techo) * (hh - 2)); g.fillRect(i * bw + 0.5, hh - 1 - a, Math.max(1, bw - 1.5), a); } });
      });
    };
    $('expl-busca').placeholder = t('pd_ex_buscar');
    $('expl-orden').addEventListener('change', (e) => { ordenExpl = e.target.value; pintaExplorador(); });
    $('expl-busca').addEventListener('input', (e) => { buscaExpl = e.target.value.trim().toLowerCase(); verTodosExpl = !!buscaExpl; pintaExplorador(); });
    filasExpl.addEventListener('click', (e) => {
      const f = e.target.closest('[data-expl]');
      if (f) { abiertoExpl = abiertoExpl === f.dataset.expl ? null : f.dataset.expl; verTodosExpl = !!buscaExpl; pintaExplorador(); return; }
      if (e.target.closest('[data-todos]')) { verTodosExpl = true; pintaExplorador(); return; }
      const c = e.target.closest('[data-cortar-prog]');
      if (c) { const p = PROD.find((x) => x.id === c.dataset.cortarProg); if (p) corteProducto(p); }
    });

    // The page's security policy allows no inline styles (style-src 'self'): widths and colours
    // travel as data-* and are set through the style object, which the policy allows.
    const estilos = (raiz) => {
      raiz.querySelectorAll('[data-w]').forEach((e) => { e.style.width = e.dataset.w + '%'; });
      raiz.querySelectorAll('[data-bg]').forEach((e) => { e.style.background = e.dataset.bg; });
      raiz.querySelectorAll('[data-c]').forEach((e) => { e.style.setProperty('--c', e.dataset.c); });
    };
    // ---------- The bottom deck ----------
    const prepara = (c) => { const r = c.getBoundingClientRect(), d = Math.min(window.devicePixelRatio || 1, 2); c.width = Math.max(1, r.width * d); c.height = Math.max(1, r.height * d); const g = c.getContext('2d'); g.setTransform(d, 0, 0, d, 0, 0); return { g, w: r.width, h: r.height }; };
    const sparkline = () => {
      const { g, w, h } = prepara($('spark'));
      if (w < 20 || h < 20) return;
      const ps = prodsVisibles(), suma = new Array(N_CUBOS).fill(0);
      ps.forEach((p) => (st.serieP[p.id] || []).forEach((v, i) => { suma[i] += v; }));
      const mx = Math.max(1, ...suma);
      g.strokeStyle = 'rgba(118,144,168,.18)'; g.lineWidth = 1;
      for (let k = 1; k < 4; k++) { g.beginPath(); g.moveTo(0, h * k / 4); g.lineTo(w, h * k / 4); g.stroke(); }
      const n = st.serie.length, X = (i) => i / (n - 1) * w, Y = (v) => h - 3 - v / mx * (h - 9);
      let base = new Array(n).fill(0);
      ps.forEach((p) => {
        const serie = st.serieP[p.id] || [], tope = base.map((b, i) => b + (serie[i] || 0));
        g.beginPath(); tope.forEach((v, i) => (i ? g.lineTo(X(i), Y(v)) : g.moveTo(X(i), Y(v))));
        for (let i = n - 1; i >= 0; i--) g.lineTo(X(i), Y(base[i]));
        g.closePath(); g.fillStyle = p.color + '55'; g.fill();
        g.beginPath(); tope.forEach((v, i) => (i ? g.lineTo(X(i), Y(v)) : g.moveTo(X(i), Y(v)))); g.strokeStyle = p.color; g.lineWidth = 1.1; g.stroke();
        base = tope;
      });
    };
    // The band, closed (a glance) or open (everything). Remembered in this browser.
    const hudAmplio = () => $('hud').classList.contains('amplio');
    const ponHud = (abierto) => {
      $('hud').classList.toggle('amplio', abierto);
      $('hud-asa').setAttribute('aria-expanded', String(abierto));
      $('hud-asa').querySelector('span').textContent = t(abierto ? 'pd_hud_recoger' : 'pd_hud_desplegar');
      $('hud-asa').title = t(abierto ? 'pd_hud_recoger' : 'pd_hud_desplegar');
      try { localStorage.setItem('guardiana_pd_hud', abierto ? '1' : ''); } catch (_) {}
      requestAnimationFrame(() => { try { sparkline(); } catch (_) {} ajusta(); });
    };
    $('hud-asa').addEventListener('click', () => ponHud(!hudAmplio()));
    try { if (localStorage.getItem('guardiana_pd_hud') === '1') ponHud(true); else ponHud(false); } catch (_) { ponHud(false); }
    const pintaHud = () => {
      // The deck follows «Todo · Datos · Antivirus», like the drawing.
      const mios = PROD.filter(enFiltro), ids = new Set(mios.map((p) => p.id)), DF = DEST.filter((d) => ids.has(d.p));
      const amplio = hudAmplio();
      $('registro').innerHTML = st.eventos.filter((e) => st.filtro === 'todo' || (st.filtro === 'av') === !!e.av).slice(0, amplio ? 40 : (window.innerWidth >= 1700 ? 14 : 9)).map((e) => `<li><span class="t">${esc(e.h)}</span> ${esc(e.prog)} → ${esc(e.host)} <span class="${e.cls}">${esc(e.que)}</span></li>`).join('') || `<li class="t">${esc(t('pd_registro_vacio'))}</li>`;
      $('hud-min').textContent = bytes(mios.reduce((a, p) => a + (st.serieP[p.id] || []).reduce((x, y) => x + y, 0), 0));
      const pp = {}; DF.forEach((d) => { if (d.pais && d.pais[0] !== '@' && !d.sede) pp[d.pais] = (pp[d.pais] || 0) + d.sub; });
      const todos = Object.entries(pp).filter((x) => x[1] > 0).sort((a, b) => b[1] - a[1]), top = amplio ? todos : todos.slice(0, window.innerWidth >= 1700 ? 8 : 5), mx = Math.max(1, ...top.map((x) => x[1])), totalP = Math.max(1, todos.reduce((a, x) => a + x[1], 0));
      $('barras').innerHTML = top.map(([c, v]) => `<div class="enlace-fila" data-pais="${esc(c)}" tabindex="0" role="button"><span>${esc(nombrePais(c))}</span><s data-w="${Math.max(2, v / mx * 100)}"></s><span>${esc(bytes(v))}</span>${amplio ? `<span class="t">${Math.round(v / totalP * 100)} %</span>` : ''}</div>`).join('') || `<div class="t">—</div>`;
      estilos($('barras'));
      $('hud-cortes').textContent = num(DF.filter((d) => d.cortado).length + mios.filter((p) => p.cortado).length);
      $('hud-bloqueos').textContent = tn('pd_intentos', DF.reduce((a, d) => a + d.bloq, 0));
      sparkline();
      // «Lo que más envía ahora» (13 Oct 2026, in place of a needle that hardly ever moved): the
      // three destinations with most sent in the last minutes, each one a click away from its card.
      const vivos = DF.filter((d) => d.ema > 0 && !bloqueado(d)).sort((a, b) => b.ema - a.ema).slice(0, amplio ? 30 : (window.innerWidth >= 1700 ? 6 : 3));
      $('hud-top').innerHTML = vivos.map((d) => `<li class="enlace-fila" data-dest="${esc(d.k)}" tabindex="0" role="button"><span>${esc(d.host)}</span><span class="o">${esc(bytes(d.ema))}/s</span></li>`).join('') || `<li class="t">${esc(t('pd_top_nada'))}</li>`;
      pintaTabla(DF, amplio);
      pintaCorte(); pintaDetalle(); pintaExplorador();
    };
    // The technical table and its downloads: exactly the rows on screen, nothing more. The file is
    // made here and never leaves the computer unless the person sends it.
    let ordenTabla = { k: 'sub', desc: true }, buscaTabla = '';
    const valorOrden = (d, k) => k === 'programa' ? prod(d.p).nombre : k === 'destino' ? d.host : k === 'pais' ? (d.pais ? nombrePais(d.pais) : '') : k === 'ciudad' ? (d.lugar && d.lugar.ciudad) || '' : k === 'proto' ? (d.proto || '') + (d.puerto || '') : k === 'estado' ? (d.cortado ? 2 : d.bloq > 0 ? 1 : 0) : d[k];
    const filasTabla = (DF) => {
      const q = buscaTabla.trim().toLocaleLowerCase(LOC);
      let f = DF.filter((d) => !d.sede);
      if (q) f = f.filter((d) => [prod(d.p).nombre, d.host, d.destino, d.empresa, d.pais && nombrePais(d.pais), d.lugar && d.lugar.ciudad, d.red, d.asn && 'AS' + d.asn, d.proto, d.puerto].filter(Boolean).join(' ').toLocaleLowerCase(LOC).includes(q));
      const { k, desc } = ordenTabla;
      return f.sort((a, b) => { const x = valorOrden(a, k), y = valorOrden(b, k); const c = typeof x === 'number' && typeof y === 'number' ? x - y : String(x ?? '').localeCompare(String(y ?? ''), LOC); return (desc ? -c : c) || (b.sub + b.baj) - (a.sub + a.baj); });
    };
    $('ht-busca').addEventListener('input', () => { buscaTabla = $('ht-busca').value; pintaHud(); });
    document.querySelectorAll('table.ht th[data-orden]').forEach((th) => {
      const elige = () => { const k = th.dataset.orden; ordenTabla = ordenTabla.k === k ? { k, desc: !ordenTabla.desc } : { k, desc: !['programa', 'destino', 'empresa', 'pais', 'ciudad', 'red', 'proto'].includes(k) }; document.querySelectorAll('table.ht th[data-orden]').forEach((x) => x.removeAttribute('aria-sort')); th.setAttribute('aria-sort', ordenTabla.desc ? 'descending' : 'ascending'); pintaHud(); };
      th.addEventListener('click', elige); th.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); elige(); } });
    });
    const fechaCorta = (ms) => ms ? new Date(ms).toLocaleString(LOC, { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' }) : '—';
    const pintaTabla = (DF, amplio) => {
      $('hud-tabla').hidden = !amplio;
      if (!amplio) return;
      const todas = filasTabla(DF), filas = todas.slice(0, 300);
      $('ht-cuantos').textContent = num(todas.length);
      const paises = new Set(todas.map((d) => d.pais).filter((p) => p && p[0] !== '@'));
      $('ht-kpi').innerHTML = [[bytes(todas.reduce((a, d) => a + d.sub, 0)), t('pd_ht_sube'), 'o'], [bytes(todas.reduce((a, d) => a + d.baj, 0)), t('pd_ht_baja'), 'i'], [num(todas.length), t('pd_ht_titulo').split(' ·')[0], ''], [num(paises.size), t('pd_ht_paises'), ''], [num(todas.reduce((a, d) => a + d.bloq, 0)), t('pd_ht_cortes'), 'c']].map(([v, l, c]) => `<div><b class="${c}">${esc(v)}</b><span>${esc(l)}</span></div>`).join('');
      $('ht-filas').innerHTML = filas.map((d) => {
        const estado = d.cortado ? `<span class="chip-ht c">${esc(t('pd_ht_cortado'))}</span>` : d.bloq > 0 ? `<span class="chip-ht o">${esc(tn('pd_intentos', d.bloq))}</span>` : d.doc === true ? `<span class="chip-ht t">${esc(t('pd_ht_documentado'))}</span>` : d.doc === false ? `<span class="chip-ht" style="color:var(--undoc)">${esc(t('pd_ht_sin_documentar'))}</span>` : '';
        return `<tr class="enlace-fila" data-dest="${esc(d.k)}" tabindex="0" role="button"><td>${esc(prod(d.p).nombre)}</td><td title="${esc(d.destino || d.host)}">${esc(d.host)}</td><td>${esc(d.empresa || '—')}</td><td>${esc(d.pais ? nombrePais(d.pais) : '—')}</td><td>${esc(d.lugar && d.lugar.ciudad ? d.lugar.ciudad : '—')}</td><td title="AS${esc(d.asn || '')}">${esc(d.red || '—')}${d.asn ? ` <span class="t">AS${esc(d.asn)}</span>` : ''}</td><td>${esc((d.proto || '') + (d.puerto ? '/' + d.puerto : ''))}</td><td class="n o">${esc(bytes(d.sub))}</td><td class="n i">${esc(bytes(d.baj))}</td><td class="n">${esc(num(d.paq))}</td><td class="n">${esc(num(d.consultas))}</td><td class="n${d.bloq ? ' c' : ''}">${esc(num(d.bloq))}</td><td class="t">${esc(fechaCorta(d.primera))}</td><td class="t">${esc(fechaCorta(d.ultima))}</td><td>${estado}</td></tr>`;
      }).join('') || `<tr><td colspan="15" class="t">${esc(t('pd_registro_vacio'))}</td></tr>`;
    };
    const descarga = (nombre, tipo, texto) => {
      const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([texto], { type: tipo })); a.download = nombre; document.body.appendChild(a); a.click(); setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
    };
    const selloArchivo = () => { const d = new Date(); const p = (n) => String(n).padStart(2, '0'); return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}`; };
    const filasExport = () => {
      const mios = PROD.filter(enFiltro), ids = new Set(mios.map((p) => p.id));
      return filasTabla(DEST.filter((d) => ids.has(d.p))).map((d) => ({ programa: prod(d.p).nombre, destino: d.destino || d.host, nombre: d.host, empresa: d.empresa || '', pais: d.pais || '', ciudad: d.lugar && d.lugar.ciudad ? d.lugar.ciudad : '', red: d.red || '', asn: d.asn || 0, protocolo: d.proto || '', puerto: d.puerto || 0, enviados_bytes: d.sub, recibidos_bytes: d.baj, paquetes: d.paq, consultas: d.consultas, cortes: d.bloq, cortado: !!d.cortado, documentado: d.doc === true ? 'si' : d.doc === false ? 'no' : '', uso: d.uso || '', primera: d.primera ? new Date(d.primera).toISOString() : '', ultima: d.ultima ? new Date(d.ultima).toISOString() : '' }));
    };
    $('ht-json').addEventListener('click', () => descarga(`guardiana-por-dentro-${selloArchivo()}.json`, 'application/json', JSON.stringify({ programa: 'GUARDIANA', pagina: 'por-dentro', ventana_h: st.datos && st.datos.ventana_h, generado: new Date().toISOString(), filtro: st.filtro, destinos: filasExport(), sucesos: st.eventos.map((e) => ({ hora: e.h, programa: e.prog, destino: e.host, que: e.que })) }, null, 2)));
    $('ht-csv').addEventListener('click', () => {
      const filas = filasExport(); if (!filas.length) return;
      const cab = Object.keys(filas[0]); const q = (v) => { const s = String(v ?? ''); return /[",\n;]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s; };
      descarga(`guardiana-por-dentro-${selloArchivo()}.csv`, 'text/csv;charset=utf-8', '\ufeff' + cab.join(',') + '\n' + filas.map((f) => cab.map((k) => q(f[k])).join(',')).join('\n') + '\n');
    });
    // «Cortar todo lo que envía el antivirus» (until 13 Oct 2026 «Protección máxima», a name that
    // hid that the antivirus then gets no updates): a small button in the antivirus's telemetry,
    // not in the top bar; while it is on, the bar's chip says so, so nobody forgets it.
    const pintaCorte = () => {
      const todo = !!(st.datos && st.datos.maxima), hay = PROD.some((p) => p.cortable);
      const chip = $('chip-modo');
      chip.hidden = !todo;
      if (todo && chip.textContent !== t('pd_chip_corte')) { chip.textContent = t('pd_chip_corte'); chip.title = t('av_max_activa'); }
      const b = $('corte-av-btn'), texto = todo ? t('av_max_quitar') : t('av_max_activar');
      $('corte-av').hidden = soloMirar || !(todo || hay);
      if (b.textContent !== texto) b.textContent = texto;
      b.classList.toggle('quitar', todo);
    };

    // ---------- «Datos en vivo» ----------
    const panelDatos = $('datos');
    let datosAbierto = true;
    let zonaAbierta = '', ciudadAbierta = '';
    // Everything on this page is live; the side panel of figures closes with its × and comes back
    // with the small «‹ Cifras» tab on the edge (the owner, 13 Oct 2026: the «Datos en vivo» button
    // did not say what it did; «● En vivo» is now a fixed sign in the bar).
    const ponDatos = (on) => { datosAbierto = on; panelDatos.hidden = !on; $('btn-datos').hidden = on; $('btn-datos').setAttribute('aria-expanded', String(on)); try { localStorage.setItem('guardiana_pd_datos', on ? '1' : '0'); } catch (_) {} pintaDatos(); };
    $('btn-datos').addEventListener('click', () => ponDatos(true));
    $('cerrar-datos').addEventListener('click', () => ponDatos(false));
    $('datos').addEventListener('click', (e) => {
      const c = e.target.closest('[data-ciudad]');
      const z = c ? null : e.target.closest('[data-zona]');
      if (!c && !z) return;
      e.stopPropagation();
      if (c) ciudadAbierta = ciudadAbierta === c.dataset.ciudad ? '' : c.dataset.ciudad;
      else { zonaAbierta = zonaAbierta === z.dataset.zona ? '' : z.dataset.zona; ciudadAbierta = ''; }
      pintaDatos();
    });
    // Two telemetries, apart (the owner, 13 Oct 2026): the data of the browser and the other
    // programs, and the antivirus's own, so what the person does is never mixed with what the
    // antivirus does.
    const esAV = (p) => !!p && p.tipo === 'antivirus';
    const resumenGrupo = (es) => {
      const ds = DEST.filter((d) => es(prod(d.p))), ps = PROD.filter(es), ahoraMs = performance.now();
      const conDatos = ds.filter((d) => d.sub && !d.sede);
      return {
        ds, ps,
        activas: ds.filter((d) => d.ult && ahoraMs - d.ult < 15000 && !bloqueado(d)).length,
        tot: conDatos.reduce((a, d) => a + d.sub, 0),
        fuera: conDatos.filter((d) => d.pais && d.pais !== st.yo && d.pais[0] !== '@').reduce((a, d) => a + d.sub, 0),
        paises: new Set(conDatos.map((d) => d.pais).filter((x) => x && x[0] !== '@')).size,
        empresas: new Set(conDatos.map((d) => d.red).filter((r) => r && r[0] !== '@')).size,
        cortes: ds.filter((d) => d.cortado).length + ps.filter((p) => p.cortado).length,
        intentos: ds.reduce((a, d) => a + d.bloq, 0),
        noche: ds.reduce((a, d) => a + (d.noche || 0), 0),
      };
    };
    const lineasGrupo = (G, ahora) => `${esc(t('pd_ahora'))} <span class="o">↑ ${esc(bytes(ahora[0]))}/s</span> · <span class="i">↓ ${esc(bytes(ahora[1]))}/s</span><br>
        ${esc(tn('pd_conexiones', G.activas))} · ${esc(tn('pd_paises', G.paises))} · ${esc(tn('pd_empresas', G.empresas))}<br>
        <span class="t">${esc(t('pd_salio').replace('{n}', pct(G.fuera, G.tot)).replace('{pais}', nombrePais(st.yo)))}</span>`;
    // Five minutes in eight heights of a block, one per 10 seconds: the shape of the activity,
    // in text, so it lives inside the panel that is rewritten every second.
    const bloques = (serie) => { const mx = Math.max(...serie); return mx > 0 ? serie.map((v) => (v > 0 ? '▁▂▃▄▅▆▇█'[Math.min(7, Math.ceil(v / mx * 8) - 1)] : '·')).join('') : ''; };
    // The antivirus, in full (the owner, 13 Oct 2026: «la telemetría de él todo el tiempo me gustaría
    // tenerla revisada… se ve muy pobre»): what for, by its maker's own documentation; where to;
    // the last five minutes; when it last spoke; its lookups by day and by night. All measured here.
    // Inside the computer (1.0.24): processor, memory and reads/writes, as Windows counts them.
    const pct1 = (x) => (x === null || x === undefined ? '—' : `${x.toLocaleString(LOC, { maximumFractionDigits: 1 })} %`);
    const htmlDentroAV = (p) => {
      const a = p.act; if (!a) return '';
      const midiendo = a.cpu === null || a.cpu === undefined;
      let h = `<div class="sub-av">${esc(t('pd_av_dentro').replace('{prog}', p.nombre))}</div>`;
      h += midiendo ? `<div class="t">${esc(t('pd_av_dentro_midiendo'))}</div>` : `<div>${esc(t('pd_av_cpu'))} <b>${esc(pct1(a.cpu))}</b> · ${esc(t('pd_av_memoria'))} <b>${esc(bytes(a.memoria))}</b></div><div>${esc(t('pd_av_lee'))} <span class="i">${esc(bytes(a.lee || 0))}/s</span> · ${esc(t('pd_av_escribe'))} <span class="o">${esc(bytes(a.escribe || 0))}/s</span> · ${esc(tn('pd_av_procesos', a.procesos))}</div>`;
      const bc = bloques(a.serie_cpu || []), be = bloques(a.serie_es || []);
      if (bc) h += `<div class="fila"><span class="t">${esc(t('pd_av_cpu'))}</span><span class="bloques">${esc(bc)}</span></div>`;
      if (be) h += `<div class="fila"><span class="t">${esc(t('pd_av_es'))}</span><span class="bloques">${esc(be)}</span></div>`;
      return h + `<div class="t">${esc(t('pd_av_dentro_nota'))}</div>`;
    };
    const htmlAV = (A) => {
      let h = A.ps.map(htmlDentroAV).join('');
      const porUso = {};
      A.ds.forEach((d) => { const k = d.doc === false ? 'sin' : d.doc ? d.uso || 'otros' : null; if (!k) return; const u = porUso[k] || (porUso[k] = [0, 0]); u[0] += d.sub; u[1] += d.baj; });
      const usos = Object.entries(porUso).sort((a, b) => b[1][0] + b[1][1] - (a[1][0] + a[1][1]));
      if (usos.length) {
        h += `<div class="sub-av">${esc(t('pd_av_para_que'))}</div>`;
        usos.forEach(([k, [e, r]]) => { h += `<div class="fila"><span class="${k === 'sin' ? 'nd' : ''}">${esc(k === 'sin' ? t('pd_av_sin_doc') : t('av_uso_' + k))}</span><span><span class="o">↑ ${esc(bytes(e))}</span> <span class="i">↓ ${esc(bytes(r))}</span></span></div>`; });
      }
      const top = A.ds.filter((d) => d.sub + d.baj + d.bloq > 0).sort((a, b) => b.sub + b.baj - (a.sub + a.baj)).slice(0, 6);
      if (top.length) {
        h += `<div class="sub-av">${esc(t('pd_av_adonde'))}</div>`;
        top.forEach((d) => { h += `<div class="fila enlace-fila" data-dest="${esc(d.k)}" tabindex="0" role="button"><span>${d.doc === false ? '<span class="nd">●</span> ' : ''}${esc(d.host)} <span class="t">${esc(d.pais || '—')}</span></span><span class="${bloqueado(d) ? 'c' : 'o'}">${bloqueado(d) ? esc(tn('pd_parados', d.bloq)) : '↑ ' + esc(bytes(d.sub))}</span></div>`; });
      }
      const serie = new Array(N_CUBOS).fill(0); A.ps.forEach((p) => (st.serieP[p.id] || []).forEach((v, i) => { serie[i] += v; }));
      const b = bloques(serie);
      h += `<div class="sub-av">${esc(t('pd_av_5min'))}</div><div class="bloques">${b ? esc(b) : `<span class="t">${esc(t('pd_av_5min_nada'))}</span>`}</div>`;
      const ultima = Math.max(0, ...A.ds.map((d) => d.ultima || 0)), consultas = A.ds.reduce((a, d) => a + (d.consultas || 0), 0);
      h += `<div>${ultima ? esc(t('pd_av_ultima').replace('{t}', hace(Date.now() - ultima))) : ''}${consultas ? (ultima ? ' · ' : '') + esc(tn('pd_consultas', consultas)) : ''}</div>`;
      A.ps.forEach((p) => { h += `<div class="t">${esc(p.nombre)}: ${esc(p.procesos.join(' · '))} · ${esc(p.firma ? t('pd_firmado').replace('{firma}', p.firma) : t('pd_sin_firma'))}</div>`; });
      if (A.ps.some((p) => /defender/i.test(p.nombre))) h += `<div class="t">${esc(t('pd_av_defender_wu'))}</div>`;
      return h;
    };
    const htmlDirecto = (completo) => {
      const yo = st.lugar, dia = esDeDia(yo.lon, yo.lat);
      const [slon, slat] = T.solLL, sobre = paisEn(slon, slat);
      const ahoraG = st.ahoraG || { datos: [0, 0], av: [0, 0] };
      const D = resumenGrupo((p) => !esAV(p)), A = resumenGrupo(esAV);
      let h = `<b>● ${esc(t('pd_en_directo').toUpperCase())}</b> <span class="t">· ${esc(hora(Date.now()))}</span><br>
        ${esc(t(dia ? 'pd_es_de_dia' : 'pd_es_de_noche').replace('{pais}', nombrePais(st.yo)))} <span class="t">· ${esc(t('pd_sol_sobre').replace('{lugar}', sobre ? nombrePais(sobre) : t('pd_oceano')))} (${slat.toFixed(1)}°, ${slon.toFixed(1)}°)</span>`;
      // The data: the browser and the other programs.
      h += `<div class="tele"><b class="s">${esc(t('pd_tele_datos').toUpperCase())}</b> <span class="t">· ${esc(t('pd_tele_datos_sub'))}</span><br>
        ${lineasGrupo(D, ahoraG.datos)}<br>
        <span class="c">${esc(tn('pd_puertas', D.cortes))} · ${esc(tn('pd_intentos', D.intentos))}</span></div>`;
      // The antivirus, on its own.
      h += `<div class="tele"><b class="s">${esc(t('pd_tele_av').toUpperCase())}</b> <span class="t">· ${esc(t('pd_tele_av_sub'))}</span><br>`;
      if (st.datos && st.datos.vigilando === false) h += `<span class="c">${esc(t('pd_vigilancia_off_corto'))}</span>`;
      else if (!A.ps.length) h += `<span class="t">${esc(t('pd_sin_antivirus'))}</span>`;
      else {
        const avs = A.ds.filter((d) => d.doc !== null && d.doc !== undefined), su = avs.reduce((a, d) => a + d.sub, 0), doc = avs.filter((d) => d.doc).reduce((a, d) => a + d.sub, 0);
        const enteros = A.ps.filter((p) => p.cortado).map((p) => p.nombre);
        h += `${lineasGrupo(A, ahoraG.av)}<br>
          ${esc(t('pd_documentado').replace('{n}', su ? pct(doc, su) : '—'))} · ${esc(tn('pd_sin_documentar', A.ds.filter((d) => d.doc === false).length))}<br>
          <span class="t">${esc(t('pd_doc_matiz'))}</span><br>
          <span class="t">${esc(t('pd_noche_av').replace('{x}', bytes(A.noche)))}</span><br>
          <span class="c">${enteros.length ? esc(t('pd_av_cortado').replace('{n}', enteros.join(', '))) + ' · ' : ''}${esc(tn('pd_puertas', A.cortes))} · ${esc(tn('pd_intentos', A.intentos))}</span>`;
        if (completo) h += htmlAV(A);
      }
      return h + '</div>';
    };
    const tipoTexto = (p) => t('pd_tipo_' + p.tipo);
    // The side list: a state, then its cities, then the arrivals. Counts are how many go there.
    const arbolZonas = () => {
      if (!VISIBLES.length) return '';
      const zonas = new Map();
      VISIBLES.forEach((d) => {
        const s = sitioDe(d);
        const zkey = (s.pais + '|' + s.region).toLowerCase();
        let z = zonas.get(zkey);
        if (!z) { z = { pais: s.pais, region: s.region, n: 0, ciudades: new Map() }; zonas.set(zkey, z); }
        z.n++;
        const ckey = s.ciudad.toLowerCase();
        let c = z.ciudades.get(ckey);
        if (!c) { c = { ciudad: s.ciudad, ds: [] }; z.ciudades.set(ckey, c); }
        c.ds.push(d);
      });
      const lista = [...zonas.values()].sort((a, b) => b.n - a.n || (a.region || a.pais).localeCompare(b.region || b.pais));
      const varios = new Set(lista.map((z) => z.pais).filter(Boolean)).size > 1;
      let h = `<h4>${esc(t('pd_adonde'))}</h4><div class="zonas">`;
      lista.forEach((z) => {
        const zkey = (z.pais + '|' + z.region).toLowerCase();
        const abierta = zonaAbierta === zkey;
        const titulo = z.region || (z.pais ? nombrePais(z.pais) : '—');
        const extra = z.region && varios && z.pais ? ` <span class="t">${esc(nombrePais(z.pais))}</span>` : '';
        h += `<button type="button" class="fila zona" data-zona="${esc(zkey)}" aria-expanded="${abierta}"><span>${esc(titulo)}${extra}</span><b>${num(z.n)}</b></button>`;
        if (!abierta) return;
        const ciudades = [...z.ciudades.values()].sort((a, b) => b.ds.length - a.ds.length);
        h += `<div class="zona-dentro">`;
        ciudades.forEach((c) => {
          if (!c.ciudad) { h += listaDest(c.ds, 40); return; }
          const ckey = zkey + '|' + c.ciudad.toLowerCase();
          const cab = ciudadAbierta === ckey;
          h += `<button type="button" class="fila ciudad" data-ciudad="${esc(ckey)}" aria-expanded="${cab}"><span>${esc(c.ciudad)}</span><b>${num(c.ds.length)}</b></button>`;
          if (cab) h += listaDest(c.ds, 40);
        });
        h += `</div>`;
      });
      return h + `</div>`;
    };
    const pintaDatos = () => {
      if (!datosAbierto) return;
      const ahoraMs = performance.now();
      const tot = DEST.reduce((a, d) => a + d.sub, 0), baj = DEST.reduce((a, d) => a + d.baj, 0), paq = DEST.reduce((a, d) => a + d.paq, 0);
      // Live figures first, then the cut's button, then the rest. The button lives outside what is
      // rewritten every second, so a click or the keyboard's focus on it is never lost.
      $('datos-vivo').innerHTML = `<h4>${esc(t('pd_en_directo'))}</h4><div>${htmlDirecto(true)}</div>`;
      pintaCorte();
      // Programs.
      let h = arbolZonas() + `<h4>${esc(t('pd_programas'))} <span class="s">${PROD.length}</span></h4>`;
      PROD.forEach((p) => {
        const ds = DEST.filter((d) => d.p === p.id), act = ds.filter((d) => d.ult && ahoraMs - d.ult < 15000 && !bloqueado(d)).length;
        const avs = ds.filter((d) => d.doc !== null && d.doc !== undefined), su = avs.reduce((a, d) => a + d.sub, 0), doc = avs.filter((d) => d.doc).reduce((a, d) => a + d.sub, 0);
        const chip = p.cortado ? `<span class="chip c">${esc(t('pd_chip_cortado'))}</span>` : `<span class="chip">${esc(t('pd_chip_pasa'))}</span>`;
        h += `<div class="prod enlace-fila" data-c="${p.color}" data-prod="${esc(p.id)}" tabindex="0" role="button"><div class="fila"><span><b>${esc(p.nombre)}</b> <span class="t">${esc(tipoTexto(p))}</span></span>${chip}</div>
          <div class="t">${esc(p.procesos.join(' · '))} · ${esc(p.firma ? t('pd_firmado').replace('{firma}', p.firma) : t('pd_sin_firma'))}</div>
          <div><span class="o">↑ ${esc(bytes(p.sub))}</span> · <span class="i">↓ ${esc(bytes(p.baj))}</span> · ${esc(tn('pd_conexiones', act))} · ${esc(tn('pd_paises', new Set(ds.map((d) => d.pais).filter((x) => x && x[0] !== '@')).size))}</div>`
          + (p.tipo === 'antivirus' ? `<div class="barrita"><i data-w="${pct(doc, su)}" data-bg="var(--shield)"></i><i data-w="${su ? 100 - pct(doc, su) : 0}" data-bg="var(--undoc)"></i></div>
          <div class="t">${esc(t('pd_documentado').replace('{n}', su ? pct(doc, su) : '—'))} · ${esc(tn('pd_sin_documentar', ds.filter((d) => d.doc === false).length))}</div>` : '')
          + `</div>`;
      });
      // Destinations.
      const top = DEST.slice().sort((a, b) => (b.sub + b.baj) - (a.sub + a.baj) || b.bloq - a.bloq).slice(0, 30), mx = Math.max(1, ...top.map((d) => d.sub));
      h += `<h4>${esc(t('pd_destinos'))} <span class="s">${num(DEST.length)}</span></h4>`;
      top.forEach((d) => {
        const nuevo = d.visto && Date.now() - d.visto < 180000, bl = bloqueado(d);
        h += `<div class="fila enlace-fila" data-dest="${esc(d.k)}" tabindex="0" role="button"><span title="${esc(d.host)}">${esc(d.host)}${nuevo ? ` <span class="chip">${esc(t('pd_chip_nuevo'))}</span>` : ''}</span><span class="${bl ? 'c' : 'o'}">${bl || (!d.sub && d.bloq) ? esc(tn('pd_parados', d.bloq)) : esc(bytes(d.sub))}</span></div>
          <div class="barrita"><i data-w="${bl ? 100 : Math.max(2, d.sub / mx * 100)}" data-bg="${bl ? 'rgba(255,107,122,.5)' : prod(d.p).color}"></i></div>`;
      });
      if (st.datos && st.datos.destinos_omitidos) h += `<div class="t">${esc(tn('pd_omitidos', st.datos.destinos_omitidos))}</div>`;
      // Companies that own the networks.
      const redes = {}; DEST.filter((d) => !d.sede && d.red && d.red[0] !== '@').forEach((d) => { const k = d.red; redes[k] = redes[k] || { b: 0, asn: d.asn, n: 0 }; redes[k].b += d.sub + d.baj; redes[k].n++; });
      const rr = Object.entries(redes).sort((a, b) => b[1].b - a[1].b).slice(0, 15), mr = Math.max(1, ...rr.map((x) => x[1].b));
      h += `<h4>${esc(t('pd_empresas_redes'))} <span class="s">${num(Object.keys(redes).length)}</span></h4>`;
      rr.forEach(([n, r]) => { h += `<div class="fila enlace-fila" data-red="${esc(n)}" tabindex="0" role="button"><span>${esc(n)} <span class="t">${r.asn ? 'AS' + r.asn : ''} · ${esc(tn('pd_nombres', r.n))}</span></span><span>${esc(bytes(r.b))}</span></div><div class="barrita"><i data-w="${Math.max(2, r.b / mr * 100)}" data-bg="var(--in)"></i></div>`; });
      // Where the servers are.
      const j = { yo: 0, ue: 0, us: 0, otros: 0, sin: 0 };
      DEST.filter((d) => !d.sede).forEach((d) => { j[!d.pais || d.pais[0] === '@' ? 'sin' : d.pais === st.yo ? 'yo' : UE.has(d.pais) ? 'ue' : d.pais === 'US' ? 'us' : 'otros'] += d.sub; });
      h += `<h4>${esc(t('pd_donde'))}</h4>
        <div class="barrita gruesa"><i data-w="${pct(j.us, tot)}" data-bg="var(--out)"></i><i data-w="${pct(j.ue, tot)}" data-bg="var(--in)"></i><i data-w="${pct(j.otros, tot)}" data-bg="#a98bff"></i><i data-w="${pct(j.yo, tot)}" data-bg="var(--shield)"></i><i data-w="${pct(j.sin, tot)}" data-bg="#55657a"></i></div>
        <div class="leyenda-j"><span><i data-bg="var(--out)"></i>${esc(t('pd_eeuu'))} ${pct(j.us, tot)} %</span><span><i data-bg="var(--in)"></i>${esc(t('pd_ue'))} ${pct(j.ue, tot)} %</span><span><i data-bg="#a98bff"></i>${esc(t('pd_otros'))} ${pct(j.otros, tot)} %</span><span><i data-bg="var(--shield)"></i>${esc(nombrePais(st.yo))} ${pct(j.yo, tot)} %</span>${j.sin ? `<span><i data-bg="#55657a"></i>${esc(t('pd_sin_pais'))} ${pct(j.sin, tot)} %</span>` : ''}</div>`;
      // How it travels.
      const tcp = DEST.filter((d) => d.proto === 'tcp').reduce((a, d) => a + d.sub, 0), udp = DEST.filter((d) => d.proto === 'udp').reduce((a, d) => a + d.sub, 0);
      const p443 = DEST.filter((d) => d.puerto === 443).reduce((a, d) => a + d.sub, 0), p80 = DEST.filter((d) => d.puerto === 80).reduce((a, d) => a + d.sub, 0);
      h += `<h4>${esc(t('pd_como_viaja'))}</h4>
        <div class="barrita gruesa"><i data-w="${pct(tcp, tot)}" data-bg="var(--in)"></i><i data-w="${pct(udp, tot)}" data-bg="#a98bff"></i></div>
        <div>TCP ${pct(tcp, tot)} % · UDP ${pct(udp, tot)} %</div>
        <div>${esc(t('pd_puerto_443'))} <b>${pct(p443, tot)} %</b> · ${esc(t('pd_puerto_80'))} <b class="${p80 ? 'c' : ''}">${pct(p80, tot)} %</b></div>
        <div>${esc(t('pd_paquetes').replace('{s}', num(Math.round(st.ahora[2]))).replace('{n}', num(paq)))}</div>
        <div class="t">${esc(t('pd_cifrado'))}</div>`;
      // The world now (the real sun).
      const [slon, slat] = T.solLL, sobre = paisEn(slon, slat);
      h += `<h4>${esc(t('pd_mundo'))}</h4><div>${esc(t('pd_sol_sobre').replace('{lugar}', sobre ? nombrePais(sobre) : t('pd_oceano')))} <span class="t">(${slat.toFixed(1)}°, ${slon.toFixed(1)}°)</span></div>`;
      [st.yo, ...paisesUsados.filter((c) => c !== st.yo).slice(0, 16)].forEach((c) => { const ce = centros[c]; if (!ce) return; const dia = esDeDia(ce.lon, ce.lat); h += `<div class="fila"><span>${dia ? '☀' : '☾'} ${esc(nombrePais(c))}${c === st.yo ? ` <span class="t">(${esc(t('pd_tu'))})</span>` : ''}</span><span class="t">${esc(t(dia ? 'pd_de_dia' : 'pd_de_noche'))}</span></div>`; });
      h += `<div class="t">${esc(t('pd_centro_pais'))}</div>`;
      // The window.
      const noche = DEST.reduce((a, d) => a + d.noche, 0);
      h += `<h4>${esc(t('pd_ventana').replace('{h}', st.datos ? st.datos.ventana_h : 24))}</h4>
        <div><span class="o">↑ ${esc(bytes(tot))}</span> · <span class="i">↓ ${esc(bytes(baj))}</span> · ${esc(t('pd_madrugada').replace('{x}', bytes(noche)))}</div>
        <div>${esc(t('pd_abierto').replace('{t}', hace(Date.now() - st.abierto)))} · <span class="c">${esc(tn('pd_intentos', DEST.reduce((a, d) => a + d.bloq, 0)))}</span></div>`;
      if (st.datos && st.datos.programas_omitidos) h += `<div class="t">${esc(tn('pd_programas_omitidos', st.datos.programas_omitidos))}</div>`;
      $('datos-resto').innerHTML = h;
      estilos(panelDatos);
    };
    const telemetria = () => {
      const el = $('telemetria');
      el.hidden = st.vista === 'programas' || datosAbierto || st.Wc < 700;
      if (!el.hidden) el.innerHTML = htmlDirecto();
    };

    // ---------- What is under the pointer ----------
    const queHay = (x, y) => {
      let mejor = null, dm = Infinity;
      st.dianas.forEach((d) => { const dd = Math.hypot(d.x - x, d.y - y); if (dd < d.r && dd < dm) { dm = dd; mejor = d; } });
      // The map itself is context, not a control. Hover only reacts to a point backed by real
      // measurements; empty countries and oceans stay still.
      return mejor;
    };
    const usoTexto = (d) => (d.doc === null || d.doc === undefined ? '' : d.uso ? `<div>${esc(t('pd_para_que'))} <b>${esc(t('av_uso_' + d.uso))}</b></div>` : `<div class="nodoc">${esc(t('pd_no_documenta'))}</div>`);
    const textoTip = (hh) => {
      if (hh.tipo === 'lugar') {
        const g = hh.ref;
        const nombre = g.ciudad || g.region || (g.pais ? nombrePais(g.pais) : '—');
        const linea = [g.ciudad ? g.region : '', g.pais ? nombrePais(g.pais) : ''].filter(Boolean).join(' · ');
        return `<b>${esc(nombre)}</b>${linea && linea !== nombre ? `<div class="t">${esc(linea)}</div>` : ''}<div>${esc(tn('pd_llegadas', g.ds.length))}</div>`;
      }
      if (hh.tipo === 'dest') {
        const d = hh.ref, p = prod(d.p), bl = bloqueado(d);
        const estado = bl ? `<span class="estado corta">${esc(d.cortado ? t('pd_cortado_nombre') : d.ancho ? t('pd_cortado_ancho') : t('pd_cortado_programa').replace('{prog}', p.nombre))}</span>` : `<span class="estado pasa">${esc(t('pd_pasa'))}</span>`;
        const dia = centros[d.pais] ? esDeDia(centros[d.pais].lon, centros[d.pais].lat) : null;
        const donde = d.lugar ? t('pd_servidor_en').replace('{lugar}', d.lugar.ciudad).replace('{pais}', nombrePais(d.lugar.pais)).replace('{red}', d.red || '—') : d.sede ? t('pd_sede').replace('{pais}', nombrePais(d.pais)).replace('{empresa}', d.red || '—') : t('pd_servidor').replace('{pais}', d.pais ? nombrePais(d.pais) : '—').replace('{red}', d.red || '—');
        return `<div class="mono">${esc(d.host)}</div><div>${esc(p.nombre)} · <span class="mono">${esc(p.procesos.join(' · '))}</span></div>${usoTexto(d)}
          ${d.empresa && !d.sede ? `<div>${esc(t('pd_nombre_de').replace('{empresa}', d.empresa))}</div>` : ''}
          <div>${esc(donde)}${d.asn ? ` <span class="mono">AS${d.asn}</span>` : ''}</div>
          ${d.red && !d.sede && d.red[0] !== '@' && d.red !== d.empresa ? `<div class="t">${esc(t('pd_red_no_es'))}</div>` : ''}
          ${d.proto ? `<div>${esc((d.vias && d.vias.length ? d.vias : [d.proto + '/' + d.puerto]).join(' · '))}${d.puerto === 443 ? ' ' + esc(t('pd_cifrado_corto')) : d.puerto === 80 ? ' ' + esc(t('pd_sin_cifrar_corto')) : ''} · ${esc(tn('pd_paquetes_n', d.paq))}</div>` : ''}
          <div>${esc(t('pd_enviado'))} <b>${esc(bytes(d.sub))}</b> · ${esc(t('pd_recibido'))} ${esc(bytes(d.baj))}${d.consultas ? ' · ' + esc(tn('pd_consultas', d.consultas)) : ''}${d.bloq ? ' · ' + esc(tn('pd_parados', d.bloq)) : ''}</div>
          <div class="t">${d.primera ? esc(t('pd_primera').replace('{t}', hace(Date.now() - d.primera))) : ''}${dia === null ? '' : ' · ' + esc(t(dia ? 'pd_alli_dia' : 'pd_alli_noche'))}</div>${estado}`;
      }
      if (hh.tipo === 'prod') {
        const p = hh.ref, ds = DEST.filter((d) => d.p === p.id);
        return `<b>${esc(p.nombre)}</b> · <span class="t">${esc(tipoTexto(p))}</span><div class="mono">${esc(p.procesos.join(' · '))}</div>
          <div>${esc(tn('pd_destinos_en', ds.length).replace('{p}', new Set(ds.map((d) => d.pais).filter((x) => x && x[0] !== '@')).size))}</div>
          <div>${esc(t('pd_enviado'))} <b>${esc(bytes(p.sub))}</b> · ${esc(t('pd_recibido'))} ${esc(bytes(p.baj))}</div>
          ${p.cortado ? `<span class="estado corta">${esc(t('pd_prog_cortado'))}</span>` : `<span class="estado pasa">${esc(t('pd_prog_pasa'))}</span>`}`;
      }
      if (hh.tipo === 'red') {
        const r = hh.ref, ds = DEST.filter((d) => d.red === r && !d.sede), asn = (ds.find((d) => d.asn) || {}).asn;
        const paises = [...new Set(ds.map((d) => d.pais).filter((x) => x && x[0] !== '@'))].map(nombrePais).join(', ');
        return `<b>${esc(r)}</b>${asn ? ` <span class="mono">AS${asn}</span>` : ''}<div>${esc(t('pd_red_paises'))} <b>${esc(paises || '—')}</b></div><div class="t">${esc(t('pd_red_no_es'))}</div>
          <div>${esc(t('pd_enviado'))} <b>${esc(bytes(ds.reduce((a, d) => a + d.sub, 0)))}</b> · ${esc(t('pd_recibido'))} ${esc(bytes(ds.reduce((a, d) => a + d.baj, 0)))}</div>
          <div class="mono arriba4">${ds.slice(0, 10).map((d) => '· ' + esc(d.host)).join('<br>')}${ds.length > 10 ? '<br>…' : ''}</div>`;
      }
      if (hh.tipo === 'yo') { const ce = st.lugar, por = ce.fuente === 'equipo' ? t('pd_lugar_equipo').replace('{precision}', ce.precision < 1000 ? Math.max(1, Math.round(ce.precision)) + ' m' : Math.max(1, Math.round(ce.precision / 1000)) + ' km') : ce.fuente === 'manual' ? t('pd_lugar_manual') : ce.fuente === 'zona' ? t('pd_lugar_zona') : t('pd_lugar_pais'); return `<b>${esc(t('pd_tu_ordenador'))}</b> · ${esc(rotuloYo())} · ${esc(t(esDeDia(ce.lon, ce.lat) ? 'pd_de_dia' : 'pd_de_noche'))}<div class="t">${esc(por)}</div><div>${esc(tn('pd_vigila_n', PROD.length))}</div>`; }
      if (hh.tipo === 'pais') {
        const c = hh.ref, ds = DEST.filter((d) => d.pais === c), ce = centros[c], dia = ce ? esDeDia(ce.lon, ce.lat) : null;
        const cab = `<b>${esc(nombrePais(c))}</b>${dia === null ? '' : ` <span class="t">· ${esc(t(dia ? 'pd_de_dia' : 'pd_de_noche'))}</span>`}`;
        if (c === st.yo && !ds.length) return `${cab}<div>${esc(t('pd_aqui_tu'))}</div>`;
        if (!ds.length) return `${cab}<div class="t">${esc(t('pd_pais_nada'))}</div>`;
        const redes = [...new Set(ds.map((d) => d.red).filter((r) => r && r[0] !== '@'))].join(', ');
        return `${cab}<div>${esc(t('pd_redes_aqui'))} <b>${esc(redes || '—')}</b></div>
          <div>${esc(t('pd_enviado'))} <b>${esc(bytes(ds.reduce((a, d) => a + d.sub, 0)))}</b> · ${esc(t('pd_recibido'))} ${esc(bytes(ds.reduce((a, d) => a + d.baj, 0)))}</div>
          <div class="mono arriba4">${ds.slice(0, 12).map((d) => (bloqueado(d) ? '✕ ' : '· ') + esc(d.host)).join('<br>')}${ds.length > 12 ? '<br>…' : ''}</div>`;
      }
      return '';
    };
    const actualizaTip = () => {
      const { x, y } = st.raton, hh = queHay(x, y);
      cv.style.cursor = hh ? 'pointer' : 'default';
      if (!hh) { tip.hidden = true; st.hover = null; return; }
      const html = textoTip(hh) + `<div class="t clic">${esc(t('pd_clic_info'))}</div>`;
      if (tip.innerHTML !== html) { tip.innerHTML = html; estilos(tip); }
      tip.hidden = false;
      const tw = tip.offsetWidth, th = tip.offsetHeight;
      tip.style.left = Math.min(st.Wc - tw - 8, x + 16) + 'px';
      tip.style.top = Math.min(st.H - th - 8, Math.max(8, y + 14)) + 'px';
      st.hover = hh;
    };

    // ---------- A click opens what is under it (the owner, 13 Oct 2026: «cuando clique, que me
    // dé información… haciendo clics y enlaces») ----------
    // The card says everything the panel knows of it, links to the pages that say more (the
    // company's profile, the country in the extract, the antivirus's page) and holds the one
    // button that cuts or lets through. Nothing is cut by a click on the picture any more.
    const detalle = $('detalle'), cuerpoDet = $('detalle-cuerpo');
    let abierto = null;
    const enlace = (href, txt) => (soloMirar ? '' : `<a href="${esc(href)}">${esc(txt)} →</a>`);
    const botonDest = (d) => `<button type="button" class="fila-det" data-dest="${esc(d.k)}"><span>${bloqueado(d) ? '✕ ' : ''}${esc(d.host)}</span><span class="${bloqueado(d) ? 'c' : 'o'}">${bloqueado(d) ? esc(tn('pd_parados', d.bloq)) : '↑ ' + esc(bytes(d.sub))}</span></button>`;
    // Which «y N más» lists are open, so the live refresh does not fold them back.
    const masAbiertos = new Set();
    document.addEventListener('toggle', (e) => { const d = e.target; if (d && d.dataset && d.dataset.mas) { if (d.open) masAbiertos.add(d.dataset.mas); else masAbiertos.delete(d.dataset.mas); } }, true);
    const listaDest = (ds, tope = 12) => { const o = ds.slice().sort((a, b) => b.sub - a.sub || b.bloq - a.bloq); return o.length ? `<div class="lista-det">${o.slice(0, tope).map(botonDest).join('')}${o.length > tope ? `<details class="mas-det" data-mas="${esc(o[0].k)}"${masAbiertos.has(o[0].k) ? ' open' : ''}><summary>${esc(tn('pd_y_mas', o.length - tope))}</summary>${o.slice(tope).map(botonDest).join('')}</details>` : ''}</div>` : ''; };
    const cerca = (lon, lat) => (st.vista === 'destinos' && lon !== undefined ? `<button type="button" class="btn-hud" data-cerca="${lon},${lat}">${esc(t('pd_ver_cerca'))}</button>` : '');
    const htmlDetalle = (hh) => {
      let h = `<div class="cuerpo">${textoTip(hh)}</div>`, acciones = '', enlaces = [];
      if (hh.tipo === 'lugar') {
        const g = hh.ref;
        h = `<div class="cuerpo">${textoTip(hh).replace(/<div class="mono arriba4">[\s\S]*$/, '')}</div><h4>${esc(t('pd_det_destinos'))}</h4>` + listaDest(g.ds);
        acciones += cerca(g.lon, g.lat);
      } else if (hh.tipo === 'dest') {
        const d = hh.ref, p = prod(d.p);
        if (d.lugar) h += `<p><b>${esc(t('pd_lugar_titulo'))}</b> ${esc(d.lugar.ciudad)} · ${esc(nombrePais(d.lugar.pais))}</p><p class="t">${esc(t(d.lugar.fuente === 'region' ? 'pd_lugar_region' : d.lugar.fuente === 'base' ? 'pd_lugar_base' : 'pd_lugar_geofeed').replace('{op}', t('pd_op_' + d.lugar.operador)).replace('{region}', d.lugar.region))}</p>`;
        else if (!d.sede) h += `<p class="t">${esc(t('pd_pos_pais').replace('{pais}', d.pais ? nombrePais(d.pais) : '—'))}</p>`;
        if (!soloMirar) acciones += !d.destino ? `<p class="t">${esc(t('pd_sin_nombre_no_corta'))}</p>` : d.ancho ? `<p class="t">${esc(t('pd_ancho_dejar'))}</p>` : `<button type="button" class="btn-hud${d.cortado ? '' : ' peligro'}" data-cortar="dest">${esc(t(d.cortado ? 'pd_btn_dejar' : 'pd_btn_cortar'))}</button>`;
        acciones += cerca(d.lon, d.lat);
        if (d.empresa) enlaces.push(enlace('/empresa?e=' + encodeURIComponent(d.empresa), t('pd_ver_empresa').replace('{empresa}', d.empresa)));
        if (d.red && d.red[0] !== '@' && d.red !== d.empresa) enlaces.push(enlace('/empresa?e=' + encodeURIComponent(d.red), t('pd_ver_red').replace('{red}', d.red)));
        if (d.pais && d.pais[0] !== '@') enlaces.push(enlace('/extracto?pais=' + encodeURIComponent(d.pais), t('pd_ver_pais_extracto').replace('{pais}', nombrePais(d.pais))));
        if (p.tipo === 'antivirus') enlaces.push(enlace('/antivirus', t('pd_ver_antivirus')));
      } else if (hh.tipo === 'prod') {
        const p = hh.ref;
        if (!soloMirar) acciones += p.cortable ? `<button type="button" class="btn-hud${p.cortado ? '' : ' peligro'}" data-cortar="prod">${esc(t(p.cortado ? 'pd_btn_dejar_prog' : 'pd_btn_cortar_prog'))}</button>` : `<p class="t">${esc(t('pd_no_cortable'))}</p>`;
        h += `<h4>${esc(t('pd_det_destinos'))}</h4>` + listaDest(DEST.filter((d) => d.p === p.id));
        if (p.tipo === 'antivirus') enlaces.push(enlace('/antivirus', t('pd_ver_antivirus')));
      } else if (hh.tipo === 'pais') {
        const c = hh.ref, ce = centros[c];
        h = `<div class="cuerpo">${textoTip(hh).replace(/<div class="mono arriba4">[\s\S]*$/, '')}</div><h4>${esc(t('pd_det_destinos'))}</h4>` + listaDest(DEST.filter((d) => d.pais === c));
        if (ce) acciones += cerca(ce.lon, ce.lat);
        enlaces.push(enlace('/extracto?pais=' + encodeURIComponent(c), t('pd_ver_pais_extracto').replace('{pais}', nombrePais(c))));
      } else if (hh.tipo === 'red') {
        const r = hh.ref;
        h = `<div class="cuerpo">${textoTip(hh).replace(/<div class="mono arriba4">[\s\S]*$/, '')}</div><h4>${esc(t('pd_det_destinos'))}</h4>` + listaDest(DEST.filter((d) => d.red === r && !d.sede));
        enlaces.push(enlace('/empresa?e=' + encodeURIComponent(r), t('pd_ver_red').replace('{red}', r)));
      } else if (hh.tipo === 'yo') acciones += cerca(st.lugar.lon, st.lugar.lat);
      enlaces = enlaces.filter(Boolean);
      return h + (acciones ? `<div class="acciones-det">${acciones}</div>` : '') + (enlaces.length ? `<div class="enlaces-det">${enlaces.join('')}</div>` : '');
    };
    const pintaDetalle = () => {
      if (!abierto) return;
      // A destination that is no longer measured keeps its last card; one that still is, its fresh figures.
      if (abierto.tipo === 'dest' && porClave.get(abierto.ref.k)) abierto.ref = porClave.get(abierto.ref.k);
      if (abierto.tipo === 'prod') abierto.ref = PROD.find((p) => p.id === abierto.ref.id) || abierto.ref;
      const html = htmlDetalle(abierto);
      if (cuerpoDet.innerHTML !== html) { cuerpoDet.innerHTML = html; estilos(cuerpoDet); }
    };
    const abreDetalle = (hh) => { abierto = hh; tip.hidden = true; detalle.hidden = false; pintaDetalle(); detalle.scrollTop = 0; };
    const cierraDetalle = () => { abierto = null; detalle.hidden = true; };
    $('detalle-cerrar').addEventListener('click', cierraDetalle);
    detalle.addEventListener('click', (e) => {
      const b = e.target.closest('button'); if (!b) return;
      if (b.dataset.dest) { const d = porClave.get(b.dataset.dest); if (d) abreDetalle({ tipo: 'dest', ref: d }); return; }
      if (b.dataset.cerca) { const [lon, lat] = b.dataset.cerca.split(',').map(Number); vuela(lon, lat); return; }
      if (b.dataset.cortar === 'dest' && abierto && abierto.tipo === 'dest') corteNombre(abierto.ref);
      if (b.dataset.cortar === 'prod' && abierto && abierto.tipo === 'prod') corteProducto(abierto.ref);
    });
    // The side panel and the deck open the same card.
    const abreDesde = (e) => {
      const el = e.target.closest('[data-dest],[data-prod],[data-red],[data-pais]'); if (!el || el.closest('#detalle')) return;
      if (el.dataset.dest) { const d = porClave.get(el.dataset.dest); if (d) abreDetalle({ tipo: 'dest', ref: d }); }
      else if (el.dataset.prod) { const p = PROD.find((x) => x.id === el.dataset.prod); if (p) abreDetalle({ tipo: 'prod', ref: p }); }
      else if (el.dataset.red) abreDetalle({ tipo: 'red', ref: el.dataset.red });
      else if (el.dataset.pais) abreDetalle({ tipo: 'pais', ref: el.dataset.pais });
    };
    stage.addEventListener('click', abreDesde);
    stage.addEventListener('keydown', (e) => { if ((e.key === 'Enter' || e.key === ' ') && e.target.matches('[data-dest],[data-prod],[data-red],[data-pais]') && e.target.tagName !== 'BUTTON') { e.preventDefault(); abreDesde(e); } });

    // ---------- Acting from the globe: cut or let through, always reversible ----------
    let temporizadorAviso = 0;
    const avisa = (txt, botones) => {
      const a = $('aviso');
      a.innerHTML = `<div>${esc(txt)}</div>` + (botones ? `<div class="aviso-botones">${botones.map((b, i) => `<button type="button" class="btn-hud${b.peligro ? ' peligro' : ''}" data-i="${i}">${esc(b.texto)}</button>`).join('')}</div>` : '');
      a.hidden = false; a.classList.toggle('pide', !!botones);
      a.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => { a.hidden = true; botones[+b.dataset.i].hacer(); }));
      clearTimeout(temporizadorAviso);
      if (!botones) temporizadorAviso = setTimeout(() => { a.hidden = true; }, 5200);
    };
    const corteNombre = async (d) => {
      if (soloMirar) { avisa(t('pd_repro_no_corta')); return; }
      if (!d.destino) { avisa(t('pd_sin_nombre_no_corta')); return; }
      if (d.ancho) { avisa(t('pd_ancho_dejar')); return; }
      try {
        if (d.regla !== null && d.regla !== undefined && d.regla >= 0) {
          await api(`/api/reglas/${d.regla}/deshacer`, { method: 'POST' });
          avisa(t('pd_hecho_dejar').replace('{nombre}', d.host));
        } else {
          const cuerpo = { scope: 'device', device_id: 'self', match_kind: 'domain', pattern: d.destino, action: 'cortar' };
          let r = await api('/api/reglas', { method: 'POST', body: cuerpo });
          if (r.necesita === 'confirmar') {
            avisa(r.mensaje || t('pd_confirmar'), [
              { texto: t('pd_cortar_igual'), peligro: true, hacer: async () => { const r2 = await api('/api/reglas', { method: 'POST', body: Object.assign({}, cuerpo, { confirmed: true }) }); if (r2.creada) avisa(t('pd_hecho_cortar').replace('{nombre}', d.host) + ' ' + t('pd_dns_tarda')); await cargar(); } },
              { texto: t('pd_cancelar'), hacer: () => {} },
            ]);
            return;
          }
          if (r.mensaje && !r.creada) { avisa(r.mensaje); return; }
          avisa(t('pd_hecho_cortar').replace('{nombre}', d.host) + ' ' + t('pd_dns_tarda'));
        }
        await cargar();
      } catch (e) { avisa(String(e && e.message || e)); }
    };
    const corteProducto = async (p) => {
      if (soloMirar) { avisa(t('pd_repro_no_corta')); return; }
      if (!p.cortable) { avisa(t('pd_no_cortable')); return; }
      const hacer = async () => {
        try { await api('/api/antivirus/cortar', { method: 'POST', body: { producto: p.id, cortar: !p.cortado } }); avisa((!p.cortado ? t('pd_hecho_cortar_prog') : t('pd_hecho_dejar_prog')).replace('{prog}', p.nombre)); await cargar(); } catch (e) { avisa(String(e && e.message || e)); }
      };
      if (p.cortado) { hacer(); return; }
      if (st.datos && !st.datos.cortar_disponible) { avisa(t('av_cortar_no_disponible')); return; }
      avisa(t('pd_aviso_cortar_prog').replace('{prog}', p.nombre), [{ texto: t('pd_cortar_igual'), peligro: true, hacer }, { texto: t('pd_cancelar'), hacer: () => {} }]);
    };
    // The same steps the top bar's «Protección máxima» took until 13 Oct 2026: a warning to
    // confirm before cutting (the antivirus then gets no updates), and one click to take it off.
    $('corte-av-btn').addEventListener('click', () => {
      const activa = !!(st.datos && st.datos.maxima);
      const hacer = async () => {
        try { await api('/api/antivirus/maxima', { method: 'POST', body: { activa: !activa } }); avisa(activa ? t('pd_maxima_off') : t('pd_maxima_on')); await cargar(); } catch (e) { avisa(String(e && e.message || e)); }
      };
      if (activa) { hacer(); return; }
      if (st.datos && !st.datos.cortar_disponible) { avisa(t('av_cortar_no_disponible')); return; }
      avisa(t('av_max_aviso'), [{ texto: t('av_max_confirmar'), peligro: true, hacer }, { texto: t('pd_cancelar'), hacer: () => {} }]);
    });

    // ---------- «Retrato»: five real minutes of this computer, kept by the person (13 Oct 2026) ----------
    // The owner wants every picture real, on the website too: a recording made here from the same
    // measurements, cleaned before it leaves (no user, no paths, no addresses, no clock times, no
    // private names: those are marked «oculto por privacidad», never replaced), as a PDF that says
    // the mode GUARDIANA was in, and as data (CSV, and JSON to replay it).
    const GRABA_MS = 5 * 60000;
    const grab = { on: false, t0: 0, filas: [], modo: null, dur: 0, fecha: '', zona: '' };
    const privado = (h) => !!h && (!h.includes('.') || /\.(local|lan|home|internal|localdomain|arpa|intranet|corp)$/i.test(h) || /^[\d.:]+$/.test(h) || /^\[/.test(h));
    const graba = (rec) => {
      if (!grab.on) return;
      const s = Math.round((performance.now() - grab.t0) / 1000);
      rec.forEach(({ d, env: e, recb: b, parada }) => {
        const p = prod(d.p), oculto = privado(d.destino);
        grab.filas.push({ s, programa: p.nombre, tipo: p.tipo, destino: oculto ? '' : (d.destino || ''), oculto, pais: d.pais || '', red: d.red && d.red[0] !== '@' ? d.red : '', empresa: d.empresa || '', enviados: e || 0, recibidos: b || 0, parados: parada || 0, documentado: d.doc === undefined ? null : d.doc });
      });
    };
    const modoAhora = async () => {
      let pm = null;
      try { pm = !!(await api('/api/proteccion-maxima')).activa; } catch (_) {}
      return { proteccion_maxima: pm, antivirus_cortado: !!(st.datos && st.datos.maxima) };
    };
    const mmss = (sg) => `${Math.floor(sg / 60)}:${String(Math.round(sg % 60)).padStart(2, '0')}`;
    const btnGrabar = $('grabar');
    if (soloMirar) btnGrabar.hidden = true;
    btnGrabar.title = t('pd_grabar_que');
    let relojGrab = 0;
    const pintaGrabar = () => {
      if (!grab.on) { btnGrabar.textContent = t('pd_grabar'); btnGrabar.setAttribute('aria-pressed', 'false'); return; }
      const sg = (performance.now() - grab.t0) / 1000;
      btnGrabar.textContent = t('pd_grabando').replace('{t}', mmss(sg)).replace('{total}', mmss(GRABA_MS / 1000));
      btnGrabar.setAttribute('aria-pressed', 'true');
      if (sg * 1000 >= GRABA_MS) paraGrabacion();
    };
    const empiezaGrabacion = async () => {
      grab.filas = []; grab.t0 = performance.now(); grab.on = true;
      const d = new Date();
      grab.fecha = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
      try { grab.zona = Intl.DateTimeFormat().resolvedOptions().timeZone || ''; } catch (_) { grab.zona = ''; }
      grab.modo = await modoAhora();
      $('retrato').hidden = true;
      clearInterval(relojGrab); relojGrab = setInterval(pintaGrabar, 1000); pintaGrabar();
      avisa(t('pd_grabacion_empieza'));
    };
    const paraGrabacion = async () => {
      if (!grab.on) return;
      grab.on = false; grab.dur = Math.round((performance.now() - grab.t0) / 1000);
      clearInterval(relojGrab); pintaGrabar();
      const fin = await modoAhora();
      // The mode is said as it was during the whole recording: if it changed, both are said.
      grab.modo = { inicio: grab.modo, fin };
      muestraRetrato();
    };
    btnGrabar.addEventListener('click', () => (grab.on ? paraGrabacion() : empiezaGrabacion()));
    // What the recording adds up to: the same sums the PDF and the panel show.
    const resumen = () => {
      const f = grab.filas, suma = (arr, k) => arr.reduce((a, x) => a + x[k], 0);
      const por = (clave) => { const m = new Map(); f.forEach((x) => { const k = x[clave] || '—'; const o = m.get(k) || { k, env: 0, rec: 0, dest: new Set(), parados: 0, tipo: x.tipo }; o.env += x.enviados; o.rec += x.recibidos; o.parados += x.parados; o.dest.add(x.destino || (x.oculto ? '·' : '')); m.set(k, o); }); return [...m.values()].sort((a, b) => b.env - a.env); };
      const dests = new Map();
      f.forEach((x) => { const k = [x.programa, x.destino, x.pais, x.red].join('|'); const o = dests.get(k) || { ...x, enviados: 0, recibidos: 0, parados: 0 }; o.enviados += x.enviados; o.recibidos += x.recibidos; o.parados += x.parados; dests.set(k, o); });
      const av = f.filter((x) => x.tipo === 'antivirus');
      return {
        env: suma(f, 'enviados'), rec: suma(f, 'recibidos'), parados: suma(f, 'parados'),
        programas: por('programa'), paises: por('pais').filter((x) => x.k !== '—' && x.k[0] !== '@'), redes: por('red').filter((x) => x.k !== '—'),
        destinos: [...dests.values()].sort((a, b) => b.enviados - a.enviados || b.parados - a.parados),
        envAv: suma(av, 'enviados'), ocultos: new Set(f.filter((x) => x.oculto).map((x) => x.programa + '|' + x.pais + '|' + x.red)).size,
      };
    };
    const textoModo = (m) => {
      const uno = (x) => [x.proteccion_maxima === null ? t('pd_modo_pm_nose') : t(x.proteccion_maxima ? 'pd_modo_pm_si' : 'pd_modo_pm_no'), t(x.antivirus_cortado ? 'pd_modo_av_si' : 'pd_modo_av_no')];
      const a = uno(m.inicio), b = uno(m.fin);
      const normal = !m.inicio.proteccion_maxima && !m.inicio.antivirus_cortado && !m.fin.proteccion_maxima && !m.fin.antivirus_cortado;
      const cambio = a.join('|') !== b.join('|');
      return { titulo: t(normal ? 'pd_modo_normal' : 'pd_modo_reforzado'), lineas: cambio ? [t('pd_modo_al_empezar') + ' ' + a.join(' · '), t('pd_modo_al_acabar') + ' ' + b.join(' · ')] : a };
    };
    const muestraRetrato = () => {
      const r = resumen(), m = textoModo(grab.modo);
      $('retrato-t').textContent = t('pd_retrato_titulo').replace('{t}', mmss(grab.dur));
      $('retrato-txt').textContent = grab.filas.length
        ? t('pd_retrato_resumen').replace('{env}', bytes(r.env)).replace('{rec}', bytes(r.rec)).replace('{p}', tn('pd_retrato_n_prog', r.programas.length)).replace('{d}', tn('pd_retrato_n_dest', r.destinos.length)).replace('{paises}', tn('pd_paises', r.paises.length))
        : t('pd_retrato_vacio');
      $('retrato-modo').textContent = m.titulo + ' ' + m.lineas.join(' · ');
      ['retrato-pdf', 'retrato-csv', 'retrato-json'].forEach((id) => { $(id).disabled = !grab.filas.length; });
      $('retrato').hidden = false;
      $('retrato-pdf').focus();
    };
    const baja = (nombre, tipo, contenido) => {
      const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([contenido], { type: tipo })); a.download = nombre;
      document.body.appendChild(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(a.href), 10000);
    };
    const nombreArchivo = (que, ext) => `guardiana-${que}-${grab.fecha}.${ext}`;
    const COLS = ['segundo', 'programa', 'tipo', 'destino', 'oculto_por_privacidad', 'pais_servidor', 'red', 'empresa_del_nombre', 'enviados', 'recibidos', 'parados', 'documentado_por_el_fabricante'];
    $('retrato-csv').addEventListener('click', () => {
      const q = (v) => { const x = v === null || v === undefined ? '' : String(v); return /[",\n]/.test(x) ? '"' + x.replace(/"/g, '""') + '"' : x; };
      const filas = grab.filas.map((x) => [x.s, x.programa, x.tipo, x.destino, x.oculto ? 'si' : '', x.pais, x.red, x.empresa, x.enviados, x.recibidos, x.parados, x.documentado === null ? '' : x.documentado ? 'si' : 'no'].map(q).join(','));
      baja(nombreArchivo('grabacion', 'csv'), 'text/csv;charset=utf-8', '\ufeff' + [COLS.join(',')].concat(filas).join('\r\n') + '\r\n');
    });
    $('retrato-json').addEventListener('click', () => {
      baja(nombreArchivo('grabacion', 'json'), 'application/json', JSON.stringify({
        formato: 'guardiana-grabacion/1', fecha: grab.fecha, zona_horaria: grab.zona, duracion_s: grab.dur, cada_s: CADA_MEDIDA,
        modo: grab.modo, quitado: ['usuario', 'rutas', 'direcciones IP', 'horas exactas', 'nombres privados'],
        columnas: COLS, filas: grab.filas.map((x) => [x.s, x.programa, x.tipo, x.destino, x.oculto, x.pais, x.red, x.empresa, x.enviados, x.recibidos, x.parados, x.documentado]),
      }, null, 1));
    });
    $('retrato-pdf').addEventListener('click', async () => {
      if (!ctx.pdf) return;
      const r = resumen(), m = textoModo(grab.modo), doc = ctx.pdf.pdfDocument(t('pd_retrato_pdf_titulo'));
      const may = (x) => x.charAt(0).toUpperCase() + x.slice(1);
      const fechaLarga = (() => { try { return new Date(grab.fecha + 'T12:00:00').toLocaleDateString(LOC, { day: 'numeric', month: 'long', year: 'numeric' }); } catch (_) { return grab.fecha; } })();
      doc.line('GUARDIANA · ' + t('pd_retrato_titulo').replace('{t}', mmss(grab.dur)), 16, true).gap(3)
        .line(t('pd_retrato_pdf_sub').replace('{fecha}', fechaLarga).replace('{zona}', grab.zona || '—'), 9, false, 0.45).regla().gap(10);
      doc.seccion(t('pd_retrato_modo'), m.titulo); m.lineas.forEach((l) => doc.line('· ' + l, 10));
      doc.seccion(t('pd_retrato_cifras'));
      doc.table([{ title: t('pd_retrato_que'), w: 0.6 }, { title: t('pd_retrato_cuanto'), w: 0.4 }], [
        [t('pd_enviado'), bytes(r.env)], [may(t('pd_recibido')), bytes(r.rec)], [t('pd_retrato_de_antivirus'), bytes(r.envAv)],
        [t('pd_col_programas'), String(r.programas.length)], [t('pd_retrato_destinos'), String(r.destinos.length)],
        [t('pd_col_pais'), String(r.paises.length)], [t('pd_col_red'), String(r.redes.length)], [t('pd_retrato_parados'), String(r.parados)],
      ]);
      doc.seccion(t('pd_col_programas'));
      doc.table([{ title: t('pd_col_programas'), w: 0.4 }, { title: t('pd_retrato_tipo'), w: 0.18 }, { title: t('pd_enviado'), w: 0.21 }, { title: may(t('pd_recibido')), w: 0.21 }],
        r.programas.map((x) => [x.k, t('pd_tipo_' + x.tipo), bytes(x.env), bytes(x.rec)]));
      if (r.paises.length) {
        doc.seccion(t('pd_col_pais'));
        doc.table([{ title: t('pd_col_pais'), w: 0.6 }, { title: t('pd_enviado'), w: 0.4 }], r.paises.map((x) => [nombrePais(x.k), bytes(x.env)]));
      }
      if (r.redes.length) {
        doc.seccion(t('pd_col_red'), t('pd_red_no_es'));
        doc.table([{ title: t('pd_col_red'), w: 0.6 }, { title: t('pd_enviado'), w: 0.4 }], r.redes.map((x) => [x.k, bytes(x.env)]));
      }
      doc.seccion(t('pd_retrato_destinos'), r.destinos.length > 40 ? t('pd_retrato_los_40') : '');
      doc.table([{ title: t('pd_retrato_destino'), w: 0.36 }, { title: t('pd_col_programas'), w: 0.2 }, { title: t('pd_col_pais'), w: 0.14 }, { title: t('pd_col_red'), w: 0.17 }, { title: t('pd_enviado'), w: 0.13 }],
        r.destinos.slice(0, 40).map((x) => [x.oculto ? t('pd_oculto_privacidad') : (x.destino || t('pd_sin_nombre')), x.programa, x.pais && x.pais[0] !== '@' ? nombrePais(x.pais) : '—', x.red || '—', x.parados && !x.enviados ? tn('pd_parados', x.parados) : bytes(x.enviados)]));
      doc.seccion(t('pd_retrato_como'));
      doc.line(t('pd_retrato_como_1'), 9, false, 0.3).gap(4).line(t('pd_retrato_como_2'), 9, false, 0.3);
      const nombre = nombreArchivo('retrato', 'pdf');
      const res = await ctx.pdf.savePdfFile(nombre, doc.bytes(t('pd_retrato_pie')));
      if (res !== 'cancelado') avisa(t('pd_retrato_guardado').replace('{nombre}', nombre));
    });
    $('retrato-cerrar').addEventListener('click', () => { $('retrato').hidden = true; });

    // ---------- Pointer, wheel and keys ---------------------------------------------------------
    // The flat map stays centred. The 3D globe can be turned; wheel, buttons and pinch change scale.
    // A click opens a destination backed by measurements and never cuts anything by itself.
    const ZOOM = { globo: [0.75, 7], mapa: [0.85, 10], plano: [1, 10] };
    const lim = (v, [a, b]) => Math.max(a, Math.min(b, v));
    ['pointermove', 'pointerdown', 'wheel', 'keydown'].forEach((ev) => window.addEventListener(ev, () => { st.ultimoGesto = performance.now(); }, { passive: true }));
    const enPlano = () => st.vista === 'destinos' && !T.ok;
    const zoomable = () => st.vista === 'destinos';
    // The first touch stops the globe's own sway where it is, without a jump.
    const toca = () => { if (!st.tocado) { st.dRot += T.osc || 0; st.tocado = true; } st.vuelo = null; st.inercia = null; };
    // Zoom by a factor f around the screen point (x, y): what is under it stays under it.
    const zoomEn = (f, x, y) => {
      if (!zoomable()) return;
      toca();
      if (enPlano()) {
        const e0 = escalaPlano(), z = lim(PLANO.z * f, ZOOM.plano); if (z === PLANO.z) return;
        const r = z / PLANO.z; PLANO.z = z;
        const s1 = e0.s * r, b = basePlano(s1);
        PLANO.px = x - (x - e0.ox) * r - b.ox; PLANO.py = y - (y - e0.oy) * r - b.oy;
        return;
      }
      if (T.m < 0.5) {
        const antes = lonLatBajo(x, y);
        T.zoomG = lim(T.zoomG * f, ZOOM.globo);
        // Turn the globe so the place under the pointer stays there (twice: the second step
        // corrects what the first left near the edges).
        for (let k = 0; antes && k < 2; k++) {
          camara(); const ahora = lonLatBajo(x, y); if (!ahora) break;
          let dl = ahora[0] - antes[0]; if (dl > 180) dl -= 360; if (dl < -180) dl += 360;
          st.dRot += dl * RAD; st.dTilt += (antes[1] - ahora[1]) * RAD;
        }
      } else {
        const d0 = T.dist; T.zoomM = lim(T.zoomM * f, ZOOM.mapa); camara();
        const c = ((st.Wc - st.W) / 2) * (2 * T.tanH / st.H), nx = (2 * x / st.Wc - 1) * T.tanH * T.asp, ny = (1 - 2 * y / st.H) * T.tanH;
        T.px += (c + nx) * (d0 - T.dist); T.py += ny * (d0 - T.dist);
        T.px = Math.max(-Math.PI, Math.min(Math.PI, T.px)); T.py = Math.max(-Math.PI / 2, Math.min(Math.PI / 2, T.py));
      }
    };
    const centroVista = () => ({ x: st.W / 2, y: st.H / 2 });
    const zoomBoton = (f) => { const c = centroVista(); zoomEn(f, c.x, c.y); };
    const zoomInicio = () => { st.dRot = 0; st.dTilt = 0; T.zoomG = 1; T.zoomM = 1; T.px = 0; T.py = 0; PLANO.z = 1; PLANO.px = 0; PLANO.py = 0; st.vuelo = null; };
    // «Ver de cerca»: fly to a place and zoom in on it.
    const vuela = (lon, lat) => {
      toca();
      if (enPlano()) { const z = Math.max(PLANO.z, 5); PLANO.z = z; const e = escalaPlano(), q = aPlano(lon, lat); PLANO.px += st.W / 2 - q.x; PLANO.py += st.H / 2 - q.y; return; }
      const yo = st.lugar;
      const dos = 2 * Math.PI, dif = ((((-(lon - yo.lon) * RAD - st.dRot) + Math.PI) % dos) + dos) % dos - Math.PI;
      st.vuelo = { t0: performance.now(), de: { r: st.dRot, ti: st.dTilt, zg: T.zoomG, zm: T.zoomM, px: T.px, py: T.py },
        a: { r: st.dRot + dif, ti: (lat - yo.lat * 0.8) * RAD, zg: Math.max(T.zoomG, 4), zm: Math.max(T.zoomM, 7), px: lon * RAD, py: lat * RAD } };
    };
    const avanzaVuelo = (ahora) => {
      const v = st.vuelo; if (!v) return;
      const u = Math.min(1, (ahora - v.t0) / 1400), e = u * u * (3 - 2 * u), m = (a, b) => a + (b - a) * e;
      st.dRot = m(v.de.r, v.a.r); st.dTilt = m(v.de.ti, v.a.ti); T.zoomG = Math.exp(m(Math.log(v.de.zg), Math.log(v.a.zg))); T.zoomM = Math.exp(m(Math.log(v.de.zm), Math.log(v.a.zm))); T.px = m(v.de.px, v.a.px); T.py = m(v.de.py, v.a.py);
      if (u >= 1) st.vuelo = null;
    };
    // Two fingers: the distance between them is the zoom.
    const dedos = new Map();
    let pellizco = null, arrastre = null;
    cv.addEventListener('pointermove', (e) => {
      const r = cv.getBoundingClientRect(); st.raton = { x: e.clientX - r.left, y: e.clientY - r.top };
      if (dedos.has(e.pointerId)) dedos.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (dedos.size === 2 && pellizco) {
        st.interactuando = true; st.inercia = null;
        const [a, b] = [...dedos.values()], d = Math.hypot(a.x - b.x, a.y - b.y);
        if (pellizco.d > 0 && d > 0) zoomBoton(d / pellizco.d);
        pellizco.d = d; return;
      }
      if (arrastre && arrastre.id === e.pointerId && enGlobo()) {
        const dx = e.clientX - arrastre.x, dy = e.clientY - arrastre.y;
        arrastre.total += Math.hypot(dx, dy);
        if (arrastre.total > 3) {
          toca(); st.interactuando = true;
          const sensibilidad = 2 * Math.PI / Math.max(420, Math.min(st.W || 800, st.H || 600));
          const dt = Math.max(8, Math.min(80, e.timeStamp - arrastre.t));
          st.dRot += dx * sensibilidad; st.dTilt = Math.max(-1.4, Math.min(1.4, st.dTilt + dy * sensibilidad * 0.82));
          arrastre.vr = dx * sensibilidad * 1000 / dt; arrastre.vt = dy * sensibilidad * 0.82 * 1000 / dt;
          arrastre.t = e.timeStamp;
          arrastre.x = e.clientX; arrastre.y = e.clientY; arrastre.movido = true;
          cv.classList.add('arrastrando'); tip.hidden = true;
        }
      }
    });
    cv.addEventListener('wheel', (e) => {
      if (!zoomable()) return; e.preventDefault();
      const dy = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY;
      zoomBoton(Math.exp(-Math.max(-200, Math.min(200, dy)) * 0.0022));
    }, { passive: false });
    cv.addEventListener('dblclick', () => zoomBoton(2));
    cv.addEventListener('pointerleave', () => { st.raton = null; tip.hidden = true; });
    cv.addEventListener('pointerdown', (e) => {
      dedos.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (dedos.size === 2) { arrastre = null; st.inercia = null; st.interactuando = true; cv.classList.remove('arrastrando'); const [a, b] = [...dedos.values()]; pellizco = { d: Math.hypot(a.x - b.x, a.y - b.y) }; return; }
      if (enGlobo() && e.button === 0) { st.inercia = null; arrastre = { id: e.pointerId, x: e.clientX, y: e.clientY, t: e.timeStamp, total: 0, vr: 0, vt: 0, movido: false }; try { cv.setPointerCapture(e.pointerId); } catch (_) {} }
    });
    const suelta = (e) => { dedos.delete(e.pointerId); if (dedos.size < 2) pellizco = null; st.interactuando = dedos.size > 0; cv.classList.remove('arrastrando'); try { if (cv.hasPointerCapture(e.pointerId)) cv.releasePointerCapture(e.pointerId); } catch (_) {} };
    cv.addEventListener('pointercancel', (e) => { if (arrastre && arrastre.id === e.pointerId) arrastre = null; suelta(e); });
    cv.addEventListener('pointerup', (e) => {
      const movido = !!(arrastre && arrastre.id === e.pointerId && arrastre.movido);
      if (movido && !reducido && dedos.size < 2) st.inercia = { vr: arrastre.vr, vt: arrastre.vt };
      if (arrastre && arrastre.id === e.pointerId) arrastre = null;
      suelta(e);
      if (dedos.size || movido) return;
      const r = cv.getBoundingClientRect();
      const hh = queHay(e.clientX - r.left, e.clientY - r.top);
      if (hh) abreDetalle(hh); else cierraDetalle();
    });
    cv.addEventListener('lostpointercapture', (e) => { if (arrastre && arrastre.id === e.pointerId) arrastre = null; suelta(e); });
    $('zoom-mas').addEventListener('click', () => zoomBoton(1.6));
    $('zoom-menos').addEventListener('click', () => zoomBoton(1 / 1.6));
    $('zoom-0').addEventListener('click', zoomInicio);
    // Two views (13 Oct 2026): Destinos, where each server is on the Earth (the flat map by
    // default, the globe as an option), and Programas, how much each program sends and where to.
    const enGlobo = () => st.vista === 'destinos' && st.proy === 'globo' && T.ok;
    const ayuda = () => t(st.vista === 'programas' ? 'pd_ayuda_programas' : !T.ok ? 'pd_ayuda_plano' : st.proy === 'globo' ? 'pd_ayuda_globo' : 'pd_ayuda_mapa');
    const VISTAS = ['destinos', 'programas'];
    const muestraAyuda = () => {
      $('ayuda-vista').textContent = ayuda();
      // The help says what to do and then gets out of the way.
      $('ayuda-vista').classList.remove('oculta'); clearTimeout(muestraAyuda.t); muestraAyuda.t = setTimeout(() => $('ayuda-vista').classList.add('oculta'), 12000);
    };
    const pulsado = (sel, attr, v) => document.querySelectorAll(sel).forEach((b) => b.setAttribute('aria-pressed', String(b.dataset[attr] === v)));
    const guarda = (k, v) => { try { localStorage.setItem(k, v); } catch (_) {} };
    const ponVista = (v) => {
      if (v !== st.vista) { st.part = []; st.estallidos = []; }
      st.vista = v;
      pulsado('.pestanas button', 'vista', v);
      $('zoom').hidden = v !== 'destinos';
      expl.hidden = v !== 'programas'; stage.dataset.vista = v; pintaExplorador();
      muestraAyuda();
      guarda('guardiana_pd_vista', v);
    };
    const ponProy = (v) => { st.proy = v; stage.dataset.proy = v; pulsado('#proy button', 'proy', v); if (st.vista === 'destinos') muestraAyuda(); guarda('guardiana_pd_proy', v); };
    const ponFiltro = (v) => {
      st.filtro = v; pulsado('#filtro button', 'filtro', v);
      recalcula(); st.part = st.part.filter((q) => VISIBLES.includes(q.d));
      pintaHud(); guarda('guardiana_pd_filtro', v);
    };
    document.querySelectorAll('.pestanas button').forEach((b) => b.addEventListener('click', () => ponVista(b.dataset.vista)));
    document.querySelectorAll('#proy button').forEach((b) => b.addEventListener('click', () => ponProy(b.dataset.proy)));
    document.querySelectorAll('#filtro button').forEach((b) => b.addEventListener('click', () => ponFiltro(b.dataset.filtro)));
    const pantalla = (on) => { stage.classList.toggle('full', on); $('pantalla').textContent = t(on ? 'pd_cerrar' : 'pd_pantalla_completa'); requestAnimationFrame(ajusta); };
    $('pantalla').addEventListener('click', () => { if (document.fullscreenElement) { document.exitFullscreen().catch(() => {}); return; } if (stage.requestFullscreen) stage.requestFullscreen().catch(() => pantalla(!stage.classList.contains('full'))); else pantalla(!stage.classList.contains('full')); });
    document.addEventListener('fullscreenchange', () => { $('pantalla').textContent = t(document.fullscreenElement ? 'pd_cerrar' : 'pd_pantalla_completa'); requestAnimationFrame(ajusta); });
    document.addEventListener('keydown', (e) => {
      if (/select|input|textarea/i.test(document.activeElement.tagName) || e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key === '1' || e.key === '2') ponVista(VISTAS[+e.key - 1]);
      else if (e.key === '+' || e.key === '=') zoomBoton(1.6);
      else if (e.key === '-') zoomBoton(1 / 1.6);
      else if (e.key === '0') zoomInicio();
      else if (e.key === 'Escape') cierraDetalle();
    });
    // The time zone is only a visual starting reference: it never proves a city. A requested
    // measurement takes several fresh samples and keeps the best one. The person can also confirm
    // a city when Windows, Microsoft or Google returns an imprecise or stale point. Everything
    // stays in this browser and GUARDIANA sends it nowhere.
    const etiquetaYo = document.querySelector('.lugar-yo [data-t="pd_tu_pais"]');
    // The VPN chip (15 Oct 2026). While a VPN is up, the city a network measurement gives is the
    // VPN's exit, so the chip names the VPN and the hint says how to place the computer right.
    let VPN = [];
    const pintaVpn = (lista) => {
      VPN = Array.isArray(lista) ? lista : [];
      const el = $('vpn');
      if (!VPN.length) { el.hidden = true; return; }
      el.hidden = false;
      el.textContent = t('pd_vpn_saliendo').replace('{n}', VPN.join(', '));
      el.title = t('pd_vpn_explica');
      if (st.lugar.fuente === 'zona' || st.lugar.fuente === 'pais') { if (!pintaVpn.avisado) { pintaVpn.avisado = true; avisa(t('pd_vpn_aviso').replace('{n}', VPN.join(', ')), [{ texto: t('pd_corregir'), hacer: abreUbicacion }]); } }
    };
    const pintaYo = () => {
      const aproximada = st.lugar.fuente === 'zona' || st.lugar.fuente === 'pais';
      etiquetaYo.textContent = t(aproximada ? 'pd_ubicacion_referencia' : 'pd_tu_pais');
      $('yo').textContent = nombrePais(st.yo) + (st.lugar.ciudad && !aproximada ? ' · ' + st.lugar.ciudad : '');
    };
    pintaYo();
    let CIUDADES = null;
    const ciudades = async () => {
      if (CIUDADES) return CIUDADES;
      const r = await fetch((ctx.tierra || '/static/tierra/') + 'ciudades.tsv');
      if (!r.ok) throw new Error(String(r.status));
      CIUDADES = (await r.text()).split('\n').filter((l) => l && l[0] !== '#').map((l) => { const [n, cc, adm, la, lo] = l.split('\t'); return { n, cc, adm, la: +la, lo: +lo }; });
      return CIUDADES;
    };
    const cercana = (lista, lat, lon) => {
      let mejor = null, dm = Infinity;
      const cl = Math.cos(lat * RAD);
      for (const c of lista) { const dx = (c.lo - lon) * cl, dy = c.la - lat, d = dx * dx + dy * dy; if (d < dm) { dm = d; mejor = c; } }
      return mejor ? { c: mejor, km: Math.sqrt(dm) * 111 } : null;
    };
    const ponLugarEquipo = (g, guardar) => {
      st.lugar = { lon: g.lon, lat: g.lat, fuente: g.fuente || 'equipo', ciudad: g.ciudad, precision: g.precision };
      if (g.cc && centros[g.cc]) st.yo = g.cc;
      pintaYo();
      if (guardar) { try { localStorage.setItem('guardiana_pd_lugar', JSON.stringify(Object.assign({ cuando: Date.now() }, g))); } catch (_) {} }
    };
    const procesaPosicion = async (pos, callado) => {
      const precision = Number.isFinite(pos.coords.accuracy) ? Math.max(0, pos.coords.accuracy) : Infinity;
      const lat = Math.round(pos.coords.latitude * 100) / 100, lon = Math.round(pos.coords.longitude * 100) / 100;
      let g = { lat, lon, ciudad: '', cc: '', precision, fuente: 'equipo' };
      try {
        const m = cercana(await ciudades(), lat, lon);
        if (m && m.km < 35 && precision <= 25000) g = { lat, lon, ciudad: m.c.n + (m.c.adm ? ', ' + m.c.adm : ''), cc: m.c.cc, precision, fuente: 'equipo' };
        else g.cc = paisEn(lon, lat) || '';
      } catch (_) { g.cc = paisEn(lon, lat) || ''; }
      if (VPN.length && precision > 1000) {
        if (!callado) avisa(t('pd_vpn_medida').replace('{n}', VPN.join(', ')), [{ texto: t('pd_corregir'), hacer: abreUbicacion }]);
        return;
      }
      if (precision > 25000) {
        if (!callado) avisa(t('pd_precisar_impreciso').replace('{km}', String(Math.max(1, Math.round(precision / 1000)))), [{ texto: t('pd_corregir'), hacer: abreUbicacion }]);
        return;
      }
      ponLugarEquipo(g, true);
      if (!callado) avisa(t('pd_precisar_hecho').replace('{lugar}', g.ciudad || nombrePais(g.cc || st.yo)).replace('{precision}', precision < 1000 ? Math.max(1, Math.round(precision)) + ' m' : Math.max(1, Math.round(precision / 1000)) + ' km'));
    };
    let medicion = null;
    const precisar = (callado) => {
      if (!navigator.geolocation) { if (!callado) avisa(t('pd_precisar_no')); return; }
      if (!callado) avisa(t('pd_precisar_pide'));
      if (medicion !== null) navigator.geolocation.clearWatch(medicion);
      let mejor = null, terminado = false;
      const acaba = (err) => {
        if (terminado) return; terminado = true;
        if (medicion !== null) navigator.geolocation.clearWatch(medicion); medicion = null; clearTimeout(acaba.reloj);
        if (mejor) procesaPosicion(mejor, callado);
        else if (!callado) avisa(t(err && err.code === 1 ? 'pd_precisar_denegado' : 'pd_precisar_fallo'), [{ texto: t('pd_corregir'), hacer: abreUbicacion }]);
      };
      medicion = navigator.geolocation.watchPosition((pos) => {
        if (!mejor || (Number.isFinite(pos.coords.accuracy) && pos.coords.accuracy < mejor.coords.accuracy)) mejor = pos;
        if (mejor && mejor.coords.accuracy <= 100) acaba();
      }, acaba, { enableHighAccuracy: true, timeout: 20000, maximumAge: 0 });
      acaba.reloj = setTimeout(() => acaba(), 15000);
    };
    const modalUbicacion = $('ubicacion'), buscaUbicacion = $('ubicacion-busca'), resultadosUbicacion = $('ubicacion-resultados');
    buscaUbicacion.placeholder = t('pd_corregir_placeholder');
    const cierraUbicacion = () => { modalUbicacion.hidden = true; resultadosUbicacion.innerHTML = ''; };
    const eligeCiudad = (c) => {
      ponLugarEquipo({ lat: Math.round(c.la * 100) / 100, lon: Math.round(c.lo * 100) / 100, ciudad: c.n + (c.adm ? ', ' + c.adm : ''), cc: c.cc, fuente: 'manual', precision: null }, true);
      cierraUbicacion(); avisa(t('pd_corregir_hecho').replace('{lugar}', st.lugar.ciudad + ', ' + nombrePais(st.yo)));
    };
    const pintaCiudades = async () => {
      const q = buscaUbicacion.value.trim().toLocaleLowerCase(LOC); resultadosUbicacion.innerHTML = '';
      if (q.length < 2) return;
      const lista = (await ciudades()).filter((c) => (c.n + ' ' + c.adm + ' ' + nombrePais(c.cc)).toLocaleLowerCase(LOC).includes(q)).slice(0, 8);
      lista.forEach((c) => { const b = document.createElement('button'); b.type = 'button'; b.setAttribute('role', 'option'); b.innerHTML = `${esc(c.n)}${c.adm ? ` · ${esc(c.adm)}` : ''}<small>${esc(nombrePais(c.cc))}</small>`; b.addEventListener('click', () => eligeCiudad(c)); resultadosUbicacion.appendChild(b); });
      if (!lista.length) resultadosUbicacion.innerHTML = `<p>${esc(t('pd_corregir_sin_resultados'))}</p>`;
    };
    function abreUbicacion() { modalUbicacion.hidden = false; buscaUbicacion.value = ''; resultadosUbicacion.innerHTML = ''; setTimeout(() => buscaUbicacion.focus(), 0); }
    if (soloMirar) $('precisar').hidden = true;
    else {
      $('precisar').title = t('pd_precisar_que');
      $('precisar').addEventListener('click', () => precisar(false));
      // What this browser measured before (within 30 days) is used at once; if the permission is
      // still granted, it is measured again quietly, so a laptop that travelled follows.
      try {
        const g = JSON.parse(localStorage.getItem('guardiana_pd_lugar') || 'null');
        if (g && Date.now() - g.cuando < 30 * 86400000 && Number.isFinite(g.lat) && Number.isFinite(g.lon) && (g.fuente === 'manual' || Number.isFinite(g.precision))) ponLugarEquipo(g, false);
      } catch (_) {}
      try { navigator.permissions.query({ name: 'geolocation' }).then((p) => { if (p.state === 'granted') precisar(true); }).catch(() => {}); } catch (_) {}
    }
    if (soloMirar) $('corregir-lugar').hidden = true;
    else {
      $('corregir-lugar').addEventListener('click', abreUbicacion);
      // Placed by the time zone only (its capital): say so once, with the way to fix it, instead
      // of letting a person in Cartagena see their computer in Bogotá (15 Oct 2026).
      try {
        if ((st.lugar.fuente === 'zona' || st.lugar.fuente === 'pais') && !localStorage.getItem('guardiana_pd_lugar_avisado')) {
          localStorage.setItem('guardiana_pd_lugar_avisado', '1');
          setTimeout(() => { if (st.lugar.fuente === 'zona' || st.lugar.fuente === 'pais') avisa(t('pd_lugar_zona_aviso').replace('{lugar}', [st.lugar.ciudad, nombrePais(st.yo)].filter(Boolean).join(', ')), [{ texto: t('pd_corregir'), hacer: abreUbicacion }, { texto: t('pd_precisar'), hacer: () => precisar(false) }]); }, 2500);
        }
      } catch (_) {}
    }
    buscaUbicacion.addEventListener('input', pintaCiudades);
    $('ubicacion-cerrar').addEventListener('click', cierraUbicacion);

    // ---------- The loop ----------
    let antes = performance.now(), ultimaTele = 0, escenaVisible = true;
    if ('IntersectionObserver' in window) new IntersectionObserver((e) => { escenaVisible = !!(e[0] && e[0].isIntersecting); }, { threshold: 0.01 }).observe(stage);
    const bucle = (ahora) => {
      requestAnimationFrame(bucle);
      if (document.hidden || !escenaVisible) { antes = ahora; return; }
      // To keep it open for hours: 30 frames a second, 20 after two quiet minutes and 60 during
      // the short guided flight opened by «View close up».
      const quieto = ahora - st.ultimoGesto > 120000;
      const activo = !!(st.vuelo || st.inercia || st.interactuando || arrastre || pellizco);
      if (ahora - antes < (activo || st.raton ? 16 : quieto ? 40 : 20)) return;
      const dt = Math.min(0.06, (ahora - antes) / 1000); antes = ahora;
      try {
        if (st.inercia && !reducido) {
          st.dRot += st.inercia.vr * dt; st.dTilt = Math.max(-1.4, Math.min(1.4, st.dTilt + st.inercia.vt * dt));
          const freno = Math.exp(-4.5 * dt); st.inercia.vr *= freno; st.inercia.vt *= freno;
          if (Math.hypot(st.inercia.vr, st.inercia.vt) < 0.004) st.inercia = null;
        }
        if (ahora - ultimaTele > 1000) { ultimaTele = ahora; telemetria(); pintaDatos(); }
        cx.setTransform(st.dpr, 0, 0, st.dpr, 0, 0);
        st.dianas = [];
        st.W = datosAbierto && st.Wc > 700 ? Math.max(320, st.Wc - panelDatos.offsetWidth - 20) : st.Wc;
        // The notices on the picture are centred on what is drawn, not under the side panel.
        lienzo.classList.toggle('con-datos', st.W !== st.Wc);
        fondo();
        if (enTierra()) {
          const obj = st.proy === 'globo' ? 0 : 1;
          if (obj !== T.mObj) { T.tw = { desde: T.m, t0: ahora }; T.mObj = obj; }
          if (T.tw) { const u = Math.min(1, (ahora - T.tw.t0) / 2200), e = u * u * (3 - 2 * u); T.m = T.tw.desde + (T.mObj - T.tw.desde) * e; if (u >= 1) T.tw = null; }
          glc.style.visibility = 'visible';
          avanzaVuelo(ahora); camara(); pintaTierra(); dibujaTierra(dt);
        } else {
          glc.style.visibility = 'hidden';
          if (st.vista !== 'programas') dibujaPlano(dt);
        }
        if (st.raton) actualizaTip();
      } catch (e) { if (!bucle.dicho) { bucle.dicho = true; console.error(e); } }
    };

    // ---------- Start ----------
    // What the person chose last time; the five views of 1.0.22 map onto the two that stay.
    const leido = (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } };
    const vAntes = leido('guardiana_pd_vista');
    ponProy('globo');
    const f0 = leido('guardiana_pd_filtro');
    st.filtro = ['todo', 'datos', 'av'].includes(f0) ? f0 : 'todo'; pulsado('#filtro button', 'filtro', st.filtro);
    ponVista(vAntes === 'programas' || vAntes === 'dentro' ? 'programas' : 'destinos');
    ajusta();
    let datosGuardado = null;
    try { datosGuardado = localStorage.getItem('guardiana_pd_datos'); } catch (_) {}
    ponDatos(datosGuardado === null ? st.Wc >= 1000 : datosGuardado === '1');
    await cargar();
    ctx.cadaTanto(cargar, 5000);
    requestAnimationFrame(bucle);
  }
})();
