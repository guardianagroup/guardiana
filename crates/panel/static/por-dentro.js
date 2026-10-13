// GUARDIANA por dentro (1.0.22). The owner asked for a view where you see «cómo trabaja GUARDIANA
// dentro del ordenador»: the antivirus, the browser and the programs that talk most, their data
// going out to countries and companies, what GUARDIANA lets pass and what it stops, live, «un
// espectáculo visual» someone can keep open for hours. Designed with Grok's help (12–13 Oct 2026).
//
// No third-party code (brief §8), and nothing from the internet: WebGL2 written here, the Earth's
// pictures (NASA, public domain) served by the panel itself, the world's countries from Natural
// Earth (public domain) in a 1.5° grid below. The light on the globe is the sun's of this very
// moment, computed here. Every figure is a measurement of this computer (`/api/por-dentro`); when
// there is none yet, the page offers a demonstration, labelled as such on screen.
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
    const dibujable = (d) => !!centros[d.pais];
    // What the person's computer sent, in 10-second buckets: 30 of them are the deck's «last five
    // minutes», whatever the rhythm of the data (Windows is read every 10 s; the demo beats faster).
    const CUBO_MS = 10000, N_CUBOS = 30, CADA_MEDIDA = 10;
    const st = {
      vista: 'globo', yo: paisDelEquipo(), W: 0, Wc: 0, H: 0, dpr: 1, t0: performance.now(),
      arrastre: null, dRot: 0, dTilt: 0, part: [], estallidos: [], hover: null, raton: null, dianas: [],
      serie: new Array(N_CUBOS).fill(0), serieP: {}, cuboT0: Date.now(), eventos: [], ahora: [0, 0, 0], intervalo: 5,
      demo: false, sinSesion: false, datos: null, abierto: Date.now(), primeraCarga: true, ultimoGesto: performance.now(),
    };

    st.lugar = lugarDe(st.yo);
    const avanzaCubos = () => {
      const n = Math.min(N_CUBOS, Math.floor((Date.now() - st.cuboT0) / CUBO_MS));
      if (n <= 0) return;
      for (let k = 0; k < n; k++) { st.serie.push(0); st.serie.shift(); Object.values(st.serieP).forEach((v) => { v.push(0); v.shift(); }); }
      st.cuboT0 = n === N_CUBOS ? Date.now() : st.cuboT0 + n * CUBO_MS;
    };

    // ---------- Stars and nebulae (fixed, made once) ----------
    let semilla = 7;
    const azar = () => { semilla = (semilla * 16807) % 2147483647; return semilla / 2147483647; };
    const gauss = () => { let u = 0, v = 0; while (!u) u = azar(); while (!v) v = azar(); return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v); };
    const estrellas = Array.from({ length: 320 }, () => ({ x: azar(), y: azar(), r: azar() * 1.2 + 0.2, f: azar() * 6.28 }));
    const nubeDe = {};
    let paisesUsados = [];
    const tono = (c) => { let h = 0; for (const ch of c) h = (h * 31 + ch.charCodeAt(0)) % 360; return h; };

    // What is drawn: the destinations that move most (and every one that is cut), so a busy
    // computer stays readable. The side panel lists them all.
    const MAX_DIBUJO = 60;
    let VISIBLES = [];
    const recalcula = () => {
      const orden = DEST.filter(dibujable).sort((a, b) => (b.sub + b.baj) - (a.sub + a.baj) || b.bloq - a.bloq);
      const cortados = orden.filter((d) => bloqueado(d) || d.bloq).slice(0, 20);
      const resto = orden.filter((d) => !cortados.includes(d)).slice(0, MAX_DIBUJO - cortados.length);
      VISIBLES = resto.concat(cortados);
      VISIBLES.forEach((d, i) => { d.i = i; });
      paisesUsados = [...new Set(VISIBLES.map((d) => d.pais))];
      paisesUsados.forEach((c) => { if (!nubeDe[c]) nubeDe[c] = Array.from({ length: 260 }, () => ({ x: gauss() * 0.55, y: gauss() * 0.4, r: azar() * 1.3 + 0.3, a: azar() * 0.6 + 0.15 })); });
      PROD.forEach((p, k) => { if (!st.serieP[p.id]) st.serieP[p.id] = new Array(N_CUBOS).fill(0); p.k = k; });
    };
    const nuevoProducto = (p) => {
      const tipo = p.tipo || 'programa', usados = PROD.filter((x) => x.tipo === tipo).length, pal = COLORES[tipo] || COLORES.programa;
      const o = { id: p.id, nombre: p.id, tipo, procesos: p.procesos || [], firma: p.firma || '', cortable: !!p.cortable, cortado: !!p.cortado, color: pal[usados % pal.length], sub: p.enviados || 0, baj: p.recibidos || 0 };
      PROD.push(o); return o;
    };
    const claveDe = (d) => [d.programa, d.destino, d.pais, d.red].join('|');
    const nombreDe = (d) => d.destino || (d.red && d.red[0] !== '@' ? t('pd_sin_nombre_red').replace('{red}', d.red) : t('pd_sin_nombre'));

    // ---------- Demonstration (no measurements yet, or no session) ----------
    const DEMO_PROD = [
      { id: t('pd_demo_av_a'), tipo: 'antivirus', procesos: ['avservicio.exe'], firma: 'Ejemplo A S. L.', cortable: true },
      { id: t('pd_demo_av_b'), tipo: 'antivirus', procesos: ['bguardia.exe'], firma: 'Ejemplo B Inc.', cortable: true },
      { id: t('pd_demo_nav'), tipo: 'navegador', procesos: ['navegador.exe'], firma: 'Ejemplo N Ltd.', cortable: false },
    ];
    const DEMO_DEST = [
      [0, 'nube.antivirus-a.example', 'US', 'Microsoft', 8075, true, 'nube', 5], [0, 'actualiza.antivirus-a.example', 'NL', 'Akamai', 20940, true, 'actualizaciones', 3],
      [0, 'telemetria.antivirus-a.example', 'IE', 'Microsoft', 8075, true, 'telemetria', 2], [0, 'licencia.antivirus-a.example', 'DE', 'Amazon', 16509, true, 'licencia', 1],
      [0, 'mapas.antivirus-a.example', 'BR', 'Google', 15169, false, null, 1], [1, 'api.antivirus-b.example', 'US', 'Amazon', 16509, false, null, 3],
      [1, 'muestras.antivirus-b.example', 'CA', 'OVH', 16276, true, 'muestras', 2], [1, 'anuncios.publicidad.example', 'US', 'Google', 15169, false, null, 2, 'cortado'],
      [1, 'cdn.antivirus-b.example', 'SG', 'Cloudflare', 13335, true, 'actualizaciones', 1], [1, '', 'JP', '', 0, false, null, 1],
      [2, 'periodico.example', 'ES', 'Cloudflare', 13335, null, null, 3], [2, 'rastreo.anuncios.example', 'US', 'Google', 15169, null, null, 3, 'cortado'],
      [2, 'pixel.redsocial.example', 'IE', 'Meta', 32934, null, null, 2, 'cortado'], [2, 'video.example', 'US', 'Akamai', 20940, null, null, 2],
    ];
    const DEMO_NUEVOS = [[1, 'informes.antivirus-b.example', 'PL', 'OVH', 16276, false, null, 1, null, 40], [0, 'reputacion.antivirus-a.example', 'FR', 'Microsoft', 8075, true, 'nube', 2, null, 95], [2, 'fotos.example', 'BR', 'Amazon', 16509, null, null, 1, null, 150]];
    const demoDestino = ([pi, host, pais, red, asn, doc, uso, peso, estado]) => {
      const p = PROD[pi];
      return { k: [p.id, host, pais, red].join('|'), p: p.id, host: host || t('pd_sin_nombre'), destino: host, pais, red, asn, proto: /video|periodico|rastreo|pixel|fotos|cdn/.test(host) && red !== 'Akamai' ? 'udp' : 'tcp', puerto: /actualiza/.test(host) ? 80 : 443, vias: [], sub: 0, baj: 0, paq: 0, noche: 0, primera: Date.now() - 3600e3, ultima: Date.now(), doc, uso, consultas: 0, bloq: 0, regla: estado === 'cortado' ? -1 : null, cortado: estado === 'cortado', sede: false, peso, ruta: [], tg: 0.3, x: 0, y: 0, vis: true, ema: 0 };
    };
    const empiezaDemo = () => {
      st.demo = true; PROD.length = 0; DEST.length = 0; porClave.clear();
      DEMO_PROD.forEach(nuevoProducto);
      DEMO_DEST.forEach((x) => { const d = demoDestino(x); DEST.push(d); porClave.set(d.k, d); });
      st.demoT0 = Date.now(); recalcula();
      $('chip-modo').textContent = t('pd_chip_demo'); $('chip-modo').hidden = false;
      $('vacio').hidden = true;
      pintaHud(); pintaDatos();
    };
    // One beat of the demonstration: what a computer like this one would measure in a second.
    const latidoDemo = () => {
      const seg = (Date.now() - st.demoT0) / 1000;
      DEMO_NUEVOS.filter((n) => !n.hecho && seg >= n[9]).forEach((n) => { n.hecho = true; const d = demoDestino(n); d.primera = Date.now(); DEST.push(d); porClave.set(d.k, d); recalcula(); evento(prod(d.p).nombre, d.host, t('pd_ev_nuevo').replace('{pais}', d.pais).replace('{red}', d.red), 'n'); avisa(t('pd_aviso_nuevo').replace('{prog}', prod(d.p).nombre).replace('{nombre}', d.host).replace('{pais}', nombrePais(d.pais))); });
      const rec = [];
      DEST.forEach((d) => { if (azar() > d.peso / 6.5) return; if (bloqueado(d)) { rec.push({ d, env: 0, recb: 0, paq: 0, parada: 1 }); return; } const s = Math.round(Math.pow(azar(), 2) * 38000 * d.peso + 300), b = Math.round(azar() * 9000 * d.peso + 200); rec.push({ d, env: s, recb: b, paq: Math.round(s / 1200 + b / 1400) + 2, parada: 0 }); });
      aplicaDeltas(rec, 0.9);
    };

    // ---------- Measurements: the panel's answer, and what changed since the last one ----------
    const filasVistas = new Map();
    let ultimaCarga = 0;
    const cargar = async () => {
      if (st.demo) return;
      let r;
      try { r = await api('/api/por-dentro'); } catch (e) {
        if (st.primeraCarga) { st.primeraCarga = false; ofrecerDemo(String(e && e.message || '').includes('sesión') ? t('pd_vacio_sin_sesion') : t('pd_vacio')); }
        return;
      }
      const ahora = performance.now(), intervalo = ultimaCarga ? Math.max(1, (ahora - ultimaCarga) / 1000) : 5;
      ultimaCarga = ahora; st.intervalo = intervalo; st.datos = r;
      r.programas.forEach((p) => {
        let o = PROD.find((x) => x.id === p.id);
        if (!o) o = nuevoProducto(p);
        Object.assign(o, { tipo: p.tipo, procesos: p.procesos, firma: p.firma, cortable: p.cortable, cortado: p.cortado, sub: p.enviados, baj: p.recibidos });
      });
      const rec = [], vistos = new Set();
      r.destinos.forEach((x) => {
        const k = claveDe(x); vistos.add(k);
        let d = porClave.get(k);
        const esNuevo = !d;
        if (!d) { d = { k, p: x.programa, ruta: [], tg: 0.3, x: 0, y: 0, vis: true, ema: 0, bloq: x.paradas }; porClave.set(k, d); DEST.push(d); }
        const antes = d.bloq || 0;
        Object.assign(d, { host: nombreDe(x), destino: x.destino, pais: x.pais, red: x.red, asn: x.asn, proto: x.proto, puerto: x.puerto, vias: x.vias, sub: x.enviados, baj: x.recibidos, paq: x.paquetes, noche: x.noche, primera: x.primera, ultima: x.ultima, doc: x.documentado, uso: x.uso, consultas: x.consultas, bloq: x.paradas, regla: x.regla ?? null, cortado: x.regla !== null && x.regla !== undefined, sede: x.sede });
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
          if (d0 && f.enviados > 0) st.eventos.push({ h: hora(f.minuto), prog: prod(d0.p).nombre, host: d0.host, que: `${bytes(f.enviados)} ↑ · ${bytes(f.recibidos)} ↓ · ${f.pais || '—'} · ${f.red || '—'}`, cls: 'o' });
          return;
        } else { de = f.enviados; dr = f.recibidos; dp = f.paquetes; }
        if (de <= 0 && dr <= 0) return;
        const d = porClave.get([f.programa, f.destino, f.pais, f.red].join('|'));
        if (d) rec.push({ d, env: Math.max(0, de), recb: Math.max(0, dr), paq: Math.max(0, dp), parada: 0 });
      });
      for (const [k, v] of filasVistas) if (v.m < r.ahora - 5 * 60000) filasVistas.delete(k);
      recalcula();
      aplicaDeltas(rec, Math.max(intervalo, CADA_MEDIDA), false);
      st.primeraCarga = false;
      $('vacio').hidden = DEST.length > 0;
      if (!DEST.length) ofrecerDemo(t('pd_vacio'));
      $('vigilancia-off').hidden = r.vigilando !== false;
      pintaHud();
    };
    // What arrived in this beat becomes particles along each route, spread over the beat so the
    // flow looks continuous, and lines in the log.
    const aplicaDeltas = (rec, segundos, suma = true) => {
      let env = 0, recb = 0, paq = 0;
      const porProd = {}, grupo = { datos: [0, 0], av: [0, 0] };
      DEST.forEach((d) => { d.ema = (d.ema || 0) * 0.7; });
      rec.forEach(({ d, env: e, recb: b, paq: q, parada }) => {
        const p = prod(d.p);
        if (parada) {
          if (suma) d.bloq += parada;
          for (let k = 0; k < Math.min(3, parada); k++) nuevo(d, 'out', true, 2000, azar() * segundos * 900);
          evento(p.nombre, d.host, t('pd_ev_parado'), 'c');
          return;
        }
        if (suma) { d.sub += e; d.baj += b; d.paq += q; p.sub += e; p.baj += b; if (new Date().getHours() < 6) d.noche += e; }
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
      st.eventos.unshift({ h: hora(Date.now()), prog, host, que, cls });
      if (st.eventos.length > 40) st.eventos.length = 40;
    };
    const ofrecerDemo = (texto) => {
      if (st.demo) return;
      $('vacio-texto').textContent = texto;
      $('vacio').hidden = false;
    };

    // ---------- Size ----------
    let fondoListo = false;
    const ajusta = () => {
      const r = lienzo.getBoundingClientRect();
      st.dpr = Math.min(window.devicePixelRatio || 1, 2);
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

    // --- Nebula ---
    const layoutNebulosa = () => {
      const c = { x: st.W / 2, y: st.H / 2 }, R0 = Math.min(st.W, st.H) * 0.16;
      const prods = posProductos(c, R0 * 0.48);
      const ax = st.W < 700 ? st.W * 0.3 : Math.min(st.W * 0.36, st.W / 2 - 150), ay = st.H * 0.33;
      const cent = {};
      paisesUsados.forEach((pc, k) => { const a = -Math.PI / 2 + (k + 0.5) * (2 * Math.PI / paisesUsados.length); cent[pc] = { x: c.x + Math.cos(a) * ax, y: c.y + Math.sin(a) * ay, a }; });
      const porPais = {};
      VISIBLES.forEach((d) => { (porPais[d.pais] = porPais[d.pais] || []).push(d); });
      VISIBLES.forEach((d) => {
        const grupo = porPais[d.pais], k = grupo.indexOf(d), n = grupo.length, ce = cent[d.pais];
        const paso = Math.min(22, (st.H * 0.3) / Math.max(1, n));
        d.x = ce.x + (k % 2 ? 10 : -10) * (n > 1 ? 1 : 0); d.y = ce.y + (k - (n - 1) / 2) * paso;
        const pr = posDe(prods, d, c);
        const ang = Math.atan2(d.y - c.y, d.x - c.x);
        const g = { x: c.x + Math.cos(ang) * R0, y: c.y + Math.sin(ang) * R0 };
        const c1 = { x: (pr.x + g.x) / 2 + Math.sin(ang) * R0 * 0.25, y: (pr.y + g.y) / 2 - Math.cos(ang) * R0 * 0.25 };
        const s = lado(d) || 0.4;
        const c2 = { x: g.x + Math.cos(ang) * R0 * 0.9 + Math.sin(ang) * 40 * s, y: g.y + Math.sin(ang) * R0 * 0.9 - Math.cos(ang) * 40 * s };
        d.ruta = tramo((u) => bez2(pr, c1, g, u), 14).concat(tramo((u) => bez2(g, c2, d, u), N - 14).slice(1));
        d.tg = 14 / N; d.vis = true;
      });
      return { c, R0, prods, cent, porPais };
    };

    // --- The computer, inside ---
    const layoutDentro = () => {
      const W = st.W, H = st.H, top = 70, bot = H - 48;
      const col = { prod: W * 0.1, puerta: W * 0.36, red: W * 0.5, inet: W * 0.66, dest: W * 0.8 };
      const caja = { x: W * 0.035, y: 30, w: W * 0.515, h: H - 66 };
      const ps = prodsVisibles();
      const prods = ps.map((p, k) => ({ p, x: col.prod, y: top + (bot - top) * (k + 1) / (ps.length + 1) }));
      const filas = Math.max(6, Math.min(24, Math.floor((bot - top) / 22)));
      const orden = VISIBLES.slice().sort((a, b) => (b.sub + b.baj) - (a.sub + a.baj)).slice(0, filas).sort((a, b) => a.pais.localeCompare(b.pais) || a.p.localeCompare(b.p));
      VISIBLES.forEach((d) => { d.vis = false; d.ruta = []; });
      const fila = (k) => top + 8 + (bot - top - 16) * (orden.length === 1 ? 0.5 : k / (orden.length - 1));
      const nic = { x: col.red, y: H / 2 };
      orden.forEach((d, k) => {
        const y = fila(k), pr = posDe(prods, d, { x: col.prod, y });
        d.x = col.dest; d.y = y; d.puerta = { x: col.puerta, y };
        const pts = [pr, { x: col.puerta, y }, nic, { x: col.inet, y: H / 2 + (y - H / 2) * 0.45 }, { x: col.dest, y }];
        const ruta = [];
        for (let i = 0; i < pts.length - 1; i++) {
          const p0 = pts[Math.max(0, i - 1)], p1 = pts[i], p2 = pts[i + 1], p3 = pts[Math.min(pts.length - 1, i + 2)];
          for (let s = i ? 1 : 0; s <= 7; s++) { const u = s / 7, u2 = u * u, u3 = u2 * u;
            ruta.push({ x: 0.5 * (2 * p1.x + (-p0.x + p2.x) * u + (2 * p0.x - 5 * p1.x + 4 * p2.x - p3.x) * u2 + (-p0.x + 3 * p1.x - 3 * p2.x + p3.x) * u3),
                        y: 0.5 * (2 * p1.y + (-p0.y + p2.y) * u + (2 * p0.y - 5 * p1.y + 4 * p2.y - p3.y) * u2 + (-p0.y + 3 * p1.y - 3 * p2.y + p3.y) * u3) }); }
        }
        d.ruta = ruta; d.tg = 7 / (ruta.length - 1); d.vis = true;
      });
      return { caja, col, prods, nic, orden };
    };

    // --- Radar (Grok's idea) ---
    const layoutRadar = () => {
      const c = { x: st.W / 2, y: st.H / 2 }, Rr = Math.min(st.W, st.H) * 0.42, R0 = Rr * 0.16;
      const prods = posProductos(c, R0 * 0.5);
      const mx = Math.max(1, ...VISIBLES.map((d) => d.sub));
      VISIBLES.forEach((d) => {
        const ce = centros[d.pais], grupo = VISIBLES.filter((x) => x.pais === d.pais), k = grupo.indexOf(d);
        const ang = Math.PI + (ce.lon + 180) / 360 * Math.PI * 2 + (k - (grupo.length - 1) / 2) * 0.06;
        const frac = Math.sqrt(d.sub / mx);
        const dist = R0 * 1.25 + (Rr - R0 * 1.25) * (1 - 0.9 * frac);
        d.ang = ang; d.x = c.x + Math.cos(ang) * dist; d.y = c.y + Math.sin(ang) * dist;
        const pr = posDe(prods, d, c), g = { x: c.x + Math.cos(ang) * R0, y: c.y + Math.sin(ang) * R0 };
        d.ruta = tramo((u) => ({ x: pr.x + (g.x - pr.x) * u, y: pr.y + (g.y - pr.y) * u }), 8).concat(tramo((u) => ({ x: g.x + (d.x - g.x) * u, y: g.y + (d.y - g.y) * u }), N - 8).slice(1));
        d.tg = 8 / N; d.vis = true;
      });
      return { c, Rr, R0, prods };
    };

    // =====================================================================================
    // The Earth in WebGL2 (globe and map are the same mesh: the map is the globe unrolled).
    // Texturas de la NASA (Blue Marble de día, luces de ciudades de noche) y la luz del sol
    // de este mismo momento: donde ahora es de noche, en la pantalla también lo es.
    // =====================================================================================
    const glc = $('gl');
    const T = { ok: false, m: 0, mObj: 0, zoom: 1, px: 0, py: 0, sol: [0, 0, 1], solLL: [0, 0], tanH: Math.tan(15 * RAD), M: null, PV: null, C: [0, 0, 5], asp: 1 };
    const gl = glc.getContext('webgl2', { antialias: true, alpha: false, powerPreference: 'low-power' });

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
    uniform vec3 uSol, uCam; uniform mat3 uM3; uniform float uMorph;
    in vec2 vUv; in vec3 vN; in vec3 vW;
    out vec4 o;
    vec3 aces(vec3 x) { return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
    void main() {
      vec3 n = normalize(vN);
      float mu = dot(n, uSol);
      vec3 dia = pow(texture(uDia, vUv).rgb, vec3(2.2));
      float luz = texture(uLuces, vUv).r;
      float agua = texture(uAgua, vUv).r;
      float lado = smoothstep(-0.08, 0.22, mu);
      // Luz del sol (Lambert) con un mínimo para que la noche no sea un agujero negro.
      vec3 col = dia * (max(mu, 0.0) * 1.25 + 0.018);
      // Crepúsculo: banda cálida a lo largo del terminador.
      col += dia * vec3(1.0, 0.45, 0.2) * 0.35 * exp(-pow((mu - 0.03) * 10.0, 2.0));
      // Luces de las ciudades, solo donde es de noche.
      float noche = 1.0 - smoothstep(-0.20, 0.05, mu);
      col += vec3(1.0, 0.70, 0.38) * pow(luz, 1.25) * 2.4 * noche;
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
    const textura = (fuente, mip) => {
      const t = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, t);
      if (fuente) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, fuente);
      else gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([8, 20, 40, 255]));
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      if (fuente && mip) { gl.generateMipmap(gl.TEXTURE_2D); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR); const an = gl.getExtension('EXT_texture_filter_anisotropic'); if (an) gl.texParameterf(gl.TEXTURE_2D, an.TEXTURE_MAX_ANISOTROPY_EXT, Math.min(8, gl.getParameter(an.MAX_TEXTURE_MAX_ANISOTROPY_EXT))); }
      else gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      return t;
    };
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
        T.ok = true;
      } catch (e) { console.error(e); T.ok = false; }
    }

    // Cámara y modelo de este cuadro. m = 0 globo, 1 mapa.
    const camara = () => {
      const W = st.Wc, H = st.H, asp = W / H, aspU = st.W / H; T.asp = asp;
      const yo = st.lugar;
      const osc = reducido ? 0 : Math.sin(tiempo() * 0.05) * 0.35;
      const rot = -yo.lon * RAD + osc + st.dRot, tilt = Math.max(-1.25, Math.min(1.25, yo.lat * RAD * 0.8 + st.dTilt));
      const g = 1 - T.m;
      const dGlobo = 1 / Math.sin(Math.atan(0.84 * T.tanH * Math.min(1, aspU)));
      const dMapa = Math.max((Math.PI / 2) / (T.tanH * 0.94), Math.PI / (T.tanH * aspU * 0.985));
      const dist = (dGlobo * g + dMapa * T.m) / T.zoom;
      // Con el panel de datos abierto, el mundo se centra en el espacio que queda libre.
      const desplaza = ((W - st.W) / 2) * (2 * dist * T.tanH / H);
      const px = T.px * T.m + desplaza, py = T.py * T.m;
      T.M = mul(rotX(tilt * g), rotY(rot * g));
      T.PV = mul(persp(asp), trasl(-px, -py, -dist));
      T.C = [px, py, dist]; T.dist = dist; T.rot = rot * g; T.tilt = tilt * g;
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
      const W = Math.round(st.Wc * st.dpr), H = Math.round(st.H * st.dpr);
      if (glc.width !== W || glc.height !== H) { glc.width = W; glc.height = H; }
      gl.viewport(0, 0, W, H);
      // Fondo y halo.
      const o = aPantalla([0, 0, 0]);
      const rpx = Math.tan(Math.asin(1 / T.dist)) / T.tanH * (st.H / 2);
      const Lw = aplica(T.M, T.sol), ls = Math.hypot(Lw[0], Lw[1]) || 1;
      gl.disable(gl.DEPTH_TEST); gl.useProgram(PF.p); gl.bindVertexArray(vaoF);
      gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, tex.cielo); gl.uniform1i(PF.u.uCielo, 0);
      gl.uniform2f(PF.u.uRes, W, H); gl.uniform1f(PF.u.uDesp, -T.rot * 0.04);
      gl.uniform2f(PF.u.uCentro, o.x * st.dpr, H - o.y * st.dpr); gl.uniform1f(PF.u.uRadio, rpx * st.dpr);
      gl.uniform1f(PF.u.uHalo, Math.max(0, 1 - T.m * 1.6)); gl.uniform2f(PF.u.uSolP, Lw[0] / ls, Lw[1] / ls);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      // La Tierra.
      gl.enable(gl.DEPTH_TEST); gl.clear(gl.DEPTH_BUFFER_BIT);
      gl.useProgram(PT.p); gl.bindVertexArray(vaoT);
      gl.uniformMatrix4fv(PT.u.uPV, false, T.PV); gl.uniformMatrix4fv(PT.u.uM, false, T.M);
      gl.uniformMatrix3fv(PT.u.uM3, false, new Float32Array([T.M[0], T.M[1], T.M[2], T.M[4], T.M[5], T.M[6], T.M[8], T.M[9], T.M[10]]));
      gl.uniform1f(PT.u.uMorph, T.m); gl.uniform3fv(PT.u.uSol, T.sol); gl.uniform3fv(PT.u.uCam, T.C);
      [['uDia', tex.dia], ['uLuces', tex.luces], ['uAgua', tex.agua]].forEach(([n, t], i) => { gl.activeTexture(gl.TEXTURE1 + i); gl.bindTexture(gl.TEXTURE_2D, t); gl.uniform1i(PT.u[n], 1 + i); });
      gl.drawElements(gl.TRIANGLES, nIdx, gl.UNSIGNED_INT, 0);
      gl.bindVertexArray(null);
      return true;
    };

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
    const layoutTierra = () => {
      camara();
      const yo = st.lugar, cen = aPantalla(lugar(yo.lon, yo.lat, 0.002));
      const R0 = 26;
      const prods = posProductos(cen, 11);
      const mx = Math.max(1, ...VISIBLES.map((d) => d.sub));
      VISIBLES.forEach((d) => {
        const ce = centros[d.pais], grupo = VISIBLES.filter((x) => x.pais === d.pais), k = grupo.indexOf(d);
        // Several servers in one country: a small spiral around its centre, never a long row.
        const ga = k * 2.399963, rr = grupo.length > 1 ? 1.1 * Math.sqrt(k + 0.5) : 0;
        const lon1 = ce.lon + Math.cos(ga) * rr / Math.max(0.35, Math.cos(ce.lat * RAD)), lat1 = ce.lat + Math.sin(ga) * rr;
        d.lon = lon1; d.lat = lat1;
        const a = [Math.cos(yo.lat * RAD) * Math.sin(yo.lon * RAD), Math.sin(yo.lat * RAD), Math.cos(yo.lat * RAD) * Math.cos(yo.lon * RAD)];
        const b = [Math.cos(lat1 * RAD) * Math.sin(lon1 * RAD), Math.sin(lat1 * RAD), Math.cos(lat1 * RAD) * Math.cos(lon1 * RAD)];
        const om = Math.acos(Math.max(-1, Math.min(1, a[0] * b[0] + a[1] * b[1] + a[2] * b[2]))) || 1e-6;
        const alt = 0.05 + 0.3 * om / Math.PI + lado(d) * 0.03;
        const dl = Math.hypot(lon1 - yo.lon, lat1 - yo.lat) * RAD, alzaMax = 0.12 + 0.22 * dl / Math.PI + lado(d) * 0.05;
        const arco = [];
        for (let i = 0; i <= N; i++) {
          const t = i / N, s1 = Math.sin((1 - t) * om) / Math.sin(om), s2 = Math.sin(t * om) / Math.sin(om), h = 1 + alt * Math.sin(Math.PI * t);
          const sx = (a[0] * s1 + b[0] * s2) * h, sy = (a[1] * s1 + b[1] * s2) * h, sz = (a[2] * s1 + b[2] * s2) * h;
          const pl = [(yo.lon + (lon1 - yo.lon) * t) * RAD, (yo.lat + (lat1 - yo.lat) * t) * RAD + alzaMax * Math.sin(Math.PI * t), 0.02];
          const m = T.m;
          arco.push(aPantalla([sx + (pl[0] - sx) * m, sy + (pl[1] - sy) * m, sz + (pl[2] - sz) * m]));
        }
        const pr = posDe(prods, d, cen);
        let ig = arco.findIndex((q) => Math.hypot(q.x - cen.x, q.y - cen.y) > R0);
        if (ig < 1) ig = 1;
        const gte = arco[ig];
        d.ruta = tramo((t) => ({ x: pr.x + (gte.x - pr.x) * t, y: pr.y + (gte.y - pr.y) * t, v: cen.v }), 5).concat(arco.slice(ig + 1));
        d.tg = 5 / (d.ruta.length - 1);
        const fin = d.ruta[d.ruta.length - 1];
        d.x = fin.x; d.y = fin.y; d.vis = fin.v;
        // Columna de luz en el destino: altura según lo enviado (raíz, con techo).
        const hb = 0.025 + 0.11 * Math.sqrt(d.sub / mx);
        d.base = aPantalla(lugar(lon1, lat1, 0.002)); d.cima = aPantalla(lugar(lon1, lat1, hb, hb * 0.9));
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
      VISIBLES.forEach((d) => {
        const bl = bloqueado(d), col = bl ? '#ff6b7a' : prod(d.p).color, n = d.ruta.length - 1;
        if (bl) {
          trazo(d.ruta, 0, d.tg, 'rgba(255,107,122,.7)', 1.3);
          trazo(d.ruta, d.tg, 1, 'rgba(160,175,200,.16)', 1, [2, 6]);
          return;
        }
        for (let s = 0; s < 6; s++) {
          const a0 = s / 6, a1 = (s + 1) / 6, mid = 1 - Math.abs((a0 + a1) / 2 - 0.5) * 1.4;
          trazo(d.ruta, a0, Math.min(1, a1 + 1 / n), col + Math.round(18 + 22 * mid).toString(16).padStart(2, '0'), 5);
          trazo(d.ruta, a0, Math.min(1, a1 + 1 / n), col + Math.round(110 + 120 * mid).toString(16).padStart(2, '0'), 1.1);
        }
      });
      // Columnas de luz en los servidores.
      VISIBLES.forEach((d) => {
        if (!d.base.v) return;
        const bl = bloqueado(d), col = bl ? '255,107,122' : d.doc ? '255,181,71' : '255,209,102';
        const gr = cx.createLinearGradient(d.base.x, d.base.y, d.cima.x, d.cima.y);
        gr.addColorStop(0, `rgba(${col},.95)`); gr.addColorStop(1, `rgba(${col},0)`);
        cx.strokeStyle = gr; cx.lineWidth = 3; cx.beginPath(); cx.moveTo(d.base.x, d.base.y); cx.lineTo(d.cima.x, d.cima.y); cx.stroke();
        cx.lineWidth = 7; cx.globalAlpha = 0.25; cx.stroke(); cx.globalAlpha = 1;
        const pulso = (tt * 0.6 + d.i * 0.13) % 1;
        trazoPuntos(circuloSuelo(d.lon, d.lat, 0.6 + pulso * 2.2, 28), `rgba(${col},${0.55 * (1 - pulso)})`, 1.2);
        cx.drawImage(brillo(bl ? '#ff6b7a' : '#ffb547'), d.base.x - 9, d.base.y - 9, 18, 18);
        st.dianas.push({ x: d.base.x, y: d.base.y, r: 14, tipo: 'dest', ref: d });
      });
      // El escudo de GUARDIANA sobre tu ordenador.
      if (L.cen.v) {
        [2.2, 3.6, 5.2].forEach((r, k) => trazoPuntos(circuloSuelo(L.yo.lon, L.yo.lat, r + (k === 2 ? Math.sin(tt * 1.6) * 0.3 : 0), 64), `rgba(63,224,197,${[0.9, 0.55, 0.3][k]})`, k ? 1 : 1.6, k === 1 ? [6, 5] : null, k === 1 ? -tt * 14 : 0));
        const cima = aPantalla(lugar(L.yo.lon, L.yo.lat, 0.07, 0.06));
        const gr = cx.createLinearGradient(L.cen.x, L.cen.y, cima.x, cima.y); gr.addColorStop(0, 'rgba(63,224,197,.9)'); gr.addColorStop(1, 'rgba(63,224,197,0)');
        cx.strokeStyle = gr; cx.lineWidth = 3; cx.beginPath(); cx.moveTo(L.cen.x, L.cen.y); cx.lineTo(cima.x, cima.y); cx.stroke();
        cx.drawImage(brillo('#3fe0c5'), L.cen.x - 16, L.cen.y - 16, 32, 32);
        st.dianas.push({ x: L.cen.x, y: L.cen.y, r: 10, tipo: 'yo' });
        L.prods.forEach((q) => { cx.drawImage(brillo(q.p.color), q.x - 8, q.y - 8, 16, 16); st.dianas.push({ x: q.x, y: q.y, r: 9, tipo: 'prod', ref: q.p }); });
        cx.globalCompositeOperation = 'source-over';
        rotulo(cima.x + 8, cima.y - 4, t('pd_tu_ordenador').toUpperCase() + ' · ' + nombrePais(st.yo).toUpperCase(), '#3fe0c5');
        cx.globalCompositeOperation = 'lighter';
      }
      particulas(dt);
      cx.globalCompositeOperation = 'source-over';
      // Rótulos: país y empresas dueñas de las redes, junto a las columnas, sin pisarse.
      const puestos = [];
      // La velocidad de los tres flujos más vivos, sobre lo alto de su arco.
      if (st.W >= 700) VISIBLES.filter((d) => !bloqueado(d) && d.ema > 800).sort((a, b) => b.ema - a.ema).slice(0, 3).forEach((d) => {
        const cima = d.ruta[Math.round(d.ruta.length * 0.55)];
        if (!cima || !cima.v) return;
        rotulo(cima.x + 6, cima.y - 6, `↑ ${bytes(d.ema)}/s · ${d.host.split('.')[0]}`, prod(d.p).color, puestos);
      });
      // En el móvil, sin rótulos: se ve el mundo y se toca para saber más.
      if (st.W >= 700) paisesUsados.forEach((c) => {
        const ds = VISIBLES.filter((d) => d.pais === c), d0 = ds.reduce((a, d) => (d.cima.y < a.cima.y ? d : a), ds[0]);
        if (!d0.cima.v) return;
        const todas = [...new Set(ds.map((d) => d.red).filter((r) => r && r[0] !== '@'))], redes = todas.slice(0, 3).join(' · ') + (todas.length > 3 ? ` +${todas.length - 3}` : '');
        const corta = ds.every(bloqueado);
        rotulo(d0.cima.x + 6, d0.cima.y - 2, `${nombrePais(c)} — ${redes}`, corta ? '#ff8d99' : '#f3e3c8', puestos);
      });
    };
    const rotulo = (x, y, txt, col, puestos) => {
      cx.font = '500 10.5px "IBM Plex Mono", monospace';
      const w = cx.measureText(txt).width + 12, h = 17;
      let bx = Math.min(st.W - w - 6, Math.max(6, x)), by = y - h + 4;
      if (puestos) { let n = 0; while (n++ < 12 && puestos.some((b) => bx < b.x + b.w && bx + w > b.x && by < b.y + b.h && by + h > b.y)) by += h + 2; puestos.push({ x: bx, y: by, w, h }); }
      cx.fillStyle = 'rgba(4,10,20,.72)'; cx.fillRect(bx, by, w, h);
      cx.fillStyle = col; cx.fillRect(bx, by, 2, h);
      cx.fillStyle = col; cx.textBaseline = 'middle'; cx.fillText(txt, bx + 7, by + h / 2 + 0.5); cx.textBaseline = 'alphabetic';
    };
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
    const paisBajo = (x, y) => { const ll = lonLatBajo(x, y); return ll ? paisEn(ll[0], ll[1]) : null; };


    // ---------- Drawing ----------
    const enTierra = () => (st.vista === 'globo' || st.vista === 'mapa') && T.ok;
    const nebulosaGL = () => st.vista === 'nebulosa' && T.ok && !!PN;
    const fondo = () => {
      if (enTierra() || nebulosaGL()) { cx.clearRect(0, 0, st.Wc, st.H); return; }
      cx.fillStyle = '#04070d'; cx.fillRect(0, 0, st.Wc, st.H);
      const tt = tiempo();
      estrellas.forEach((e) => { const a = 0.25 + 0.25 * Math.sin(tt * 0.8 + e.f); cx.fillStyle = `rgba(190,215,240,${reducido ? 0.35 : a})`; cx.fillRect(e.x * st.W, e.y * st.H, e.r, e.r); });
    };
    const trazo = (ruta, desde, hasta, color, ancho, guion) => {
      if (!ruta.length) return;
      const n = ruta.length - 1, i0 = Math.round(desde * n), i1 = Math.round(hasta * n);
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
      VISIBLES.forEach((d) => {
        if (!d.ruta.length) return;
        const bl = bloqueado(d), base = prod(d.p).color;
        if (bl) {
          trazo(d.ruta, 0, d.tg, 'rgba(255,107,122,.7)', 1.3);
          trazo(d.ruta, d.tg, 1, 'rgba(160,175,200,.16)', 1, [2, 6]);
          const g = punto(d.ruta, d.tg), h = punto(d.ruta, d.tg + 0.01);
          if (g.v) puerta(g, Math.atan2(h.y - g.y, h.x - g.x));
        } else {
          trazo(d.ruta, 0, 1, base + '55', 1.1);
          trazo(d.ruta, 0, 1, base + '14', 4);
        }
      });
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
    // Labels only for the ones that move most (and the cut ones): the rest on hover.
    const conNombre = (d) => st.W >= 700 && (bloqueado(d) || VISIBLES.indexOf(d) < 22);

    const dibujaNebulosa = (dt) => {
      const L = layoutNebulosa();
      Object.entries(L.cent).forEach(([c, ce]) => {
        const h = tono(c), escala = Math.min(st.W, st.H) * 0.11;
        nubeDe[c].forEach((s) => { cx.fillStyle = `hsla(${h},75%,72%,${s.a * 0.8})`; cx.fillRect(ce.x + s.x * escala, ce.y + s.y * escala, s.r, s.r); });
        cx.font = '600 11px "IBM Plex Mono", monospace'; cx.fillStyle = `hsla(${h},70%,78%,.9)`; cx.textAlign = 'center';
        const n = (L.porPais[c] || []).length, alto = Math.min(n * 11, st.H * 0.15), ly = ce.y + (Math.sin(ce.a) > 0 ? alto + 22 : -alto - 14);
        cx.fillText(nombrePais(c).toUpperCase(), ce.x, ly); cx.textAlign = 'left';
      });
      tentaculos();
      escudo(L.c, L.R0);
      ordenador(L.c, Math.max(18, L.R0 * 0.32));
      st.dianas.push({ x: L.c.x, y: L.c.y, r: L.R0 * 0.3, tipo: 'yo' });
      L.prods.forEach(nodoProducto);
      particulas(dt);
      VISIBLES.forEach((d) => nodoDestino(d, conNombre(d), d.x < L.c.x ? 'izq' : 'der'));
    };
    const dibujaDentro = (dt) => {
      const L = layoutDentro();
      const k = L.caja;
      cx.strokeStyle = 'rgba(118,144,168,.35)'; cx.lineWidth = 1; cx.setLineDash([6, 5]); cx.strokeRect(k.x, k.y, k.w, k.h); cx.setLineDash([]);
      cx.font = '500 11px "IBM Plex Mono", monospace'; cx.fillStyle = 'rgba(160,185,210,.85)';
      cx.fillText(t('pd_tu_ordenador').toUpperCase(), k.x + 10, k.y + 16);
      cx.fillText(t('pd_internet').toUpperCase(), L.col.inet - 30, 46);
      cx.fillText(t('pd_pais_red').toUpperCase(), L.col.dest - 6, 46);
      const tt = tiempo();
      const gm = cx.createLinearGradient(L.col.puerta - 14, 0, L.col.puerta + 14, 0);
      gm.addColorStop(0, 'rgba(63,224,197,0)'); gm.addColorStop(0.5, `rgba(63,224,197,${0.2 + 0.06 * Math.sin(tt * 2)})`); gm.addColorStop(1, 'rgba(63,224,197,0)');
      cx.fillStyle = gm; cx.fillRect(L.col.puerta - 14, k.y + 26, 28, k.h - 34);
      cx.fillStyle = '#3fe0c5'; cx.textAlign = 'center'; cx.fillText('GUARDIANA', L.col.puerta, k.y + 16);
      cx.fillText(t('pd_tarjeta_red').toUpperCase(), L.nic.x, L.nic.y - 26); cx.textAlign = 'left';
      cx.strokeStyle = 'rgba(160,185,210,.7)'; cx.strokeRect(L.nic.x - 16, L.nic.y - 14, 32, 28);
      tentaculos();
      L.orden.forEach((d) => {
        if (bloqueado(d)) puerta(d.puerta, 0); else { cx.fillStyle = '#3fe0c5'; cx.fillRect(d.puerta.x - 1, d.puerta.y - 7, 2, 4); cx.fillRect(d.puerta.x - 1, d.puerta.y + 3, 2, 4); }
      });
      L.prods.forEach((q) => { nodoProducto(q); cx.font = '600 12px "IBM Plex Sans", sans-serif'; cx.fillStyle = '#d3e6f3'; cx.fillText(q.p.nombre, q.x - 34, q.y - 18); cx.font = '10.5px "IBM Plex Mono", monospace'; cx.fillStyle = 'rgba(160,185,210,.8)'; cx.fillText((q.p.procesos[0] || ''), q.x - 34, q.y + 24); });
      particulas(dt);
      L.orden.forEach((d) => {
        nodoDestino(d, false);
        cx.font = '500 10.5px "IBM Plex Mono", monospace'; cx.fillStyle = bloqueado(d) ? '#ff8d99' : '#f3e3c8';
        const txt = `${nombrePais(d.pais)} · ${d.red && d.red[0] !== '@' ? d.red : d.host}`; cx.fillText(txt.length > 30 ? txt.slice(0, 29) + '…' : txt, d.x + 10, d.y + 3.5);
      });
    };
    const dibujaRadar = (dt) => {
      const L = layoutRadar(), tt = tiempo(), barrido = (tt * (reducido ? 0.3 : 0.9)) % (Math.PI * 2);
      cx.strokeStyle = 'rgba(63,224,197,.16)'; cx.lineWidth = 1;
      for (let k = 1; k <= 4; k++) { cx.beginPath(); cx.arc(L.c.x, L.c.y, L.Rr * k / 4, 0, 7); cx.stroke(); }
      for (let k = 0; k < 12; k++) { const a = k * Math.PI / 6; cx.beginPath(); cx.moveTo(L.c.x, L.c.y); cx.lineTo(L.c.x + Math.cos(a) * L.Rr, L.c.y + Math.sin(a) * L.Rr); cx.stroke(); }
      for (let k = 0; k < 24; k++) { const a0 = barrido - k * 0.03; cx.fillStyle = `rgba(63,224,197,${0.12 * (1 - k / 24)})`; cx.beginPath(); cx.moveTo(L.c.x, L.c.y); cx.arc(L.c.x, L.c.y, L.Rr, a0 - 0.03, a0); cx.closePath(); cx.fill(); }
      cx.strokeStyle = 'rgba(63,224,197,.8)'; cx.beginPath(); cx.moveTo(L.c.x, L.c.y); cx.lineTo(L.c.x + Math.cos(barrido) * L.Rr, L.c.y + Math.sin(barrido) * L.Rr); cx.stroke();
      cx.font = '500 10px "IBM Plex Mono", monospace'; cx.fillStyle = 'rgba(160,185,210,.7)'; cx.textAlign = 'center';
      [[t('pd_america'), -100], [t('pd_europa'), 10], [t('pd_asia'), 110]].forEach(([n, lon]) => { const a = Math.PI + (lon + 180) / 360 * Math.PI * 2; cx.fillText(n.toUpperCase(), L.c.x + Math.cos(a) * (L.Rr + 14), L.c.y + Math.sin(a) * (L.Rr + 14)); });
      cx.fillText(t('pd_radar_centro'), L.c.x, L.c.y + L.Rr + 30); cx.textAlign = 'left';
      tentaculos();
      escudo(L.c, L.R0); ordenador(L.c, Math.max(14, L.R0 * 0.32));
      st.dianas.push({ x: L.c.x, y: L.c.y, r: L.R0 * 0.3, tipo: 'yo' });
      L.prods.forEach(nodoProducto);
      particulas(dt);
      VISIBLES.forEach((d) => {
        const delta = ((barrido - ((d.ang % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2)) + Math.PI * 4) % (Math.PI * 2);
        if (delta < 0.6) { cx.strokeStyle = bloqueado(d) ? `rgba(255,107,122,${0.8 - delta})` : `rgba(255,181,71,${0.8 - delta})`; cx.lineWidth = 1.5; cx.beginPath(); cx.arc(d.x, d.y, 6 + delta * 22, 0, 7); cx.stroke(); }
        nodoDestino(d, conNombre(d) && delta < 2.2, d.x < L.c.x ? 'izq' : 'der');
      });
    };


    // =====================================================================================
    // Second pass on three views (the owner, 13 Oct 2026: «el radar, nebulosa y ordenador
    // necesito más calidad profesional»), worked out with Grok from the best references there
    // are: the nebula as thousands of points in WebGL, one turning galaxy per country, with
    // depth; the radar as a real one, with phosphor persistence, a true logarithmic scale of
    // bytes, bearings and air-traffic-control tags; the computer as a Sankey diagram whose
    // bands are as wide as the bytes, through GUARDIANA's gate.
    // =====================================================================================

    // ---------- 1. Nebula: WebGL2 points, one galaxy per country ----------
    const VS_NEB = `#version 300 es
    in vec3 aCentro; in vec3 aOff; in vec4 aColor; in vec2 aTam;
    uniform mat4 uPV; uniform float uT, uPx; uniform float uAct[24];
    out vec4 vColor;
    void main() {
      float r = length(aOff.xy);
      float ang = uT * (0.025 + 0.2 / (0.45 + r));
      float c = cos(ang), s = sin(ang);
      vec3 o = vec3(aOff.x * c - aOff.y * s, aOff.x * s + aOff.y * c, aOff.z);
      o += 0.04 * vec3(sin(uT * 0.7 + aTam.y * 13.0), cos(uT * 0.6 + aTam.y * 7.0), sin(uT * 0.5 + aTam.y * 5.0));
      float act = uAct[int(aColor.a + 0.5)];
      vec4 p = uPV * vec4(aCentro + o, 1.0);
      gl_Position = p;
      gl_PointSize = clamp(aTam.x * uPx / p.w * (1.0 + act * 0.5), 1.0, 26.0);
      vColor = vec4(aColor.rgb * (0.75 + act * 0.9), 0.7 + 0.3 * sin(uT * 1.6 + aTam.y * 40.0));
    }`;
    const FS_NEB = `#version 300 es
    precision mediump float;
    in vec4 vColor; out vec4 o;
    void main() {
      vec2 q = gl_PointCoord * 2.0 - 1.0; float d = dot(q, q);
      if (d > 1.0) discard;
      float a = exp(-d * 4.2) * vColor.a;
      o = vec4(vColor.rgb * a, a);
    }`;
    let PN = null, vaoN = null, bufN = null, nPuntos = 0, firmaNeb = '';
    if (T.ok) {
      try {
        PN = programa(VS_NEB, FS_NEB);
        vaoN = gl.createVertexArray(); bufN = gl.createBuffer();
        gl.bindVertexArray(vaoN); gl.bindBuffer(gl.ARRAY_BUFFER, bufN);
        const paso = 12 * 4;
        [['aCentro', 3, 0], ['aOff', 3, 12], ['aColor', 4, 24], ['aTam', 2, 40]].forEach(([n, k, o]) => { const l = gl.getAttribLocation(PN.p, n); gl.enableVertexAttribArray(l); gl.vertexAttribPointer(l, k, gl.FLOAT, false, paso, o); });
        gl.bindVertexArray(null);
      } catch (e) { console.error(e); PN = null; }
    }
    const hsl = (h, s, l) => { s /= 100; l /= 100; const k = (n) => (n + h / 30) % 12, a = s * Math.min(l, 1 - l), f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1))); return [f(0), f(8), f(4)]; };
    const NEB = { rx: 10.5, ry: 6.2, centros: {}, ordenPaises: [] };
    // Countries west to east around the ring: the Americas on the left, Europe on top, Asia right.
    const centroNeb = (k, n) => { const a = Math.PI - (k + 0.5) * (2 * Math.PI / Math.max(1, n)); return [Math.cos(a) * NEB.rx, Math.sin(a) * NEB.ry, 2.2 * Math.sin(2 * a)]; };
    const construyeNebulosa = () => {
      // A tall screen (a phone) gets a tall ring, so the galaxies are not drawn tiny.
      const vertical = st.W < st.H * 0.9;
      NEB.rx = vertical ? 6.4 : 10.5; NEB.ry = vertical ? 9.6 : 6.2;
      const orden = paisesUsados.slice().sort((a, b) => centros[a].lon - centros[b].lon);
      const totPais = {}; VISIBLES.forEach((d) => { totPais[d.pais] = (totPais[d.pais] || 0) + d.sub + d.baj + d.bloq * 2000; });
      const total = Math.max(1, Object.values(totPais).reduce((a, b) => a + b, 0));
      const firma = (vertical ? 'v' : 'h') + orden.join(',') + '|' + orden.map((c) => Math.round(Math.sqrt((totPais[c] || 0) / total) * 6)).join('');
      NEB.ordenPaises = orden;
      orden.forEach((c, k) => { NEB.centros[c] = centroNeb(k, orden.length); });
      if (!PN || firma === firmaNeb) return;
      firmaNeb = firma;
      const datos = [];
      const punto3 = (cen, off, col, grupo, tam) => { datos.push(cen[0], cen[1], cen[2], off[0], off[1], off[2], col[0], col[1], col[2], grupo, tam, azar()); };
      orden.forEach((c, k) => {
        const cen = NEB.centros[c], h = tono(c), m = Math.round(600 + 2600 * Math.sqrt((totPais[c] || 0) / total)), brazos = 2 + (k % 2), giro = azar() * 6.28, inclina = 0.35 + azar() * 0.4;
        for (let i = 0; i < m; i++) {
          const r = Math.min(3.4, -Math.log(1 - azar() * 0.985) * 0.72), brazo = i % brazos;
          const th = r * 2.3 + brazo * 2 * Math.PI / brazos + gauss() * 0.3 + giro;
          let x = Math.cos(th) * r, y = Math.sin(th) * r, z = gauss() * 0.14 * (1 - r / 3);
          const y2 = y * Math.cos(inclina) - z * Math.sin(inclina), z2 = y * Math.sin(inclina) + z * Math.cos(inclina); y = y2; z = z2;
          const blanco = r < 0.25 && azar() < 0.5, col = blanco ? [1, 0.95, 0.9] : hsl((h + gauss() * 14 + 360) % 360, 70 + azar() * 20, 55 + azar() * 28);
          punto3(cen, [x, y * 0.8, z], col, k % 21, 0.05 + Math.pow(azar(), 3) * 0.12);
        }
      });
      // The computer: a hot core, and GUARDIANA's ring around it (a dashed torus of light).
      for (let i = 0; i < 700; i++) { const r = Math.abs(gauss()) * 0.32; const th = azar() * 6.28; punto3([0, 0, 0], [Math.cos(th) * r, Math.sin(th) * r, gauss() * 0.15], azar() < 0.5 ? [1, 0.55, 0.62] : [1, 0.9, 0.92], 22, 0.03 + azar() * 0.06); }
      for (let i = 0; i < 1800; i++) { const th = azar() * 6.28; if ((th % (2 * Math.PI / 28)) > (2 * Math.PI / 28) * 0.72) continue; const r = 2.05 + gauss() * 0.04; punto3([0, 0, 0], [Math.cos(th) * r, Math.sin(th) * r, gauss() * 0.04], [0.25, 0.88, 0.77], 21, 0.025 + azar() * 0.03); }
      // Dust between them.
      for (let i = 0; i < 2600; i++) punto3([0, 0, 0], [gauss() * 7, gauss() * 4.2, gauss() * 4], [0.55, 0.65, 0.85], 23, 0.012 + azar() * 0.02);
      nPuntos = datos.length / 12;
      gl.bindBuffer(gl.ARRAY_BUFFER, bufN); gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(datos), gl.STATIC_DRAW);
    };
    const NC = { PV: null, C: [0, 0, 20], tanH: Math.tan(20 * RAD) };
    const camaraNeb = () => {
      const W = st.Wc, H = st.H, asp = W / H, aspU = st.W / H, tt = tiempo();
      // Room for each galaxy (up to 3.4 across) and the title under it; the whole ring a little
      // up, clear of the legend and the help at the bottom.
      const D = Math.max((NEB.ry + 5.2) / NC.tanH, (NEB.rx + 4.2) / (NC.tanH * aspU)) * 1.02 / T.zoom;
      const px = 2 * D * NC.tanH / H, despl = ((W - st.W) / 2) * px, subir = 28 * px;
      const rx = st.raton ? (st.raton.x / W - 0.5) : 0, ry = st.raton ? (st.raton.y / H - 0.5) : 0;
      NEB.yaw += ((reducido ? 0 : 0.2 * Math.sin(tt * 0.045)) + rx * 0.16 + st.dRot - NEB.yaw) * 0.04;
      NEB.pitch += ((reducido ? 0 : 0.07 * Math.sin(tt * 0.031)) + ry * 0.1 + st.dTilt * 0.5 - NEB.pitch) * 0.04;
      const t0 = 1 / NC.tanH, n = 0.5, f = 120, P = new Float32Array(16); P[0] = t0 / asp; P[5] = t0; P[10] = (f + n) / (n - f); P[11] = -1; P[14] = 2 * f * n / (n - f);
      NC.PV = mul(P, mul(trasl(-despl, subir, -D), mul(rotX(NEB.pitch), rotY(NEB.yaw))));
      NC.D = D;
    };
    NEB.yaw = 0; NEB.pitch = 0;
    const aPantallaNeb = (q) => { const c = aplica(NC.PV, q); return { x: (c[0] / c[3] + 1) / 2 * st.Wc, y: (1 - c[1] / c[3]) / 2 * st.H, v: c[3] > 0.1, w: c[3] }; };
    const actividad = new Float32Array(24);
    const pintaNebulosaGL = () => {
      const W = Math.round(st.Wc * st.dpr), H = Math.round(st.H * st.dpr);
      if (glc.width !== W || glc.height !== H) { glc.width = W; glc.height = H; }
      gl.viewport(0, 0, W, H);
      gl.disable(gl.DEPTH_TEST); gl.useProgram(PF.p); gl.bindVertexArray(vaoF);
      gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, tex.cielo); gl.uniform1i(PF.u.uCielo, 0);
      gl.uniform2f(PF.u.uRes, W, H); gl.uniform1f(PF.u.uDesp, -NEB.yaw * 0.06); gl.uniform1f(PF.u.uHalo, 0); gl.uniform2f(PF.u.uCentro, 0, 0); gl.uniform1f(PF.u.uRadio, 1); gl.uniform2f(PF.u.uSolP, 1, 0);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      if (!PN || !nPuntos) return;
      // How alive each galaxy is right now: what its destinations sent in the last beats.
      const porPais = {}; let todo = 0;
      VISIBLES.forEach((d) => { porPais[d.pais] = (porPais[d.pais] || 0) + (d.ema || 0); todo += d.ema || 0; });
      NEB.ordenPaises.forEach((c, k) => { const v = Math.min(1, Math.sqrt((porPais[c] || 0) / 40000)); actividad[k % 21] += (v - actividad[k % 21]) * 0.08; });
      actividad[22] += (Math.min(1, Math.sqrt(todo / 80000)) - actividad[22]) * 0.08; actividad[21] = 0.35 + 0.25 * Math.sin(tiempo() * 1.4); actividad[23] = 0;
      gl.useProgram(PN.p); gl.bindVertexArray(vaoN);
      gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE);
      gl.uniformMatrix4fv(PN.u.uPV, false, NC.PV); gl.uniform1f(PN.u.uT, reducido ? 0 : tiempo()); gl.uniform1f(PN.u.uPx, H / (2 * NC.tanH));
      gl.uniform1fv(PN.u['uAct[0]'], actividad);
      gl.drawArrays(gl.POINTS, 0, nPuntos);
      gl.disable(gl.BLEND); gl.bindVertexArray(null);
    };
    // Routes and labels of the nebula, drawn over the points with the same camera.
    const layoutNebulosaPro = () => {
      const tt = tiempo(), cen = aPantallaNeb([0, 0, 0]);
      const ps = prodsVisibles();
      const prods = ps.map((p, k) => { const a = tt * (reducido ? 0.03 : 0.12) + k * 2 * Math.PI / Math.max(1, ps.length); const q = aPantallaNeb([Math.cos(a) * 1.0, Math.sin(a) * 1.0, 0]); return { p, x: q.x, y: q.y, w3: [Math.cos(a), Math.sin(a), 0] }; });
      const porPais = {};
      VISIBLES.forEach((d) => { (porPais[d.pais] = porPais[d.pais] || []).push(d); });
      Object.values(porPais).forEach((g) => g.sort((a, b) => (b.sub + b.baj) - (a.sub + a.baj)));
      VISIBLES.forEach((d) => {
        const g = porPais[d.pais], j = g.indexOf(d), c3 = NEB.centros[d.pais] || [0, 0, 0];
        const ga = j * 2.399963, rr = 0.8 + 0.42 * Math.sqrt(j);
        const p3 = [c3[0] + Math.cos(ga) * rr, c3[1] + Math.sin(ga) * rr * 0.6, c3[2] + 0.3 * Math.sin(j * 1.7)];
        d.p3 = p3;
        const dir = Math.atan2(c3[1], c3[0]), gate3 = [Math.cos(dir) * 2.05, Math.sin(dir) * 2.05, 0];
        const lad = lado(d) || 0.3, ctrl3 = [c3[0] * 0.5 - Math.sin(dir) * 1.1 * lad, c3[1] * 0.5 + Math.cos(dir) * 1.1 * lad, c3[2] * 0.5];
        const pr = prods.find((q) => q.p.id === d.p), from3 = pr ? pr.w3 : [0, 0, 0];
        const ruta = [];
        for (let i = 0; i <= 6; i++) { const u = i / 6; ruta.push(aPantallaNeb([from3[0] + (gate3[0] - from3[0]) * u, from3[1] + (gate3[1] - from3[1]) * u, 0])); }
        for (let i = 1; i <= N - 6; i++) { const u = i / (N - 6), a = (1 - u) * (1 - u), b = 2 * (1 - u) * u, c = u * u; ruta.push(aPantallaNeb([a * gate3[0] + b * ctrl3[0] + c * p3[0], a * gate3[1] + b * ctrl3[1] + c * p3[1], a * gate3[2] + b * ctrl3[2] + c * p3[2]])); }
        d.ruta = ruta; d.tg = 6 / N;
        const fin = ruta[ruta.length - 1]; d.x = fin.x; d.y = fin.y; d.vis = fin.v;
      });
      return { cen, prods, porPais };
    };
    const dibujaNebulosaPro = (dt) => {
      const L = layoutNebulosaPro();
      const tt = tiempo();
      // GUARDIANA's ring, crisp over its points.
      const anillo = []; for (let i = 0; i <= 96; i++) { const a = i / 96 * Math.PI * 2; anillo.push(aPantallaNeb([Math.cos(a) * 2.05, Math.sin(a) * 2.05, 0])); }
      cx.globalCompositeOperation = 'lighter';
      trazoPuntos(anillo, 'rgba(63,224,197,.55)', 1.2, [10, 6], -tt * 16);
      // Filaments: a fine thread per destination, a faint wide veil per country (they bundle).
      VISIBLES.forEach((d) => {
        if (!d.ruta.length) return;
        const bl = bloqueado(d), col = bl ? '#ff6b7a' : prod(d.p).color, vivo = Math.min(1, (d.ema || 0) / 20000);
        if (bl) { trazo(d.ruta, 0, d.tg, 'rgba(255,107,122,.65)', 1.2); trazo(d.ruta, d.tg, 1, 'rgba(160,175,200,.12)', 1, [2, 6]); return; }
        trazo(d.ruta, 0, 1, col + '10', 6);
        trazo(d.ruta, 0, 1, col + Math.round(60 + 120 * vivo).toString(16).padStart(2, '0'), 0.9);
      });
      cx.globalCompositeOperation = 'source-over';
      ordenador(L.cen, 15);
      st.dianas.push({ x: L.cen.x, y: L.cen.y, r: 16, tipo: 'yo' });
      L.prods.forEach(nodoProducto);
      particulas(dt);
      VISIBLES.forEach((d) => { if (d.vis) nodoDestino(d, false); });
      // Country titles under each galaxy, with what moved.
      const puestos = [];
      NEB.ordenPaises.forEach((c) => {
        const g = L.porPais[c] || [], q = aPantallaNeb(NEB.centros[c]);
        if (!q.v) return;
        st.dianas.push({ x: q.x, y: q.y, r: 26, tipo: 'pais', ref: c });
        const sub = g.reduce((a, d) => a + d.sub, 0), h = tono(c), c3 = NEB.centros[c];
        cx.textAlign = 'center';
        cx.font = '500 11px "Unbounded", "IBM Plex Sans", sans-serif'; cx.fillStyle = `hsla(${h},75%,80%,.95)`;
        const y0 = aPantallaNeb([c3[0], c3[1] - 2.6, c3[2]]).y + 12;
        cx.fillText(nombrePais(c).toUpperCase(), q.x, y0);
        cx.font = '10px "IBM Plex Mono", monospace'; cx.fillStyle = 'rgba(170,190,210,.8)';
        cx.fillText(`${tn('pd_nombres', g.length)} · ${bytes(sub)}`, q.x, y0 + 14);
        cx.textAlign = 'left';
        puestos.push({ x: q.x - 60, y: y0 - 12, w: 120, h: 30 });
      });
      // Callouts with leader lines for the ones that move most (and every one stopped).
      if (st.W >= 700) {
        const marcados = VISIBLES.filter((d) => d.vis && (bloqueado(d) || VISIBLES.indexOf(d) < 18)).slice(0, 24);
        marcados.forEach((d) => {
          const derecha = d.x >= L.cen.x;
          const txt = d.host.length > 30 ? d.host.slice(0, 29) + '…' : d.host;
          const sub = bloqueado(d) ? tn('pd_parados', d.bloq) : `${d.pais} · ${d.red && d.red[0] !== '@' ? d.red.slice(0, 16) : '—'} · ${bytes(d.sub)}`;
          cx.font = '500 10.5px "IBM Plex Mono", monospace';
          const w = Math.max(cx.measureText(txt).width, cx.measureText(sub).width) + 14, h = 30;
          // Beside its point; if the place is taken, a little above or below; else no callout. Never
          // over the legend at the bottom.
          const bx = Math.max(4, Math.min(st.W - w - 4, derecha ? d.x + 26 : d.x - 26 - w));
          const libre = (y) => y >= 6 && y + h <= st.H - 78 && !puestos.some((b) => bx < b.x + b.w && bx + w > b.x && y < b.y + b.h && y + h > b.y);
          const by = [0, 1, -1, 2, -2, 3, -3].map((k) => d.y - h / 2 + k * (h + 3)).find(libre);
          if (by === undefined) return;
          puestos.push({ x: bx, y: by, w, h });
          const bl = bloqueado(d), col = bl ? '#ff6b7a' : d.doc === false ? '#ffd166' : 'rgba(255,181,71,.8)';
          cx.strokeStyle = 'rgba(170,190,210,.45)'; cx.lineWidth = 1;
          const ex = derecha ? bx : bx + w;
          cx.beginPath(); cx.moveTo(d.x, d.y); cx.lineTo(ex + (derecha ? -8 : 8), by + h / 2); cx.lineTo(ex, by + h / 2); cx.stroke();
          cx.fillStyle = 'rgba(4,10,20,.8)'; cx.fillRect(bx, by, w, h);
          cx.strokeStyle = col; cx.setLineDash(d.doc === false && !bl ? [3, 2] : []); cx.strokeRect(bx + 0.5, by + 0.5, w - 1, h - 1); cx.setLineDash([]);
          cx.fillStyle = bl ? '#ff8d99' : '#f3e3c8'; cx.fillText(txt, bx + 7, by + 12);
          cx.font = '10px "IBM Plex Mono", monospace'; cx.fillStyle = 'rgba(160,185,210,.85)'; cx.fillText(sub, bx + 7, by + 24);
        });
      }
    };

    // ---------- 2. Radar: phosphor, a true scale of bytes, bearings, tags ----------
    let fosforo = null, barridoAntes = 0, radarAntes = 0;
    const capaFosforo = () => {
      if (!fosforo || fosforo.width !== cv.width || fosforo.height !== cv.height) { fosforo = document.createElement('canvas'); fosforo.width = cv.width; fosforo.height = cv.height; }
      const f = fosforo.getContext('2d'); f.setTransform(st.dpr, 0, 0, st.dpr, 0, 0); return f;
    };
    let rayas = null;
    const patronRayas = () => { if (rayas) return rayas; const c = document.createElement('canvas'); c.width = 4; c.height = 3; const g = c.getContext('2d'); g.fillStyle = 'rgba(0,0,0,.18)'; g.fillRect(0, 0, 4, 1); rayas = cx.createPattern(c, 'repeat'); return rayas; };
    const PERIODO = 7;
    const layoutRadarPro = () => {
      // Room around the scope for its bearings and regions, and for the legend under it. A narrow
      // screen: the scope as large as it fits under its readout, without the region arcs.
      const estrecho = st.W < 700;
      const Rr = estrecho ? Math.max(60, Math.min(st.W / 2 - 34, st.H / 2 - 77)) : Math.max(80, Math.min(st.W * 0.4, st.H / 2 - 72)), R0 = Rr * 0.12;
      const c = { x: st.W / 2, y: estrecho ? 72 + Rr : st.H / 2 + 4 };
      // The scale follows the data: the busiest contact near the inner ring, the quietest near the
      // edge, at least three powers of ten between them.
      const vals = VISIBLES.map((d) => d.sub).filter((v) => v > 0);
      const hi = Math.log10(Math.max(1000, ...vals, 1)) + 0.25, lo = Math.min(hi - 3, Math.log10(Math.max(10, Math.min(...vals, 1e12))) - 0.25);
      const lmax = hi, lmin = Math.max(1, lo);
      const radio = (b) => { const l = Math.log10(Math.max(1, b)); const f = Math.min(1, Math.max(0, (lmax - l) / (lmax - lmin))); return R0 * 1.6 + (Rr * 0.93 - R0 * 1.6) * f; };
      const prods = posProductos(c, R0 * 0.55);
      VISIBLES.forEach((d) => {
        const ce = centros[d.pais], grupo = VISIBLES.filter((x) => x.pais === d.pais), k = grupo.indexOf(d);
        const rumbo = ((ce.lon % 360) + 360) % 360 + (k - (grupo.length - 1) / 2) * Math.min(2.4, 20 / grupo.length);
        const ang = (rumbo - 90) * RAD, dist = d.sub ? radio(d.sub) : Rr * 0.97;
        d.ang = ang; d.rumbo = (rumbo + 360) % 360; d.x = c.x + Math.cos(ang) * dist; d.y = c.y + Math.sin(ang) * dist;
        const pr = posDe(prods, d, c), g = { x: c.x + Math.cos(ang) * R0, y: c.y + Math.sin(ang) * R0 };
        d.ruta = tramo((u) => ({ x: pr.x + (g.x - pr.x) * u, y: pr.y + (g.y - pr.y) * u }), 8).concat(tramo((u) => ({ x: g.x + (d.x - g.x) * u, y: g.y + (d.y - g.y) * u }), N - 8).slice(1));
        d.tg = 8 / N; d.vis = true;
      });
      return { c, Rr, R0, prods, lmin, lmax, radio, estrecho };
    };
    // A line of facts split at its « · » so each piece fits the width.
    const partir = (texto, ancho) => {
      const out = []; let linea = '';
      texto.split(' · ').forEach((p) => { const prueba = linea ? linea + ' · ' + p : p; if (linea && cx.measureText(prueba).width > ancho) { out.push(linea); linea = p; } else linea = prueba; });
      if (linea) out.push(linea);
      return out;
    };
    const etiquetaBytes = (l) => bytes(Math.pow(10, l)).replace('.0 ', ' ');
    const dibujaRadarPro = (dt) => {
      const L = layoutRadarPro(), tt = tiempo(), { c, Rr, R0 } = L;
      const barrido = ((tt % PERIODO) / PERIODO) * Math.PI * 2 - Math.PI / 2;
      // The scope's glass: a dark teal disc with a vignette.
      const fondoG = cx.createRadialGradient(c.x, c.y, Rr * 0.05, c.x, c.y, Rr * 1.15);
      fondoG.addColorStop(0, 'rgba(10,40,44,.55)'); fondoG.addColorStop(0.7, 'rgba(5,20,26,.5)'); fondoG.addColorStop(1, 'rgba(4,7,13,0)');
      cx.fillStyle = fondoG; cx.beginPath(); cx.arc(c.x, c.y, Rr * 1.15, 0, 7); cx.fill();
      // Range rings: one per power of ten of bytes, labelled; nearer the centre, more data.
      cx.lineWidth = 1; cx.font = '10px "IBM Plex Mono", monospace'; cx.textAlign = 'left';
      for (let l = Math.ceil(L.lmin); l <= Math.floor(L.lmax); l++) {
        const r = L.radio(Math.pow(10, l));
        cx.strokeStyle = 'rgba(63,224,197,.16)'; cx.beginPath(); cx.arc(c.x, c.y, r, 0, 7); cx.stroke();
        const a = 135 * RAD - Math.PI / 2; cx.fillStyle = 'rgba(63,224,197,.55)'; cx.fillText(etiquetaBytes(l), c.x + Math.cos(a) * r + 4, c.y + Math.sin(a) * r - 3);
      }
      cx.strokeStyle = 'rgba(63,224,197,.35)'; cx.beginPath(); cx.arc(c.x, c.y, Rr, 0, 7); cx.stroke();
      cx.strokeStyle = 'rgba(63,224,197,.08)'; cx.beginPath(); cx.moveTo(c.x - Rr, c.y); cx.lineTo(c.x + Rr, c.y); cx.moveTo(c.x, c.y - Rr); cx.lineTo(c.x, c.y + Rr); cx.stroke();
      // Bearings: a tick every 5°, a number every 30° (000 = the Greenwich meridian, up).
      for (let g = 0; g < 360; g += 5) {
        const a = (g - 90) * RAD, largo = g % 30 === 0 ? 10 : g % 10 === 0 ? 6 : 3;
        cx.strokeStyle = g % 30 === 0 ? 'rgba(63,224,197,.7)' : 'rgba(63,224,197,.35)';
        cx.beginPath(); cx.moveTo(c.x + Math.cos(a) * Rr, c.y + Math.sin(a) * Rr); cx.lineTo(c.x + Math.cos(a) * (Rr + largo), c.y + Math.sin(a) * (Rr + largo)); cx.stroke();
        if (g % 30 === 0) { cx.fillStyle = 'rgba(63,224,197,.75)'; cx.textAlign = 'center'; cx.fillText(String(g).padStart(3, '0'), c.x + Math.cos(a) * (Rr + 22), c.y + Math.sin(a) * (Rr + 22) + 3.5); }
      }
      // Regions as arcs outside the bearings (by longitude: west on the left, east on the right).
      const cajasRegion = [];
      if (!L.estrecho) [[t('pd_america'), 190, 330], [t('pd_europa') + ' · ' + t('pd_africa'), 335, 55], [t('pd_asia') + ' · ' + t('pd_oceania'), 60, 185]].forEach(([n, a0, a1]) => {
        const g0 = (a0 - 90) * RAD, g1 = ((a1 < a0 ? a1 + 360 : a1) - 90) * RAD, rr = Rr + 38;
        cx.strokeStyle = 'rgba(63,224,197,.35)'; cx.beginPath(); cx.arc(c.x, c.y, rr, g0, g1); cx.stroke();
        const gm = (g0 + g1) / 2; cx.fillStyle = 'rgba(170,200,215,.8)'; cx.font = '500 10px "IBM Plex Mono", monospace';
        const lx = c.x + Math.cos(gm) * (rr + 12), ly = c.y + Math.sin(gm) * (rr + 12) + 3, lw = cx.measureText(n.toUpperCase()).width;
        cx.textAlign = 'center'; cx.fillText(n.toUpperCase(), lx, ly);
        cajasRegion.push({ x: lx - lw / 2 - 4, y: ly - 11, w: lw + 8, h: 15 });
      });
      cx.textAlign = 'left';
      // Phosphor: what the sweep lights stays and fades (half-life 1.4 s of real time, whatever the
      // frame rate), like a real scope. The swept sector is filled whole, so a slow frame leaves
      // no gaps.
      const ahora = performance.now();
      const real = Math.min(2, (ahora - (radarAntes || ahora)) / 1000); radarAntes = ahora;
      const f = capaFosforo();
      f.globalCompositeOperation = 'destination-out'; f.fillStyle = `rgba(0,0,0,${1 - Math.pow(0.5, real / 1.4)})`; f.fillRect(0, 0, st.Wc, st.H);
      f.globalCompositeOperation = 'lighter';
      const norm = (x) => ((x % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2);
      const paso = norm(barrido - barridoAntes), seguido = paso > 0 && paso < 1;
      if (seguido) { f.fillStyle = 'rgba(63,224,197,.07)'; f.beginPath(); f.moveTo(c.x, c.y); f.arc(c.x, c.y, Rr, barrido - paso, barrido); f.closePath(); f.fill(); }
      const cruza = (a) => seguido && norm(a - (barrido - paso)) <= paso;
      VISIBLES.forEach((d) => {
        if (!cruza(d.ang)) return;
        d.eco = ahora;
        const bl = bloqueado(d), col = bl ? '255,107,122' : d.doc === false ? '255,209,102' : '120,255,225';
        const r = 3 + Math.min(7, Math.log10(1 + d.sub) * 0.9);
        const gr = f.createRadialGradient(d.x, d.y, 0, d.x, d.y, r * 2.6); gr.addColorStop(0, `rgba(${col},.95)`); gr.addColorStop(0.35, `rgba(${col},.45)`); gr.addColorStop(1, `rgba(${col},0)`);
        f.fillStyle = gr; f.beginPath(); f.arc(d.x, d.y, r * 2.6, 0, 7); f.fill();
      });
      barridoAntes = barrido;
      cx.save(); cx.setTransform(1, 0, 0, 1, 0, 0); cx.globalCompositeOperation = 'lighter'; cx.drawImage(fosforo, 0, 0); cx.restore();
      cx.setTransform(st.dpr, 0, 0, st.dpr, 0, 0);
      // The beam itself, sharp, with a short glow behind it, drawn fresh each frame.
      cx.globalCompositeOperation = 'lighter';
      for (let k = 0; k < 14; k++) { const a0 = barrido - k * 0.02; cx.fillStyle = `rgba(63,224,197,${0.06 * (1 - k / 14)})`; cx.beginPath(); cx.moveTo(c.x, c.y); cx.arc(c.x, c.y, Rr, a0 - 0.021, a0); cx.closePath(); cx.fill(); }
      cx.strokeStyle = 'rgba(140,255,230,.9)'; cx.lineWidth = 1.4; cx.beginPath(); cx.moveTo(c.x, c.y); cx.lineTo(c.x + Math.cos(barrido) * Rr, c.y + Math.sin(barrido) * Rr); cx.stroke();
      cx.globalCompositeOperation = 'source-over';
      // The routes, faint, and the data moving along them.
      cx.globalCompositeOperation = 'lighter';
      VISIBLES.forEach((d) => { const bl = bloqueado(d); trazo(d.ruta, 0, bl ? d.tg : 1, bl ? 'rgba(255,107,122,.3)' : prod(d.p).color + '12', 0.8); });
      cx.globalCompositeOperation = 'source-over';
      escudo(c, R0); ordenador(c, Math.max(13, R0 * 0.3));
      st.dianas.push({ x: c.x, y: c.y, r: R0 * 0.35, tipo: 'yo' });
      L.prods.forEach(nodoProducto);
      particulas(dt);
      VISIBLES.forEach((d) => st.dianas.push({ x: d.x, y: d.y, r: 12, tipo: 'dest', ref: d }));
      // Air-traffic-control tags for the busiest contacts and the stopped ones: they light up as
      // the sweep passes and dim after.
      if (st.W >= 700) {
        const puestos = [{ x: c.x - R0, y: c.y - R0, w: 2 * R0, h: 2 * R0 }, ...cajasRegion];
        VISIBLES.filter((d) => bloqueado(d) || VISIBLES.indexOf(d) < 10).slice(0, 14).forEach((d) => {
          const brillo2 = 0.45 + 0.55 * Math.exp(-((ahora - (d.eco || 0)) / 1000) / 2.2);
          const prog = (prod(d.p).nombre || '').replace(/[^A-Za-z0-9]/g, '').slice(0, 4).toUpperCase();
          const l1 = `${prog} ${(d.destino || d.host).toUpperCase().slice(0, 26)}`;
          const l2 = `${String(Math.round(d.rumbo)).padStart(3, '0')}° · ${d.pais} · ${(d.red && d.red[0] !== '@' ? d.red : '—').toUpperCase().slice(0, 14)}${d.asn ? ' AS' + d.asn : ''}`;
          const l3 = bloqueado(d) ? tn('pd_parados', d.bloq).toUpperCase() : `↑ ${bytes(d.ema || 0)}/S · ${bytes(d.sub)}`;
          cx.font = '500 10px "IBM Plex Mono", monospace';
          const w = Math.max(cx.measureText(l1).width, cx.measureText(l2).width, cx.measureText(l3).width) + 10, h = 40;
          // Outwards from the centre; if the place is taken, a little along the ring; else no tag.
          const ux = Math.cos(d.ang), uy = Math.sin(d.ang), dcha = ux >= 0;
          const choca = (x0, y0) => puestos.some((b) => x0 < b.x + b.w && x0 + w > b.x && y0 < b.y + b.h && y0 + h > b.y) || x0 < 4 || x0 + w > st.W - 4 || y0 < 50 || y0 + h > st.H - 30;
          let bx = 0, by = 0, ok = false;
          for (const [dist, tang] of [[22, 0], [22, 26], [22, -26], [44, 0], [44, 34], [44, -34], [66, 0]]) {
            const ax = d.x + ux * dist - uy * tang, ay = d.y + uy * dist + ux * tang;
            bx = dcha ? ax : ax - w; by = ay - h / 2;
            if (!choca(bx, by)) { ok = true; break; }
          }
          if (!ok) return;
          puestos.push({ x: bx, y: by, w, h });
          const col = bloqueado(d) ? `rgba(255,141,153,${brillo2})` : `rgba(140,255,225,${brillo2})`;
          cx.strokeStyle = col; cx.lineWidth = 1; cx.beginPath(); cx.moveTo(d.x, d.y); cx.lineTo(dcha ? bx : bx + w, by + h / 2); cx.stroke();
          cx.fillStyle = `rgba(4,14,18,${0.55 + 0.25 * brillo2})`; cx.fillRect(bx, by, w, h);
          cx.fillStyle = col; cx.fillText(l1, bx + 5, by + 11); cx.fillStyle = `rgba(170,200,215,${brillo2})`; cx.fillText(l2, bx + 5, by + 23); cx.fillStyle = col; cx.fillText(l3, bx + 5, by + 35);
        });
      }
      // The glass: fine scan lines and a vignette, very light.
      cx.save(); cx.beginPath(); cx.arc(c.x, c.y, Rr, 0, 7); cx.clip();
      cx.fillStyle = patronRayas(); cx.globalAlpha = 0.5; cx.fillRect(c.x - Rr, c.y - Rr, Rr * 2, Rr * 2); cx.globalAlpha = 1;
      const vi = cx.createRadialGradient(c.x, c.y, Rr * 0.6, c.x, c.y, Rr); vi.addColorStop(0, 'rgba(0,0,0,0)'); vi.addColorStop(1, 'rgba(0,0,0,.35)'); cx.fillStyle = vi; cx.fillRect(c.x - Rr, c.y - Rr, Rr * 2, Rr * 2);
      cx.restore();
      // The scope's own readout.
      cx.font = '500 10.5px "IBM Plex Mono", monospace'; cx.fillStyle = 'rgba(140,255,225,.75)'; cx.textAlign = 'center';
      // The scope's readout: centred on top of a narrow screen, in the top right corner otherwise
      // (the top left is the live telemetry's).
      const lectura = partir(t('pd_radar_lectura').replace('{n}', VISIBLES.length).replace('{p}', VISIBLES.filter(bloqueado).length).replace('{s}', PERIODO).toUpperCase(), L.estrecho ? st.W - 24 : st.W / 2 - 90);
      if (!L.estrecho) cx.textAlign = 'right';
      lectura.forEach((l, i) => cx.fillText(l, L.estrecho ? c.x : st.W - 14, (L.estrecho ? 16 : 22) + i * 13));
      cx.textAlign = 'left';
    };

    // ---------- 3. The computer: a Sankey diagram through GUARDIANA's gate ----------
    const banda = (x0, a0, b0, x1, a1, b1) => { const xm = (x0 + x1) / 2; cx.beginPath(); cx.moveTo(x0, a0); cx.bezierCurveTo(xm, a0, xm, a1, x1, a1); cx.lineTo(x1, b1); cx.bezierCurveTo(xm, b1, xm, b0, x0, b0); cx.closePath(); };
    const curvaCentro = (x0, y0, x1, y1, n) => { const out = [], xm = (x0 + x1) / 2; for (let i = 0; i <= n; i++) { const u = i / n, a = (1 - u) ** 3, b = 3 * (1 - u) ** 2 * u, c2 = 3 * (1 - u) * u * u, d2 = u ** 3; out.push({ x: a * x0 + b * xm + c2 * xm + d2 * x1, y: a * y0 + b * y0 + c2 * y1 + d2 * y1 }); } return out; };
    // A label cut to the width it has, with an ellipsis.
    const corta = (txt, ancho) => { if (cx.measureText(txt).width <= ancho) return txt; let r = txt; while (r.length > 1 && cx.measureText(r + '…').width > ancho) r = r.slice(0, -1); return r + '…'; };
    const layoutSankey = () => {
      const W = st.W, H = st.H, estrecho = W < 700, nodoW = estrecho ? 8 : 10;
      // Room on top for the column titles and the gate's figures; under it, for the legend.
      const top = estrecho ? 64 : 86, bot = H - (estrecho ? 70 : 72), alto = Math.max(60, bot - top);
      const ps = prodsVisibles();
      // A narrow screen keeps three columns (programs, gate, country of the server); the network's
      // company is in each band's tip and in «Datos en vivo».
      let col;
      if (estrecho) {
        cx.font = '600 11px "IBM Plex Sans", sans-serif';
        const izq = Math.min(W * 0.3, Math.max(48, ...ps.map((p) => cx.measureText(p.nombre).width))) + 12;
        const pais = W - Math.min(W * 0.3, 100) - nodoW - 6;
        col = { prog: izq, puerta: izq + nodoW + (pais - izq - nodoW) * 0.45, pais, red: null };
      } else col = { prog: Math.max(150, W * 0.14), puerta: W * 0.38, pais: W * 0.62, red: W * 0.84 };
      const pasan = VISIBLES.filter((d) => !bloqueado(d) && d.sub > 0), paran = VISIBLES.filter((d) => bloqueado(d));
      // Countries and networks beyond the busiest go together, so it stays readable.
      const totP = {}, totR = {};
      pasan.forEach((d) => { totP[d.pais] = (totP[d.pais] || 0) + d.sub; const r = d.red && d.red[0] !== '@' ? d.red : '—'; totR[r] = (totR[r] || 0) + d.sub; });
      const paises = Object.keys(totP).sort((a, b) => totP[b] - totP[a]), redes = Object.keys(totR).sort((a, b) => totR[b] - totR[a]);
      const maxPaises = estrecho ? 7 : 11;
      const OTROS = '…', paisDe = (d) => (paises.indexOf(d.pais) < maxPaises ? d.pais : OTROS), redDe = (d) => { const r = d.red && d.red[0] !== '@' ? d.red : '—'; return redes.indexOf(r) < 13 ? r : OTROS; };
      const nodosPais = [...new Set(pasan.map(paisDe))].sort((a, b) => (a === OTROS) - (b === OTROS) || (totP[b] || 0) - (totP[a] || 0));
      const nodosRed = col.red === null ? [] : [...new Set(pasan.map(redDe))].sort((a, b) => (a === OTROS) - (b === OTROS) || (totR[b] || 0) - (totR[a] || 0));
      const total = Math.max(1, pasan.reduce((a, d) => a + d.sub, 0));
      const parPx = (d) => Math.min(10, 2 + 1.6 * Math.log2(1 + d.bloq));
      const stopTot = paran.reduce((a, d) => a + parPx(d), 0);
      const gap = estrecho ? 7 : 10, nMax = Math.max(ps.length, nodosPais.length, nodosRed.length);
      const k = Math.max(0.000001, (alto - gap * (nMax - 1) - stopTot) / total * 0.92);
      const alturaDe = (v) => Math.max(2, v * k);
      // Programs: what passes, then what was stopped, stacked. Each keeps room for its label: two
      // lines (name and bytes), or only the name when a phone has no room for more.
      const nodoProg = {}; let y = top;
      ps.forEach((p) => {
        const v = pasan.filter((d) => d.p === p.id).reduce((a, d) => a + d.sub, 0), st2 = paran.filter((d) => d.p === p.id).reduce((a, d) => a + parPx(d), 0);
        const h = (v ? alturaDe(v) : 0) + st2 || 6, lineas = !estrecho || h + gap >= 28 ? 2 : 1;
        nodoProg[p.id] = { p, x: col.prog, y, h, v, cur: y, lineas }; y += Math.max(h + gap, lineas === 2 ? 30 : 15);
      });
      // Countries and networks, centred, each with room for its one-line label.
      const colocar = (lista, tot, x) => {
        const out = {}, alt = (n) => alturaDe(tot[n] || 0), paso = (n) => Math.max(alt(n) + gap, 14);
        const usado = lista.reduce((a, n) => a + paso(n), 0) - gap;
        let y2 = top + Math.max(0, (alto - usado) / 2);
        lista.forEach((n) => { const h = alt(n); out[n] = { n, x, y: y2, h, cur: y2, curIn: y2 }; y2 += paso(n); });
        return out;
      };
      const tP = {}; pasan.forEach((d) => { const n = paisDe(d); tP[n] = (tP[n] || 0) + d.sub; });
      const tR = {}; pasan.forEach((d) => { const n = redDe(d); tR[n] = (tR[n] || 0) + d.sub; });
      const nodoPais = colocar(nodosPais, tP, col.pais), nodoRed = col.red === null ? {} : colocar(nodosRed, tR, col.red);
      // The gate: everything that leaves the programs arrives here; what passes goes on.
      let yPuerta = top + Math.max(0, (alto - (total * k + stopTot + gap * Math.max(0, ps.length - 1))) / 2);
      const bandas = [], mid = (y0, hh) => y0 + hh / 2;
      VISIBLES.forEach((d) => { d.vis = false; d.ruta = []; });
      ps.forEach((p) => {
        const np = nodoProg[p.id];
        // Passing flows of this program, grouped by country (fewer crossings).
        const mias = pasan.filter((d) => d.p === p.id).sort((a, b) => nodosPais.indexOf(paisDe(a)) - nodosPais.indexOf(paisDe(b)) || nodosRed.indexOf(redDe(a)) - nodosRed.indexOf(redDe(b)));
        mias.forEach((d) => {
          const h = d.sub * k, a0 = np.cur, g0 = yPuerta; np.cur += h; yPuerta += h;
          const npais = nodoPais[paisDe(d)], c0 = npais.curIn; npais.curIn += h;
          const tramos = [[col.prog + nodoW, a0, col.puerta - 3, g0, h], [col.puerta + 3, g0, col.pais, c0, h]];
          d.ruta = curvaCentro(col.prog + nodoW, mid(a0, h), col.puerta, mid(g0, h), 12).concat(curvaCentro(col.puerta, mid(g0, h), col.pais, mid(c0, h), 18).slice(1));
          if (col.red !== null) {
            const nred = nodoRed[redDe(d)], r0 = nred.curIn, s0 = npais.cur; nred.curIn += h; npais.cur += h;
            tramos.push([col.pais + nodoW, s0, col.red, r0, h]);
            d.ruta = d.ruta.concat(curvaCentro(col.pais + nodoW, mid(s0, h), col.red, mid(r0, h), 18).slice(1));
          }
          bandas.push({ tipo: 'pasa', d, col: p.color, tramos });
          d.tg = 12 / (d.ruta.length - 1); d.vis = true; const fin = d.ruta[d.ruta.length - 1]; d.x = fin.x; d.y = fin.y;
        });
        // Stopped flows end at the gate.
        paran.filter((d) => d.p === p.id).forEach((d) => {
          const h = parPx(d), a0 = np.cur, g0 = yPuerta; np.cur += h; yPuerta += h;
          bandas.push({ tipo: 'para', d, col: '#ff6b7a', tramos: [[col.prog + nodoW, a0, col.puerta - 3, g0, h]] });
          d.ruta = curvaCentro(col.prog + nodoW, a0 + h / 2, col.puerta - 2, g0 + h / 2, 20); d.tg = 0.98; d.vis = true; d.x = col.puerta - 2; d.y = g0 + h / 2;
        });
        yPuerta += gap;
      });
      return { col, nodoW, nodoProg, nodoPais, nodoRed, bandas, top, bot, total, OTROS, estrecho };
    };
    const dibujaSankey = (dt) => {
      const L = layoutSankey(), tt = tiempo(), hov = st.hover, W = st.W;
      // Column titles (on a phone: programs over their names, the country at the right edge).
      const yT = L.estrecho ? 20 : 46;
      cx.font = '500 10.5px "IBM Plex Mono", monospace'; cx.fillStyle = 'rgba(160,185,210,.75)';
      (L.estrecho
        ? [[t('pd_col_programas'), L.col.prog + L.nodoW, 'right'], ['GUARDIANA', L.col.puerta, 'center'], [t('pd_col_pais'), W - 6, 'right']]
        : [[t('pd_col_programas'), L.col.prog, 'left'], ['GUARDIANA', L.col.puerta, 'center'], [t('pd_col_pais'), L.col.pais, 'left'], [t('pd_col_red'), L.col.red, 'left']]
      ).forEach(([n, x, al]) => { cx.textAlign = al; cx.fillText(n.toUpperCase(), x, yT); });
      cx.textAlign = 'left';
      // Bands: a gradient from the program's colour to the country's.
      cx.globalCompositeOperation = 'lighter';
      L.bandas.forEach((b) => {
        const resaltada = hov && hov.ref === b.d;
        b.tramos.forEach(([x0, y0, x1, y1, h], i) => {
          const hh = Math.max(1, h);
          const g = cx.createLinearGradient(x0, 0, x1, 0);
          const c1 = b.tipo === 'para' ? '#ff6b7a' : i < 2 ? b.col : `hsl(${tono(b.d.pais)},70%,62%)`, c2 = b.tipo === 'para' ? '#ff6b7a' : i === 0 ? b.col : `hsl(${tono(b.d.pais)},70%,62%)`;
          const al = resaltada ? 'aa' : b.tipo === 'para' ? '55' : '38';
          g.addColorStop(0, (c1.startsWith('#') ? c1 + al : c1.replace('hsl', 'hsla').replace(')', `,${parseInt(al, 16) / 255})`))); g.addColorStop(1, (c2.startsWith('#') ? c2 + al : c2.replace('hsl', 'hsla').replace(')', `,${parseInt(al, 16) / 255})`)));
          cx.fillStyle = g; banda(x0, y0, y0 + hh, x1, y1, y1 + hh); cx.fill();
        });
      });
      cx.globalCompositeOperation = 'source-over';
      // GUARDIANA's gate: a column of light, with what it let through and what it stopped.
      const pg = cx.createLinearGradient(L.col.puerta - 16, 0, L.col.puerta + 16, 0);
      pg.addColorStop(0, 'rgba(63,224,197,0)'); pg.addColorStop(0.5, `rgba(63,224,197,${0.28 + 0.08 * Math.sin(tt * 2)})`); pg.addColorStop(1, 'rgba(63,224,197,0)');
      cx.fillStyle = pg; cx.fillRect(L.col.puerta - 16, L.top - 6, 32, L.bot - L.top + 12);
      cx.fillStyle = '#3fe0c5'; cx.fillRect(L.col.puerta - 1.5, L.top - 6, 3, L.bot - L.top + 12);
      const parados = VISIBLES.filter(bloqueado);
      cx.font = '10px "IBM Plex Mono", monospace'; cx.textAlign = 'center'; cx.fillStyle = 'rgba(140,255,225,.85)';
      cx.fillText(t('pd_puerta_pasa').replace('{x}', bytes(L.total)), L.col.puerta, yT + 14);
      cx.fillStyle = 'rgba(255,141,153,.9)'; cx.fillText(tn('pd_parados', parados.reduce((a, d) => a + d.bloq, 0)), L.col.puerta, yT + 26);
      cx.textAlign = 'left';
      L.bandas.filter((b) => b.tipo === 'para').forEach((b) => { const [, , x1, y1, h] = b.tramos[0]; cx.fillStyle = '#ff6b7a'; cx.fillRect(x1 - 1, y1, 4, Math.max(2, h)); });
      // Nodes and their labels, each cut to the room it has.
      Object.values(L.nodoProg).forEach((n) => {
        cx.fillStyle = n.p.color; cx.fillRect(n.x, n.y, L.nodoW, n.h);
        st.dianas.push({ x: n.x + L.nodoW / 2, y: n.y + n.h / 2, r: Math.max(12, n.h / 2), tipo: 'prod', ref: n.p });
        const yl = n.y + Math.min(n.h, n.lineas === 2 ? 28 : 12) / 2 + (n.lineas === 2 ? 1 : 4);
        cx.textAlign = 'right'; cx.font = `600 ${L.estrecho ? 11 : 12}px "IBM Plex Sans", sans-serif`; cx.fillStyle = '#e3eef6'; cx.fillText(corta(n.p.nombre, n.x - 12), n.x - 8, yl);
        if (n.lineas === 2) { cx.font = '10px "IBM Plex Mono", monospace'; cx.fillStyle = 'rgba(160,185,210,.85)'; cx.fillText(`↑ ${bytes(n.p.sub)}`, n.x - 8, yl + 13); }
        cx.textAlign = 'left';
      });
      const etiqueta = (n, txt) => { cx.font = '500 11px "IBM Plex Sans", sans-serif'; cx.fillStyle = '#e3eef6'; cx.fillText(corta(txt, W - (n.x + L.nodoW + 6) - 4), n.x + L.nodoW + 6, n.y + Math.min(n.h, 26) / 2 + 4); };
      Object.values(L.nodoPais).forEach((n) => {
        cx.fillStyle = n.n === L.OTROS ? '#55657a' : `hsl(${tono(n.n)},70%,62%)`; cx.fillRect(n.x, n.y, L.nodoW, n.h);
        if (n.n !== L.OTROS) st.dianas.push({ x: n.x + L.nodoW / 2, y: n.y + n.h / 2, r: Math.max(12, n.h / 2), tipo: 'pais', ref: n.n });
        etiqueta(n, n.n === L.OTROS ? t('pd_otros_paises') : nombrePais(n.n));
      });
      Object.values(L.nodoRed).forEach((n) => {
        cx.fillStyle = n.n === L.OTROS ? '#55657a' : '#6fb6ff'; cx.fillRect(n.x, n.y, L.nodoW, n.h);
        if (n.n !== L.OTROS && n.n !== '—') st.dianas.push({ x: n.x + L.nodoW / 2, y: n.y + n.h / 2, r: Math.max(12, n.h / 2), tipo: 'red', ref: n.n });
        etiqueta(n, n.n === L.OTROS ? t('pd_otras_redes') : n.n === '—' ? t('pd_red_desconocida') : n.n);
      });
      particulas(dt);
      VISIBLES.forEach((d) => { if (d.vis && d.ruta.length) { const q = d.ruta[Math.min(d.ruta.length - 1, Math.round(d.ruta.length * 0.75))]; st.dianas.push({ x: q.x, y: q.y, r: 8, tipo: 'dest', ref: d }); } });
    };

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
      const mx = Math.max(1, ...st.serie);
      g.strokeStyle = 'rgba(118,144,168,.18)'; g.lineWidth = 1;
      for (let k = 1; k < 4; k++) { g.beginPath(); g.moveTo(0, h * k / 4); g.lineTo(w, h * k / 4); g.stroke(); }
      const n = st.serie.length, X = (i) => i / (n - 1) * w, Y = (v) => h - 3 - v / mx * (h - 9);
      let base = new Array(n).fill(0);
      prodsVisibles().forEach((p) => {
        const serie = st.serieP[p.id] || [], tope = base.map((b, i) => b + (serie[i] || 0));
        g.beginPath(); tope.forEach((v, i) => (i ? g.lineTo(X(i), Y(v)) : g.moveTo(X(i), Y(v))));
        for (let i = n - 1; i >= 0; i--) g.lineTo(X(i), Y(base[i]));
        g.closePath(); g.fillStyle = p.color + '55'; g.fill();
        g.beginPath(); tope.forEach((v, i) => (i ? g.lineTo(X(i), Y(v)) : g.moveTo(X(i), Y(v)))); g.strokeStyle = p.color; g.lineWidth = 1.1; g.stroke();
        base = tope;
      });
    };
    const indicador = (f) => {
      const { g, w, h } = prepara($('indicador'));
      if (w < 40 || h < 30) return;
      const r = Math.min(w / 2 - 6, h - 10), c = { x: w / 2, y: h - 4 };
      g.lineWidth = 7; g.lineCap = 'round';
      g.strokeStyle = 'rgba(118,144,168,.22)'; g.beginPath(); g.arc(c.x, c.y, r, Math.PI, 2 * Math.PI); g.stroke();
      if (f !== null) { g.strokeStyle = '#3fe0c5'; g.beginPath(); g.arc(c.x, c.y, r, Math.PI, Math.PI + Math.PI * f); g.stroke(); }
      g.fillStyle = '#d3e6f3'; g.font = '600 17px "IBM Plex Mono", monospace'; g.textAlign = 'center'; g.fillText(f === null ? '—' : Math.round(f * 100) + '%', c.x, c.y - 6);
    };
    const pintaHud = () => {
      $('registro').innerHTML = st.eventos.slice(0, 9).map((e) => `<li><span class="t">${esc(e.h)}</span> ${esc(e.prog)} → ${esc(e.host)} <span class="${e.cls}">${esc(e.que)}</span></li>`).join('') || `<li class="t">${esc(t('pd_registro_vacio'))}</li>`;
      $('hud-min').textContent = bytes(st.serie.reduce((a, b) => a + b, 0));
      const pp = {}; DEST.forEach((d) => { if (d.pais && d.pais[0] !== '@' && !d.sede) pp[d.pais] = (pp[d.pais] || 0) + d.sub; });
      const top = Object.entries(pp).filter((x) => x[1] > 0).sort((a, b) => b[1] - a[1]).slice(0, 5), mx = Math.max(1, ...top.map((x) => x[1]));
      $('barras').innerHTML = top.map(([c, v]) => `<div><span>${esc(nombrePais(c))}</span><s data-w="${Math.max(2, v / mx * 100)}"></s><span>${esc(bytes(v))}</span></div>`).join('') || `<div class="t">—</div>`;
      estilos($('barras'));
      $('hud-cortes').textContent = num(DEST.filter((d) => d.cortado).length + PROD.filter((p) => p.cortado).length);
      $('hud-bloqueos').textContent = tn('pd_intentos', DEST.reduce((a, d) => a + d.bloq, 0));
      const av = DEST.filter((d) => d.doc !== null && d.doc !== undefined), tot = av.reduce((a, d) => a + d.sub, 0), doc = av.filter((d) => d.doc).reduce((a, d) => a + d.sub, 0);
      sparkline(); indicador(tot ? doc / tot : null);
      $('hud-doc').textContent = tot ? t('pd_doc_explica').replace('{n}', pct(doc, tot)).replace('{m}', 100 - pct(doc, tot)) : t('pd_doc_nada');
      const corte = PROD.filter((p) => p.cortable);
      $('maxima').setAttribute('aria-pressed', String(corte.length > 0 && corte.every((p) => p.cortado)));
      $('maxima').hidden = !corte.length;
    };

    // ---------- «Datos en vivo» ----------
    const panelDatos = $('datos');
    let datosAbierto = true;
    const ponDatos = (on) => { datosAbierto = on; panelDatos.hidden = !on; $('btn-datos').setAttribute('aria-pressed', String(on)); try { localStorage.setItem('guardiana_pd_datos', on ? '1' : '0'); } catch (_) {} pintaDatos(); };
    $('btn-datos').addEventListener('click', () => ponDatos(!datosAbierto));
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
    const htmlDirecto = () => {
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
      }
      return h + '</div>';
    };
    const tipoTexto = (p) => t('pd_tipo_' + p.tipo);
    const pintaDatos = () => {
      if (!datosAbierto) return;
      const ahoraMs = performance.now();
      const tot = DEST.reduce((a, d) => a + d.sub, 0), baj = DEST.reduce((a, d) => a + d.baj, 0), paq = DEST.reduce((a, d) => a + d.paq, 0);
      let h = `<h4>${esc(t('pd_en_directo'))}${st.demo ? ` <span class="chip">${esc(t('pd_chip_demo'))}</span>` : ''}</h4><div>${htmlDirecto()}</div>`;
      // Programs.
      h += `<h4>${esc(t('pd_programas'))} <span class="s">${PROD.length}</span></h4>`;
      PROD.forEach((p) => {
        const ds = DEST.filter((d) => d.p === p.id), act = ds.filter((d) => d.ult && ahoraMs - d.ult < 15000 && !bloqueado(d)).length;
        const avs = ds.filter((d) => d.doc !== null && d.doc !== undefined), su = avs.reduce((a, d) => a + d.sub, 0), doc = avs.filter((d) => d.doc).reduce((a, d) => a + d.sub, 0);
        const chip = p.cortado ? `<span class="chip c">${esc(t('pd_chip_cortado'))}</span>` : `<span class="chip">${esc(t('pd_chip_pasa'))}</span>`;
        h += `<div class="prod" data-c="${p.color}"><div class="fila"><span><b>${esc(p.nombre)}</b> <span class="t">${esc(tipoTexto(p))}</span></span>${chip}</div>
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
        h += `<div class="fila"><span title="${esc(d.host)}">${esc(d.host)}${nuevo ? ` <span class="chip">${esc(t('pd_chip_nuevo'))}</span>` : ''}</span><span class="${bl ? 'c' : 'o'}">${bl || (!d.sub && d.bloq) ? esc(tn('pd_parados', d.bloq)) : esc(bytes(d.sub))}</span></div>
          <div class="barrita"><i data-w="${bl ? 100 : Math.max(2, d.sub / mx * 100)}" data-bg="${bl ? 'rgba(255,107,122,.5)' : prod(d.p).color}"></i></div>`;
      });
      if (st.datos && st.datos.destinos_omitidos) h += `<div class="t">${esc(tn('pd_omitidos', st.datos.destinos_omitidos))}</div>`;
      // Companies that own the networks.
      const redes = {}; DEST.filter((d) => !d.sede && d.red && d.red[0] !== '@').forEach((d) => { const k = d.red; redes[k] = redes[k] || { b: 0, asn: d.asn, n: 0 }; redes[k].b += d.sub + d.baj; redes[k].n++; });
      const rr = Object.entries(redes).sort((a, b) => b[1].b - a[1].b).slice(0, 15), mr = Math.max(1, ...rr.map((x) => x[1].b));
      h += `<h4>${esc(t('pd_empresas_redes'))} <span class="s">${num(Object.keys(redes).length)}</span></h4>`;
      rr.forEach(([n, r]) => { h += `<div class="fila"><span>${esc(n)} <span class="t">${r.asn ? 'AS' + r.asn : ''} · ${esc(tn('pd_nombres', r.n))}</span></span><span>${esc(bytes(r.b))}</span></div><div class="barrita"><i data-w="${Math.max(2, r.b / mr * 100)}" data-bg="var(--in)"></i></div>`; });
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
        <div>TCP ${pct(tcp, tot)} % · UDP/QUIC ${pct(udp, tot)} %</div>
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
      h += `<h4>${esc(st.demo ? t('pd_sesion_demo') : t('pd_ventana').replace('{h}', st.datos ? st.datos.ventana_h : 24))}</h4>
        <div><span class="o">↑ ${esc(bytes(tot))}</span> · <span class="i">↓ ${esc(bytes(baj))}</span> · ${esc(t('pd_madrugada').replace('{x}', bytes(noche)))}</div>
        <div>${esc(t('pd_abierto').replace('{t}', hace(Date.now() - st.abierto)))} · <span class="c">${esc(tn('pd_intentos', DEST.reduce((a, d) => a + d.bloq, 0)))}</span></div>`;
      if (st.datos && st.datos.programas_omitidos) h += `<div class="t">${esc(tn('pd_programas_omitidos', st.datos.programas_omitidos))}</div>`;
      panelDatos.innerHTML = h;
      estilos(panelDatos);
    };
    const telemetria = () => {
      const el = $('telemetria');
      el.hidden = st.vista === 'dentro' || datosAbierto || st.Wc < 700;
      if (!el.hidden) el.innerHTML = htmlDirecto();
    };

    // ---------- What is under the pointer ----------
    const queHay = (x, y) => {
      let mejor = null, dm = Infinity;
      st.dianas.forEach((d) => { const dd = Math.hypot(d.x - x, d.y - y); if (dd < d.r && dd < dm) { dm = dd; mejor = d; } });
      if (mejor) return mejor;
      if (enTierra()) { const c = paisBajo(x, y); return c ? { tipo: 'pais', ref: c } : null; }
      return null;
    };
    const usoTexto = (d) => (d.doc === null || d.doc === undefined ? '' : d.uso ? `<div>${esc(t('pd_para_que'))} <b>${esc(t('av_uso_' + d.uso))}</b></div>` : `<div class="nodoc">${esc(t('pd_no_documenta'))}</div>`);
    const textoTip = (hh) => {
      if (hh.tipo === 'dest') {
        const d = hh.ref, p = prod(d.p), bl = bloqueado(d);
        const estado = bl ? `<span class="estado corta">${esc(d.cortado ? t('pd_cortado_nombre') : d.ancho ? t('pd_cortado_ancho') : t('pd_cortado_programa').replace('{prog}', p.nombre))}</span>` : `<span class="estado pasa">${esc(t('pd_pasa'))}</span>`;
        const dia = centros[d.pais] ? esDeDia(centros[d.pais].lon, centros[d.pais].lat) : null;
        const donde = d.sede ? t('pd_sede').replace('{pais}', nombrePais(d.pais)).replace('{empresa}', d.red || '—') : t('pd_servidor').replace('{pais}', d.pais ? nombrePais(d.pais) : '—').replace('{red}', d.red || '—');
        const accion = !d.destino ? t('pd_sin_nombre_no_corta') : d.ancho ? t('pd_ancho_dejar') : d.cortado ? t('pd_clic_dejar') : t('pd_clic_cortar');
        return `<div class="mono">${esc(d.host)}</div><div>${esc(p.nombre)} · <span class="mono">${esc(p.procesos.join(' · '))}</span></div>${usoTexto(d)}
          <div>${esc(donde)}${d.asn ? ` <span class="mono">AS${d.asn}</span>` : ''}</div>
          ${d.proto ? `<div>${esc((d.vias && d.vias.length ? d.vias : [d.proto + '/' + d.puerto]).join(' · '))}${d.puerto === 443 ? ' ' + esc(t('pd_cifrado_corto')) : d.puerto === 80 ? ' ' + esc(t('pd_sin_cifrar_corto')) : ''} · ${esc(tn('pd_paquetes_n', d.paq))}</div>` : ''}
          <div>${esc(t('pd_enviado'))} <b>${esc(bytes(d.sub))}</b> · ${esc(t('pd_recibido'))} ${esc(bytes(d.baj))}${d.consultas ? ' · ' + esc(tn('pd_consultas', d.consultas)) : ''}${d.bloq ? ' · ' + esc(tn('pd_parados', d.bloq)) : ''}</div>
          <div class="t">${d.primera ? esc(t('pd_primera').replace('{t}', hace(Date.now() - d.primera))) : ''}${dia === null ? '' : ' · ' + esc(t(dia ? 'pd_alli_dia' : 'pd_alli_noche'))}</div>${estado}
          <div class="t">${esc(accion)}</div>`;
      }
      if (hh.tipo === 'prod') {
        const p = hh.ref, ds = DEST.filter((d) => d.p === p.id);
        return `<b>${esc(p.nombre)}</b> · <span class="t">${esc(tipoTexto(p))}</span><div class="mono">${esc(p.procesos.join(' · '))}</div>
          <div>${esc(tn('pd_destinos_en', ds.length).replace('{p}', new Set(ds.map((d) => d.pais).filter((x) => x && x[0] !== '@')).size))}</div>
          <div>${esc(t('pd_enviado'))} <b>${esc(bytes(p.sub))}</b> · ${esc(t('pd_recibido'))} ${esc(bytes(p.baj))}</div>
          ${p.cortado ? `<span class="estado corta">${esc(t('pd_prog_cortado'))}</span>` : `<span class="estado pasa">${esc(t('pd_prog_pasa'))}</span>`}
          <div class="t">${esc(p.cortable ? (p.cortado ? t('pd_clic_dejar_prog') : t('pd_clic_cortar_prog')) : t('pd_no_cortable'))}</div>`;
      }
      if (hh.tipo === 'red') {
        const r = hh.ref, ds = DEST.filter((d) => d.red === r && !d.sede), asn = (ds.find((d) => d.asn) || {}).asn;
        const paises = [...new Set(ds.map((d) => d.pais).filter((x) => x && x[0] !== '@'))].map(nombrePais).join(', ');
        return `<b>${esc(r)}</b>${asn ? ` <span class="mono">AS${asn}</span>` : ''}<div>${esc(t('pd_red_paises'))} <b>${esc(paises || '—')}</b></div>
          <div>${esc(t('pd_enviado'))} <b>${esc(bytes(ds.reduce((a, d) => a + d.sub, 0)))}</b> · ${esc(t('pd_recibido'))} ${esc(bytes(ds.reduce((a, d) => a + d.baj, 0)))}</div>
          <div class="mono arriba4">${ds.slice(0, 10).map((d) => '· ' + esc(d.host)).join('<br>')}${ds.length > 10 ? '<br>…' : ''}</div>`;
      }
      if (hh.tipo === 'yo') { const ce = st.lugar, por = ce.fuente === 'zona' ? t('pd_lugar_zona').replace('{ciudad}', ce.ciudad) : t('pd_lugar_pais'); return `<b>${esc(t('pd_tu_ordenador'))}</b> · ${esc(nombrePais(st.yo))} · ${esc(t(esDeDia(ce.lon, ce.lat) ? 'pd_de_dia' : 'pd_de_noche'))}<div class="t">${esc(por)}</div><div>${esc(tn('pd_vigila_n', PROD.length))}</div>`; }
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
      cv.style.cursor = hh && (hh.tipo === 'dest' || hh.tipo === 'prod') ? 'pointer' : st.vista === 'globo' || nebulosaGL() ? 'grab' : 'crosshair';
      if (!hh) { tip.hidden = true; st.hover = null; return; }
      const html = textoTip(hh);
      if (tip.innerHTML !== html) { tip.innerHTML = html; estilos(tip); }
      tip.hidden = false;
      const tw = tip.offsetWidth, th = tip.offsetHeight;
      tip.style.left = Math.min(st.Wc - tw - 8, x + 16) + 'px';
      tip.style.top = Math.min(st.H - th - 8, Math.max(8, y + 14)) + 'px';
      st.hover = hh;
    };

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
      if (!d.destino) { avisa(t('pd_sin_nombre_no_corta')); return; }
      if (d.ancho) { avisa(t('pd_ancho_dejar')); return; }
      if (st.demo) { d.cortado = !d.cortado; avisa((d.cortado ? t('pd_hecho_cortar') : t('pd_hecho_dejar')).replace('{nombre}', d.host) + ' ' + t('pd_demo_simulacion')); pintaHud(); return; }
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
      if (!p.cortable) { avisa(t('pd_no_cortable')); return; }
      const hacer = async () => {
        if (st.demo) { p.cortado = !p.cortado; avisa((p.cortado ? t('pd_hecho_cortar_prog') : t('pd_hecho_dejar_prog')).replace('{prog}', p.nombre) + ' ' + t('pd_demo_simulacion')); pintaHud(); return; }
        try { await api('/api/antivirus/cortar', { method: 'POST', body: { producto: p.id, cortar: !p.cortado } }); avisa((!p.cortado ? t('pd_hecho_cortar_prog') : t('pd_hecho_dejar_prog')).replace('{prog}', p.nombre)); await cargar(); } catch (e) { avisa(String(e && e.message || e)); }
      };
      if (p.cortado) { hacer(); return; }
      if (!st.demo && st.datos && !st.datos.cortar_disponible) { avisa(t('av_cortar_no_disponible')); return; }
      avisa(t('pd_aviso_cortar_prog').replace('{prog}', p.nombre), [{ texto: t('pd_cortar_igual'), peligro: true, hacer }, { texto: t('pd_cancelar'), hacer: () => {} }]);
    };
    $('maxima').addEventListener('click', () => {
      const cortables = PROD.filter((p) => p.cortable), activa = cortables.length > 0 && cortables.every((p) => p.cortado);
      const hacer = async () => {
        if (st.demo) { cortables.forEach((p) => { p.cortado = !activa; }); avisa(t(activa ? 'pd_maxima_off' : 'pd_maxima_on') + ' ' + t('pd_demo_simulacion')); pintaHud(); return; }
        try { await api('/api/antivirus/maxima', { method: 'POST', body: { activa: !activa } }); avisa(t(activa ? 'pd_maxima_off' : 'pd_maxima_on')); await cargar(); } catch (e) { avisa(String(e && e.message || e)); }
      };
      if (activa) { hacer(); return; }
      if (!st.demo && st.datos && !st.datos.cortar_disponible) { avisa(t('av_cortar_no_disponible')); return; }
      avisa(t('av_max_aviso'), [{ texto: t('av_max_confirmar'), peligro: true, hacer }, { texto: t('pd_cancelar'), hacer: () => {} }]);
    });

    // ---------- Pointer and keys ----------
    ['pointermove', 'pointerdown', 'wheel', 'keydown'].forEach((ev) => window.addEventListener(ev, () => { st.ultimoGesto = performance.now(); }, { passive: true }));
    cv.addEventListener('pointermove', (e) => {
      const r = cv.getBoundingClientRect(); st.raton = { x: e.clientX - r.left, y: e.clientY - r.top };
      if (st.arrastre) { st.dRot += (e.clientX - st.arrastre.x) * 0.006; st.dTilt -= (e.clientY - st.arrastre.y) * 0.004; st.arrastre = { x: e.clientX, y: e.clientY, mov: true }; }
    });
    cv.addEventListener('wheel', (e) => { if (!enTierra() && !nebulosaGL()) return; e.preventDefault(); T.zoom = Math.max(0.7, Math.min(3.2, T.zoom * Math.exp(-e.deltaY * 0.0012))); }, { passive: false });
    cv.addEventListener('pointerleave', () => { st.raton = null; tip.hidden = true; });
    cv.addEventListener('pointerdown', (e) => { if (st.vista === 'globo' || nebulosaGL()) { st.arrastre = { x: e.clientX, y: e.clientY, mov: false }; cv.setPointerCapture(e.pointerId); } });
    cv.addEventListener('pointerup', (e) => {
      const movido = st.arrastre && st.arrastre.mov; st.arrastre = null;
      if (movido) return;
      const r = cv.getBoundingClientRect();
      const hh = queHay(e.clientX - r.left, e.clientY - r.top);
      if (!hh) return;
      if (hh.tipo === 'dest') corteNombre(hh.ref);
      if (hh.tipo === 'prod') corteProducto(hh.ref);
    });
    const AYUDA = { globo: 'pd_ayuda_globo', mapa: 'pd_ayuda_mapa', nebulosa: 'pd_ayuda_nebulosa', dentro: 'pd_ayuda_dentro', radar: 'pd_ayuda_radar' };
    const VISTAS = ['globo', 'mapa', 'nebulosa', 'dentro', 'radar'];
    const ponVista = (v) => {
      const antesT = st.vista === 'globo' || st.vista === 'mapa', ahoraT = v === 'globo' || v === 'mapa';
      if (!(antesT && ahoraT)) { st.part = []; st.estallidos = []; }
      st.vista = v;
      document.querySelectorAll('.pestanas button').forEach((b) => {
        b.setAttribute('aria-pressed', String(b.dataset.vista === v));
        // On a phone the tabs slide sideways: the chosen one comes into sight.
        const fila = b.parentElement;
        if (b.dataset.vista === v && fila.scrollWidth > fila.clientWidth) fila.scrollLeft = Math.max(0, b.offsetLeft - fila.offsetLeft - (fila.clientWidth - b.offsetWidth) / 2);
      });
      $('ayuda-vista').textContent = t(AYUDA[v]) + ((v === 'globo' || v === 'mapa') && !T.ok ? ' ' + t('pd_sin_webgl') : '');
      // The help says what to do and then gets out of the way.
      $('ayuda-vista').classList.remove('oculta'); clearTimeout(ponVista.ayuda); ponVista.ayuda = setTimeout(() => $('ayuda-vista').classList.add('oculta'), 12000);
      try { localStorage.setItem('guardiana_pd_vista', v); } catch (_) {}
    };
    document.querySelectorAll('.pestanas button').forEach((b) => b.addEventListener('click', () => ponVista(b.dataset.vista)));
    const pantalla = (on) => { stage.classList.toggle('full', on); $('pantalla').textContent = t(on ? 'pd_cerrar' : 'pd_pantalla_completa'); requestAnimationFrame(ajusta); };
    $('pantalla').addEventListener('click', () => { if (document.fullscreenElement) { document.exitFullscreen().catch(() => {}); return; } if (stage.requestFullscreen) stage.requestFullscreen().catch(() => pantalla(!stage.classList.contains('full'))); else pantalla(!stage.classList.contains('full')); });
    document.addEventListener('fullscreenchange', () => { $('pantalla').textContent = t(document.fullscreenElement ? 'pd_cerrar' : 'pd_pantalla_completa'); requestAnimationFrame(ajusta); });
    document.addEventListener('keydown', (e) => {
      if (/select|input|textarea/i.test(document.activeElement.tagName)) return;
      if (['1', '2', '3', '4', '5'].includes(e.key)) ponVista(VISTAS[+e.key - 1]);
    });
    $('demo').addEventListener('click', () => empiezaDemo());
    // Where the computer is: said, not chosen.
    $('yo').textContent = nombrePais(st.yo) + (st.lugar.ciudad ? ' · ' + st.lugar.ciudad : '');

    // ---------- The loop ----------
    let antes = performance.now(), ultimaTele = 0, ultimoDemo = 0;
    const bucle = (ahora) => {
      requestAnimationFrame(bucle);
      // To keep it open for hours: 30 frames a second, 20 after two quiet minutes, 60 while dragging.
      const quieto = ahora - st.ultimoGesto > 120000;
      if (!st.arrastre && ahora - antes < (quieto ? 48 : 31)) return;
      const dt = Math.min(0.06, (ahora - antes) / 1000); antes = ahora;
      try {
        if (st.demo && ahora - ultimoDemo > 900) { ultimoDemo = ahora; latidoDemo(); }
        if (ahora - ultimaTele > 1000) { ultimaTele = ahora; telemetria(); pintaDatos(); }
        cx.setTransform(st.dpr, 0, 0, st.dpr, 0, 0);
        st.dianas = [];
        st.W = datosAbierto && st.Wc > 700 ? Math.max(320, st.Wc - panelDatos.offsetWidth - 20) : st.Wc;
        fondo();
        if (enTierra()) {
          const obj = st.vista === 'mapa' ? 1 : 0;
          if (obj !== T.mObj) { T.tw = { desde: T.m, t0: ahora }; T.mObj = obj; }
          if (T.tw) { const u = Math.min(1, (ahora - T.tw.t0) / 2200), e = u * u * (3 - 2 * u); T.m = T.tw.desde + (T.mObj - T.tw.desde) * e; if (u >= 1) T.tw = null; }
          glc.style.visibility = 'visible';
          camara(); pintaTierra(); dibujaTierra(dt);
        } else if (nebulosaGL()) {
          glc.style.visibility = 'visible';
          construyeNebulosa(); camaraNeb(); pintaNebulosaGL(); dibujaNebulosaPro(dt);
        } else {
          glc.style.visibility = 'hidden';
          ({ nebulosa: dibujaNebulosa, dentro: dibujaSankey, radar: dibujaRadarPro, globo: dibujaNebulosa, mapa: dibujaNebulosa })[st.vista](dt);
        }
        if (st.raton) actualizaTip();
      } catch (e) { if (!bucle.dicho) { bucle.dicho = true; console.error(e); } }
    };

    // ---------- Start ----------
    let vistaGuardada = 'globo';
    try { const v = localStorage.getItem('guardiana_pd_vista'); if (VISTAS.includes(v)) vistaGuardada = v; } catch (_) {}
    ponVista(vistaGuardada);
    ajusta();
    let datosGuardado = null;
    try { datosGuardado = localStorage.getItem('guardiana_pd_datos'); } catch (_) {}
    ponDatos(datosGuardado === null ? st.Wc >= 1000 : datosGuardado === '1');
    // The website shows the same universe with example data only (`demo`); the panel, the person's.
    if (ctx.demo) empiezaDemo();
    else { await cargar(); ctx.cadaTanto(cargar, 5000); }
    requestAnimationFrame(bucle);
  }
})();
