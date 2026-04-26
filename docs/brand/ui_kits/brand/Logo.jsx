/* global React */
// Chainworks Forge brand primitives — direct SVG re-renders of the
// originals shipped at docs/brand/render/*.html in the upstream repo.

const FlockSVG = ({ scale = 1, rotation = 0 }) => (
  <g>
    <defs>
      <symbol id="cwf-bird" viewBox="0 0 247 136">
        <path d="M0 44C27 24 58 13 91 13C112 13 136 18 162 31C183 12 203 3 220 3C207 19 198 34 193 49C218 58 236 74 247 97C228 89 207 82 183 78C165 108 136 127 95 133C72 136 49 135 27 130C52 118 71 103 84 86C57 86 29 72 0 44Z" fill="#FAFBFD"/>
        <path d="M167 34C184 18 202 7 220 3C209 18 201 33 196 47C184 43 175 39 167 34Z" fill="#F59A2B"/>
        <path d="M17 53C43 34 71 24 102 24C122 24 145 29 170 41C155 62 144 76 138 83C110 77 86 76 67 80C49 73 32 64 17 53Z" fill="#243244"/>
        <path d="M86 86C125 61 158 42 186 28C176 50 171 67 170 79C148 96 123 111 95 123C68 127 45 127 26 122C52 114 72 102 86 86Z" fill="#EEF3FA"/>
      </symbol>
    </defs>
  </g>
);

window.CWFBrand = {};

window.CWFBrand.HorizontalLogo = function HorizontalLogo({ width = 480, theme = "dark" }) {
  const bg = theme === "dark" ? "#091019" : "transparent";
  const fg = theme === "dark" ? "#F6F8FC" : "#0E1623";
  const fg2 = theme === "dark" ? "#E5EAF2" : "#475365";
  const h = width * (560 / 1600);
  return (
    <svg width={width} height={h} viewBox="0 0 1600 560" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Chainworks Forge">
      <defs>
        <linearGradient id="orbit-h" x1="92" y1="348" x2="618" y2="138" gradientUnits="userSpaceOnUse">
          <stop stopColor="#B8C3D1"/>
          <stop offset="0.55" stopColor="#8794A6"/>
          <stop offset="1" stopColor="#475365"/>
        </linearGradient>
        <symbol id="bird-h" viewBox="0 0 247 136">
          <path d="M0 44C27 24 58 13 91 13C112 13 136 18 162 31C183 12 203 3 220 3C207 19 198 34 193 49C218 58 236 74 247 97C228 89 207 82 183 78C165 108 136 127 95 133C72 136 49 135 27 130C52 118 71 103 84 86C57 86 29 72 0 44Z" fill="#FAFBFD"/>
          <path d="M167 34C184 18 202 7 220 3C209 18 201 33 196 47C184 43 175 39 167 34Z" fill="#F59A2B"/>
          <path d="M17 53C43 34 71 24 102 24C122 24 145 29 170 41C155 62 144 76 138 83C110 77 86 76 67 80C49 73 32 64 17 53Z" fill="#243244"/>
          <path d="M86 86C125 61 158 42 186 28C176 50 171 67 170 79C148 96 123 111 95 123C68 127 45 127 26 122C52 114 72 102 86 86Z" fill="#EEF3FA"/>
        </symbol>
      </defs>
      <rect width="1600" height="560" fill={bg}/>
      <path d="M111 358C173 266 275 186 418 118C556 52 680 12 788 1" stroke="url(#orbit-h)" strokeWidth="10" strokeLinecap="round" fill="none"/>
      <use href="#bird-h" x="170" y="245" width={247*0.86} height={136*0.86} transform="rotate(-10 293 313)"/>
      <use href="#bird-h" x="322" y="160" width={247*0.76} height={136*0.76} transform="rotate(-7 416 211)"/>
      <use href="#bird-h" x="474" y="78"  width={247*0.66} height={136*0.66} transform="rotate(-4 555 123)"/>
      <text x="760" y="252" fill={fg} fontFamily='"SF Pro Display", -apple-system, "Helvetica Neue", system-ui, sans-serif' fontSize="92" fontWeight="650" letterSpacing="5">CHAINWORKS</text>
      <text x="925" y="342" fill={fg2} fontFamily='"SF Pro Display", -apple-system, "Helvetica Neue", system-ui, sans-serif' fontSize="58" fontWeight="500" letterSpacing="16">FORGE</text>
    </svg>
  );
};

window.CWFBrand.BrandHero = function BrandHero({ width = 1200 }) {
  const h = width * (900 / 1600);
  return (
    <svg width={width} height={h} viewBox="0 0 1600 900" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <radialGradient id="glow-hero" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(312 172) rotate(49.6) scale(1044 1218)">
          <stop stopColor="#24344B"/>
          <stop offset="0.47" stopColor="#0E1623"/>
          <stop offset="1" stopColor="#04080D"/>
        </radialGradient>
        <linearGradient id="orbit-hero" x1="192" y1="514" x2="764" y2="251" gradientUnits="userSpaceOnUse">
          <stop stopColor="#E7EDF4"/>
          <stop offset="0.5" stopColor="#A3AFC0"/>
          <stop offset="1" stopColor="#627083"/>
        </linearGradient>
        <symbol id="bird-hero" viewBox="0 0 247 136">
          <path d="M0 44C27 24 58 13 91 13C112 13 136 18 162 31C183 12 203 3 220 3C207 19 198 34 193 49C218 58 236 74 247 97C228 89 207 82 183 78C165 108 136 127 95 133C72 136 49 135 27 130C52 118 71 103 84 86C57 86 29 72 0 44Z" fill="#FAFBFD"/>
          <path d="M167 34C184 18 202 7 220 3C209 18 201 33 196 47C184 43 175 39 167 34Z" fill="#F59A2B"/>
          <path d="M17 53C43 34 71 24 102 24C122 24 145 29 170 41C155 62 144 76 138 83C110 77 86 76 67 80C49 73 32 64 17 53Z" fill="#243244"/>
          <path d="M86 86C125 61 158 42 186 28C176 50 171 67 170 79C148 96 123 111 95 123C68 127 45 127 26 122C52 114 72 102 86 86Z" fill="#EEF3FA"/>
        </symbol>
      </defs>
      <rect width="1600" height="900" fill="url(#glow-hero)"/>
      <path d="M137 523C208 406 333 306 510 223C655 155 792 116 919 105" stroke="url(#orbit-hero)" strokeWidth="12" strokeLinecap="round" fill="none"/>
      <use href="#bird-hero" x="209" y="409" width={247*1.08} height={136*1.08} transform="rotate(-10 342 482)"/>
      <use href="#bird-hero" x="394" y="292" width={247*0.95} height={136*0.95} transform="rotate(-7 511 357)"/>
      <use href="#bird-hero" x="576" y="184" width={247*0.82} height={136*0.82} transform="rotate(-4 677 240)"/>
      <text x="886" y="392" fill="#F8FAFC" fontFamily='"SF Pro Display", -apple-system, system-ui, sans-serif' fontSize="120" fontWeight="650" letterSpacing="5">CHAINWORKS</text>
      <text x="1099" y="488" fill="#E5EAF2" fontFamily='"SF Pro Display", -apple-system, system-ui, sans-serif' fontSize="72" fontWeight="500" letterSpacing="18">FORGE</text>
      <text x="889" y="586" fill="#9FB0C4" fontFamily='"SF Pro Text", -apple-system, system-ui, sans-serif' fontSize="32" fontWeight="500" letterSpacing="1">Local control plane for agent-driven engineering work.</text>
    </svg>
  );
};

window.CWFBrand.AppIconMark = function AppIconMark({ size = 128, rounded = true }) {
  return (
    <svg width={size} height={size} viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg" style={{display:"block"}}>
      <defs>
        <linearGradient id="ai-bg" x1="0" y1="0" x2="0" y2="1">
          <stop stopColor="#1B2738"/>
          <stop offset="1" stopColor="#04080D"/>
        </linearGradient>
        <linearGradient id="ai-orbit" x1="20" y1="150" x2="180" y2="60" gradientUnits="userSpaceOnUse">
          <stop stopColor="#E7EDF4"/>
          <stop offset="0.5" stopColor="#A3AFC0"/>
          <stop offset="1" stopColor="#627083"/>
        </linearGradient>
        <symbol id="bird-app" viewBox="0 0 247 136">
          <path d="M0 44C27 24 58 13 91 13C112 13 136 18 162 31C183 12 203 3 220 3C207 19 198 34 193 49C218 58 236 74 247 97C228 89 207 82 183 78C165 108 136 127 95 133C72 136 49 135 27 130C52 118 71 103 84 86C57 86 29 72 0 44Z" fill="#FAFBFD"/>
          <path d="M167 34C184 18 202 7 220 3C209 18 201 33 196 47C184 43 175 39 167 34Z" fill="#F59A2B"/>
          <path d="M17 53C43 34 71 24 102 24C122 24 145 29 170 41C155 62 144 76 138 83C110 77 86 76 67 80C49 73 32 64 17 53Z" fill="#243244"/>
          <path d="M86 86C125 61 158 42 186 28C176 50 171 67 170 79C148 96 123 111 95 123C68 127 45 127 26 122C52 114 72 102 86 86Z" fill="#EEF3FA"/>
        </symbol>
      </defs>
      <rect width="200" height="200" rx={rounded ? 44 : 0} fill="url(#ai-bg)"/>
      <path d="M30 150 C 60 110, 100 80, 170 60" stroke="url(#ai-orbit)" strokeWidth="3.5" strokeLinecap="round" fill="none" opacity="0.85"/>
      <use href="#bird-app" x="40"  y="90"  width="62"  height="34" transform="rotate(-12 71 107)"/>
      <use href="#bird-app" x="78"  y="74"  width="56"  height="31" transform="rotate(-9 106 89)"/>
      <use href="#bird-app" x="115" y="60"  width="50"  height="28" transform="rotate(-6 140 74)"/>
    </svg>
  );
};

// Compact mark — just the lead bird + arc, for tight headers / sidebars
window.CWFBrand.Mark = function Mark({ size = 32 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg" style={{display:"block"}}>
      <defs>
        <linearGradient id="mk-orbit" x1="20" y1="150" x2="180" y2="60" gradientUnits="userSpaceOnUse">
          <stop stopColor="#A3AFC0"/>
          <stop offset="1" stopColor="#475365"/>
        </linearGradient>
        <symbol id="bird-mk" viewBox="0 0 247 136">
          <path d="M0 44C27 24 58 13 91 13C112 13 136 18 162 31C183 12 203 3 220 3C207 19 198 34 193 49C218 58 236 74 247 97C228 89 207 82 183 78C165 108 136 127 95 133C72 136 49 135 27 130C52 118 71 103 84 86C57 86 29 72 0 44Z" fill="currentColor"/>
          <path d="M167 34C184 18 202 7 220 3C209 18 201 33 196 47C184 43 175 39 167 34Z" fill="#F59A2B"/>
          <path d="M86 86C125 61 158 42 186 28C176 50 171 67 170 79C148 96 123 111 95 123C68 127 45 127 26 122C52 114 72 102 86 86Z" fill="currentColor" fillOpacity="0.6"/>
        </symbol>
      </defs>
      <path d="M30 150 C 60 110, 100 80, 170 60" stroke="url(#mk-orbit)" strokeWidth="6" strokeLinecap="round" fill="none"/>
      <use href="#bird-mk" x="55" y="65" width="120" height="66" transform="rotate(-8 115 98)"/>
    </svg>
  );
};
