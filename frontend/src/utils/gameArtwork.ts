export interface GameArtSpec {
  title: string;
  badge: string;
  accent: string;
  secondary: string;
  bgSvgDataUri: string;
}

function encodeSvg(svg: string): string {
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg.trim())}`;
}

export function getGameArt(templateId: string, category?: string): GameArtSpec {
  const id = (templateId || '').toLowerCase();

  // 1. Horizon MC (Rust 26.3)
  if (id.includes('horizon') || category === 'horizon') {
    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 320">
        <defs>
          <linearGradient id="hzG" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#f5b932" stop-opacity="0.55"/>
            <stop offset="50%" stop-color="#ea580c" stop-opacity="0.35"/>
            <stop offset="100%" stop-color="#0d0d12" stop-opacity="0.05"/>
          </linearGradient>
        </defs>
        <circle cx="465" cy="115" r="78" fill="url(#hzG)"/>
        <circle cx="465" cy="115" r="98" fill="none" stroke="#f5b932" stroke-opacity="0.35" stroke-width="2" stroke-dasharray="10 6"/>
        <!-- Isometric Voxel Terrain Horizon -->
        <polygon points="360,210 430,175 500,210 430,245" fill="#f5b932" fill-opacity="0.32" stroke="#ffd466" stroke-opacity="0.5" stroke-width="2"/>
        <polygon points="360,210 430,245 430,305 360,270" fill="#b87c10" fill-opacity="0.35"/>
        <polygon points="500,210 430,245 430,305 500,270" fill="#78350f" fill-opacity="0.4"/>
        <polygon points="450,155 505,128 560,155 505,182" fill="#38bdf8" fill-opacity="0.28" stroke="#38bdf8" stroke-opacity="0.5" stroke-width="1.5"/>
        <polygon points="280,245 340,215 400,245 340,275" fill="#f59e0b" fill-opacity="0.22" stroke="#f5b932" stroke-opacity="0.35" stroke-width="1.5"/>
      </svg>
    `;
    return {
      title: 'Horizon MC (Rust 26.3)',
      badge: 'RUST NATIVE ENGINE',
      accent: '#f5b932',
      secondary: '#ea580c',
      bgSvgDataUri: encodeSvg(svg),
    };
  }

  // 2. Minecraft Java / Bedrock
  if (id.includes('minecraft') || category === 'minecraft') {
    const isBedrock = id.includes('bedrock');
    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 320">
        <!-- Isometric Grass Block & Pickaxe silhouette -->
        <polygon points="430,85 520,130 430,175 340,130" fill="#22c55e" fill-opacity="0.38" stroke="#4ade80" stroke-opacity="0.55" stroke-width="2"/>
        <polygon points="340,130 430,175 430,275 340,230" fill="#854d0e" fill-opacity="0.36" stroke="#a16207" stroke-opacity="0.4" stroke-width="1.5"/>
        <polygon points="520,130 430,175 430,275 520,230" fill="#713f12" fill-opacity="0.42" stroke="#a16207" stroke-opacity="0.4" stroke-width="1.5"/>
        <polygon points="340,130 430,175 430,198 340,153" fill="#16a34a" fill-opacity="0.35"/>
        <polygon points="520,130 430,175 430,198 520,153" fill="#15803d" fill-opacity="0.35"/>
        <rect x="260" y="65" width="38" height="38" rx="4" fill="#38bdf8" fill-opacity="0.2" stroke="#38bdf8" stroke-opacity="0.4"/>
      </svg>
    `;
    return {
      title: isBedrock ? 'Minecraft Bedrock' : 'Minecraft Java',
      badge: isBedrock ? 'BEDROCK UDP' : 'PAPER / FABRIC / MODS',
      accent: '#22c55e',
      secondary: '#eab308',
      bgSvgDataUri: encodeSvg(svg),
    };
  }

  // 3. Palworld
  if (id.includes('palworld')) {
    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 320">
        <!-- Pal Sphere Artwork -->
        <circle cx="450" cy="160" r="96" fill="#0ea5e9" fill-opacity="0.22" stroke="#38bdf8" stroke-opacity="0.55" stroke-width="3"/>
        <path d="M 354 160 L 546 160" stroke="#f5b932" stroke-opacity="0.6" stroke-width="6"/>
        <circle cx="450" cy="160" r="28" fill="#160f02" stroke="#ffd466" stroke-opacity="0.8" stroke-width="4"/>
        <circle cx="450" cy="160" r="13" fill="#38bdf8" fill-opacity="0.7"/>
      </svg>
    `;
    return {
      title: 'Palworld Dedicated',
      badge: 'STEAMCMD • 2394010',
      accent: '#38bdf8',
      secondary: '#f5b932',
      bgSvgDataUri: encodeSvg(svg),
    };
  }

  // 4. Valheim
  if (id.includes('valheim')) {
    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 320">
        <!-- Viking Runic Shield & Longship Crest -->
        <circle cx="455" cy="160" r="92" fill="#dc2626" fill-opacity="0.22" stroke="#f5b932" stroke-opacity="0.5" stroke-width="4"/>
        <circle cx="455" cy="160" r="72" fill="none" stroke="#f87171" stroke-opacity="0.35" stroke-width="2" stroke-dasharray="12 6"/>
        <path d="M 455 68 L 455 252 M 363 160 L 547 160" stroke="#f5b932" stroke-opacity="0.45" stroke-width="4"/>
        <circle cx="455" cy="160" r="22" fill="#f5b932" fill-opacity="0.45"/>
      </svg>
    `;
    return {
      title: 'Valheim Dedicated',
      badge: 'VIKING REALM • 896660',
      accent: '#f97316',
      secondary: '#dc2626',
      bgSvgDataUri: encodeSvg(svg),
    };
  }

  // 5. Counter-Strike 2 (CS2)
  if (id.includes('cs2')) {
    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 320">
        <!-- Tactical Reticle & Radar Sweep -->
        <circle cx="455" cy="160" r="90" fill="#f59e0b" fill-opacity="0.16" stroke="#f5b932" stroke-opacity="0.55" stroke-width="2.5"/>
        <circle cx="455" cy="160" r="52" fill="none" stroke="#38bdf8" stroke-opacity="0.45" stroke-width="2"/>
        <path d="M 455 48 L 455 125 M 455 195 L 455 272 M 343 160 L 420 160 M 490 160 L 567 160" stroke="#ffd466" stroke-opacity="0.65" stroke-width="3"/>
        <circle cx="455" cy="160" r="6" fill="#ef4444" fill-opacity="0.85"/>
      </svg>
    `;
    return {
      title: 'Counter-Strike 2',
      badge: 'SOURCE 2 • TICK 128',
      accent: '#f59e0b',
      secondary: '#38bdf8',
      bgSvgDataUri: encodeSvg(svg),
    };
  }

  // 6. Rust Survival
  if (id.includes('steam-rust')) {
    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 320">
        <!-- Hazard Bio-Gear Artwork -->
        <circle cx="455" cy="160" r="88" fill="#ea580c" fill-opacity="0.24" stroke="#f97316" stroke-opacity="0.55" stroke-width="8" stroke-dasharray="22 12"/>
        <circle cx="455" cy="160" r="45" fill="#7c2d12" fill-opacity="0.4" stroke="#f5b932" stroke-opacity="0.5" stroke-width="3"/>
        <polygon points="455,115 495,185 415,185" fill="none" stroke="#ffd466" stroke-opacity="0.6" stroke-width="3"/>
      </svg>
    `;
    return {
      title: 'Rust Survival',
      badge: 'OXIDE / UMOD • 258550',
      accent: '#ea580c',
      secondary: '#f5b932',
      bgSvgDataUri: encodeSvg(svg),
    };
  }

  // 7. Terraria / tModLoader
  if (id.includes('terraria')) {
    const svg = `
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 320">
        <!-- Pixel Tree & Crystal Star -->
        <polygon points="455,60 480,115 540,122 495,162 508,220 455,190 402,220 415,162 370,122 430,115" fill="#a855f7" fill-opacity="0.26" stroke="#f5b932" stroke-opacity="0.55" stroke-width="2.5"/>
        <circle cx="455" cy="145" r="24" fill="#22c55e" fill-opacity="0.4"/>
      </svg>
    `;
    return {
      title: 'Terraria / tModLoader',
      badge: 'TMODLOADER • 1281930',
      accent: '#22c55e',
      secondary: '#f5b932',
      bgSvgDataUri: encodeSvg(svg),
    };
  }

  // 8. Node.js / Python / Native Custom
  const svg = `
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 320">
      <polygon points="455,62 538,110 538,206 455,254 372,206 372,110" fill="#38bdf8" fill-opacity="0.18" stroke="#f5b932" stroke-opacity="0.5" stroke-width="3"/>
      <circle cx="455" cy="158" r="38" fill="none" stroke="#38bdf8" stroke-opacity="0.5" stroke-width="2.5" stroke-dasharray="8 5"/>
      <path d="M 435 145 L 420 158 L 435 171 M 475 145 L 490 158 L 475 171 M 462 140 L 448 176" stroke="#ffd466" stroke-opacity="0.75" stroke-width="3" stroke-linecap="round"/>
    </svg>
  `;
  return {
    title: id.includes('python') ? 'Python 3 Service' : id.includes('node') ? 'Node.js Runtime' : 'Native Binary',
    badge: 'NATIVE PROCESS',
    accent: '#38bdf8',
    secondary: '#f5b932',
    bgSvgDataUri: encodeSvg(svg),
  };
}
