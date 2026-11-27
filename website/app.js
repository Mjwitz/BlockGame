/* ==========================================================================
   Roots & Ridges — rootsandridges.gg
   Interactive Showcase, 20-Biome Atlas, Soundtrack Player & Utilities
   ========================================================================== */

'use strict';

/* --------------------------------------------------------------------------
   1. Pillars of Survival Data (100% grounded in Roots & Ridges v0.3.0)
   -------------------------------------------------------------------------- */
const PILLARS = {
  engine: {
    image: 'assets/images/promo_hero.png',
    alt: 'In-game screenshot of a wildflower and pumpkin highland overlooking misty glacial ice spires across a frozen sea',
    caption: 'Captured in-game · Procedural Highlands & Glacial Sea',
    title: 'Vast, Seamless Worlds Built to Explore',
    description:
      'Every world unfolds smoothly around you with zero lag spikes—even at full sprint or soaring in flight. Sunlight filters naturally through forest canopies, torches and electric lamps cast warm glow across deep caverns, and crystal rivers shift from dawn mist to starlit nights.',
    facts: [
      { key: 'Seamless Exploration', val: 'Fluid 60+ FPS terrain streaming with sweeping vistas and atmospheric fog' },
      { key: 'Dynamic Sky & Water', val: 'Translucent rivers and glacial ice under a full celestial day/night cycle' },
      { key: 'Survival & Creative', val: 'True Survival with health, hunger & beds—or unlimited flight in Creative Mode' }
    ]
  },
  power: {
    image: 'assets/images/promo_power.png',
    alt: 'Real in-engine dusk screenshot of a workshop with two spinning Wind Turbines, rooftop Solar Panels, glowing Electric Lamps, Battery Boxes, Electric Furnace, Storage Terminal, and Diamond, Gold, and Iron Chests',
    caption: 'Rendered live in-engine · Wind, Solar, EU Grid & Storage Terminal',
    title: 'Electrical Power (EU/s) & Smart Storage',
    description:
      'Graduate from coal fires to full base automation. Generate EU with Coal Generators (+40 EU/s), Solar Panels (+15/+60 EU/s), and 4-block animated Wind Turbines (+45 EU/s). Store power in Quantum Battery Boxes (250k EU), wire Diamond Cables (5,000 EU/s), and link Iron, Gold, or Diamond Chests into searchable Storage Terminals.',
    facts: [
      { key: 'Power Generation', val: 'Coal (+40 EU/s), Solar (+15/+60 EU/s) & altitude-scaled Wind Turbines' },
      { key: 'Storage Terminals', val: 'Green Storage Cables link chests for live search, sorting & Shift-Click' },
      { key: 'Portable Backpacks', val: 'Unique per-instance Leather (18), Iron (27) & Diamond (36) backpacks (B)' }
    ]
  },
  homestead: {
    image: 'assets/images/promo_homestead.png',
    alt: 'Real in-engine screenshot of a birch and spruce cabin with a lit Cooking Pot, irrigated rows of Wheat, Tomatoes, Carrots, Potatoes, Sugar Cane, Pumpkins, and Melons, plus a Cow, Sheep, Chicken, and Pig',
    caption: 'Rendered live in-engine · Farming, Cooking Pot & Passive Mobs',
    title: 'Farming, Cooking Pot & Living Villages',
    description:
      'Cultivate 7 crops including Stem Crops (Melons & Pumpkins) enriched with passive Compost (1.5×) and Bone Fertilizer (2.0× + bonus yield). Brew hearty stews, Sweet Cake, and poison-curing Herbal Tea in the Cooking Pot, then trade with biome-themed Village Shopkeepers for Gold Coins and permanent gear Enhancements.',
    facts: [
      { key: 'Cooking & Saturation', val: '4-slot shapeless Cooking Pot & two-tone gold/orange Saturation HUD' },
      { key: 'Living Villages', val: 'Profession Shopkeepers (Tiers 1–5) guarded by mortal Village Guards' },
      { key: 'Animal Ranching', val: 'Breed Cows, Pigs, Chickens & Sheep with species-specific Breeding Feed' }
    ]
  },
  skills: {
    image: 'assets/images/promo_lakeside.png',
    alt: 'Real in-engine screenshot of a wooden fishing dock with lantern and tackle chest along a stony shoreline overlooking translucent water with seagrass and misty wildflower hills',
    caption: 'Rendered live in-engine · Shoreline Fishing Dock & Translucent Water',
    title: 'MMO Skills (1–1000) & Tackle Fishing',
    description:
      'Every swing, harvest, and cast builds lasting mastery saved per world and server. Level 7 skills—Combat, Woodcutting, Mining, Fishing, Digging, Farming, and Agility—from 1 to 1000 with major Milestone Unlocks. Cast Wooden to Diamond Rods with custom lines, lures, and bait in a timing-based fishing minigame.',
    facts: [
      { key: '7 MMO Skills', val: 'Scaling perks & major Milestone Unlocks at 25, 50, 100, 250, 500, 750 & 1000' },
      { key: 'Fishing Minigame', val: '4 rod tiers, upgradeable tackle & 10 fish species plus rare treasure' },
      { key: 'JEI Recipe Browser', val: 'Hover any item in inventory and press R or U to inspect recipes & uses' }
    ]
  },
  identity: {
    image: 'assets/images/promo_characters.png',
    alt: 'Real in-engine screenshot of three stylized low-poly player characters with distinct hairstyles and outfits standing by a camp with a crafting table, lit furnace, and diamond chest',
    caption: 'Rendered live in-engine · Stylized 3D Character Creator & Co-Op',
    title: 'Your Character, Career & Controller',
    description:
      'Craft a stylized low-poly Character Appearance with 3 Body Frames, hairstyles, RGB base clothing, and saved Character Presets synced across multiplayer. Hunt 42 global Achievements across 8 pillars (plus spoiler-hidden secrets), or kick back with native plug-and-play controller support.',
    facts: [
      { key: '42 Global Achievements', val: 'Profile-wide Career Stats across Survival worlds with HUD unlock toasts' },
      { key: '5 Controller Families', val: 'Native Xbox, PlayStation, Nintendo, Steam Deck & Steam Controller glyphs' },
      { key: 'Hybrid Virtual Cursor', val: 'Left-stick pointer with D-pad/bumper snapping & all-source damage rumble' }
    ]
  }
};

/* --------------------------------------------------------------------------
   2. The 20 Authentic Procedural Biomes
   -------------------------------------------------------------------------- */
const BIOMES = [
  { name: 'Plains', climate: 'temperate', label: 'Temperate', desc: 'Flat to gently rolling grasslands, wildflowers, and scattered oak trees.', swatch: 'linear-gradient(90deg, #6ba347, #89be58)' },
  { name: 'Flower Meadow', climate: 'temperate', label: 'Temperate', desc: 'Vibrant wildflower fields draped over sunlit gentle hills.', swatch: 'linear-gradient(90deg, #6ea54b, #d85a4a, #e8c246)' },
  { name: 'Forest', climate: 'temperate', label: 'Temperate', desc: 'Dense oak and birch woodlands rich in timber and wildlife.', swatch: 'linear-gradient(90deg, #3c6e38, #588b45)' },
  { name: 'Birch Forest', climate: 'temperate', label: 'Temperate', desc: 'Bright silver-barked birch groves with airy dappled foliage.', swatch: 'linear-gradient(90deg, #87b55c, #d9d5c9)' },
  { name: 'Dark Forest', climate: 'temperate', label: 'Temperate', desc: 'Thick dark-oak canopy with shadowy fog and looming monsters.', swatch: 'linear-gradient(90deg, #213a25, #3a5234)' },
  { name: 'Taiga', climate: 'boreal', label: 'Boreal', desc: 'Cool evergreen spruce forests rising across northern foothills.', swatch: 'linear-gradient(90deg, #284b3e, #486b58)' },
  { name: 'Snowy Tundra', climate: 'boreal', label: 'Boreal', desc: 'Crisp snow-blanketed expanses and frozen winter waterways.', swatch: 'linear-gradient(90deg, #c9dae4, #edf4f8)' },
  { name: 'Glacier', climate: 'boreal', label: 'Alpine', desc: 'Towering translucent ice formations and windswept frozen peaks.', swatch: 'linear-gradient(90deg, #6ba6cb, #b8dff2)' },
  { name: 'Mountains', climate: 'boreal', label: 'Alpine', desc: 'High-altitude stone peaks, steep cliffs, and prime wind-turbine ridges.', swatch: 'linear-gradient(90deg, #646c70, #9da5a8)' },
  { name: 'Desert', climate: 'arid', label: 'Arid', desc: 'Sweeping golden sand dunes, sandstone outcrops, and cacti.', swatch: 'linear-gradient(90deg, #d7bc7a, #ebd59d)' },
  { name: 'Red Desert', climate: 'arid', label: 'Arid', desc: 'Sunlit rust-red sand dunes shimmering under warm desert haze.', swatch: 'linear-gradient(90deg, #b85d38, #d9824f)' },
  { name: 'Badlands', climate: 'arid', label: 'Canyon', desc: 'Layered terracotta canyons, mesas, and sun-baked red clay.', swatch: 'linear-gradient(90deg, #9a4732, #cc7645, #d9a76a)' },
  { name: 'Savanna', climate: 'arid', label: 'Arid', desc: 'Warm golden grasslands dotted with flat-topped acacia trees.', swatch: 'linear-gradient(90deg, #9c9a42, #c4b558)' },
  { name: 'Jungle', climate: 'tropical', label: 'Tropical', desc: 'Towering tropical canopy trees and dense emerald undergrowth.', swatch: 'linear-gradient(90deg, #277836, #49a848)' },
  { name: 'Swamp', climate: 'tropical', label: 'Wetlands', desc: 'Shallow murky waters, rich clay deposits, and moss-tinted fog.', swatch: 'linear-gradient(90deg, #475b3c, #687a52)' },
  { name: 'Beach', climate: 'coastal', label: 'Coastal', desc: 'Warm sandy coastal shorelines meeting gentle ocean swells.', swatch: 'linear-gradient(90deg, #dfca91, #61a5c2)' },
  { name: 'Stony Shore', climate: 'coastal', label: 'Coastal', desc: 'Rugged rocky coastal cliffs, gravel spits, and exposed ore veins.', swatch: 'linear-gradient(90deg, #6c7275, #4d7e96)' },
  { name: 'River', climate: 'coastal', label: 'Freshwater', desc: 'Winding freshwater channels carving valleys between macro biomes.', swatch: 'linear-gradient(90deg, #3c86b5, #69b2d9)' },
  { name: 'Ocean', climate: 'coastal', label: 'Marine', desc: 'Deep expansive marine basins waiting for shoreline harbors.', swatch: 'linear-gradient(90deg, #1d4e78, #3072a6)' },
  { name: 'Coral Reef', climate: 'coastal', label: 'Marine', desc: 'Vibrant turquoise lagoons alive with colorful coral formations.', swatch: 'linear-gradient(90deg, #2aa6b0, #e07668, #f0c259)' }
];

/* --------------------------------------------------------------------------
   3. Original Stereo Soundtrack Data (Real MP3s from Music/)
   -------------------------------------------------------------------------- */
const TRACKS = [
  {
    title: 'Where Maps End',
    trigger: 'Triggers when running and sprinting across the world adventuring.',
    src: 'assets/audio/Where_Maps_End.mp3'
  },
  {
    title: 'Measure of the Dark',
    trigger: 'Triggers while mining stone, ores, or digging deep underground.',
    src: 'assets/audio/Measure_of_the_Dark.mp3'
  },
  {
    title: 'Where the Roots Break Soil',
    trigger: 'Atmospheric anytime background composition while building or exploring.',
    src: 'assets/audio/Where_the_Roots_Break_Soil.mp3'
  },
  {
    title: 'The View From The Ridge',
    trigger: 'Atmospheric anytime background composition overlooking the wilds.',
    src: 'assets/audio/The_View_From_The_Ridge.mp3'
  }
];

/* --------------------------------------------------------------------------
   Initialization
   -------------------------------------------------------------------------- */
document.addEventListener('DOMContentLoaded', () => {
  initHeaderAndNav();
  initPillarShowcase();
  initBiomeAtlas();
  initSoundtrackPlayer();
  initServerCopy();
  initOsHighlight();
  initScrollReveal();
});

/* Header Scroll State & Mobile Drawer */
function initHeaderAndNav() {
  const header = document.getElementById('site-header');
  const toggle = document.getElementById('nav-toggle');
  const navLinks = document.querySelectorAll('#nav-links a');

  const onScroll = () => {
    header.classList.toggle('is-scrolled', window.scrollY > 24);
  };
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  if (toggle) {
    toggle.addEventListener('click', () => {
      const open = header.classList.toggle('nav-open');
      toggle.setAttribute('aria-expanded', String(open));
    });
  }

  navLinks.forEach((link) => {
    link.addEventListener('click', () => {
      header.classList.remove('nav-open');
      if (toggle) toggle.setAttribute('aria-expanded', 'false');
    });
  });
}

/* Interactive Pillars of Survival Showcase */
function initPillarShowcase() {
  const tabs = document.querySelectorAll('.pillar-tab');
  const mediaWrap = document.getElementById('pillar-media-wrap');
  const imgEl = document.getElementById('pillar-image');
  const captionEl = document.getElementById('pillar-caption');
  const titleEl = document.getElementById('pillar-title');
  const descEl = document.getElementById('pillar-description');
  const factsEl = document.getElementById('pillar-facts');
  const panelEl = document.getElementById('pillar-panel');

  if (!tabs.length || !imgEl) return;

  function selectPillar(key, activeTab) {
    const data = PILLARS[key];
    if (!data) return;

    tabs.forEach((t) => {
      const isMatch = t === activeTab;
      t.classList.toggle('is-active', isMatch);
      t.setAttribute('aria-selected', String(isMatch));
    });

    panelEl.setAttribute('aria-labelledby', activeTab.id);

    mediaWrap.classList.add('is-fading');
    window.setTimeout(() => {
      imgEl.src = data.image;
      imgEl.alt = data.alt;
      captionEl.textContent = data.caption;
      titleEl.textContent = data.title;
      descEl.textContent = data.description;

      factsEl.innerHTML = data.facts
        .map(
          (f) => `
          <div class="pillar-fact">
            <span class="pillar-fact-key">${f.key}</span>
            <span class="pillar-fact-val">${f.val}</span>
          </div>`
        )
        .join('');

      mediaWrap.classList.remove('is-fading');
    }, 150);
  }

  tabs.forEach((tab) => {
    tab.addEventListener('click', () => selectPillar(tab.dataset.pillar, tab));
  });
}

/* 20 Biomes Atlas with Climate Filtering */
function initBiomeAtlas() {
  const grid = document.getElementById('biome-grid');
  const filterButtons = document.querySelectorAll('.filter-btn');
  if (!grid) return;

  function renderBiomes(climate) {
    const list = climate === 'all' ? BIOMES : BIOMES.filter((b) => b.climate === climate);
    grid.innerHTML = list
      .map(
        (b) => `
        <article class="biome-card">
          <div class="biome-swatch" style="background: ${b.swatch};" aria-hidden="true"></div>
          <div class="biome-meta">
            <h3>${b.name}</h3>
            <span class="biome-climate">${b.label}</span>
          </div>
          <p>${b.desc}</p>
        </article>`
      )
      .join('');
  }

  filterButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      filterButtons.forEach((b) => b.classList.remove('is-active'));
      btn.classList.add('is-active');
      renderBiomes(btn.dataset.climate);
    });
  });

  renderBiomes('all');
}

/* Original Stereo Soundtrack Player */
function initSoundtrackPlayer() {
  const audio = document.getElementById('soundtrack-audio');
  const playBtn = document.getElementById('audio-play-btn');
  const iconPlay = document.getElementById('icon-play');
  const iconPause = document.getElementById('icon-pause');
  const statusEl = document.getElementById('player-status');
  const titleEl = document.getElementById('player-title');
  const triggerEl = document.getElementById('player-trigger');
  const scrubber = document.getElementById('audio-scrubber');
  const currentEl = document.getElementById('audio-current');
  const durationEl = document.getElementById('audio-duration');
  const volumeSlider = document.getElementById('audio-volume');
  const trackRows = document.querySelectorAll('.track-row');

  if (!audio || !playBtn) return;

  let currentIndex = 0;
  let isSeeking = false;

  audio.volume = Number(volumeSlider ? volumeSlider.value : 0.75);

  function formatTime(sec) {
    if (!Number.isFinite(sec) || sec < 0) return '0:00';
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  }

  function setPlayUI(playing) {
    iconPlay.hidden = playing;
    iconPause.hidden = !playing;
    playBtn.setAttribute('aria-label', playing ? 'Pause soundtrack' : 'Play soundtrack');
    statusEl.textContent = playing ? 'Now Playing' : 'Paused';
  }

  function loadTrack(index, autoplay) {
    currentIndex = index;
    const track = TRACKS[index];
    audio.src = track.src;
    titleEl.textContent = track.title;
    triggerEl.textContent = track.trigger;
    scrubber.value = 0;
    currentEl.textContent = '0:00';
    durationEl.textContent = '0:00';

    trackRows.forEach((row, i) => {
      row.classList.toggle('is-active', i === index);
    });

    if (autoplay) {
      audio.play().then(() => setPlayUI(true)).catch(() => setPlayUI(false));
    } else {
      statusEl.textContent = 'Ready to Play';
    }
  }

  playBtn.addEventListener('click', () => {
    if (audio.paused) {
      audio.play().then(() => setPlayUI(true)).catch(() => setPlayUI(false));
    } else {
      audio.pause();
      setPlayUI(false);
    }
  });

  trackRows.forEach((row) => {
    row.addEventListener('click', () => {
      const idx = Number(row.dataset.trackIndex);
      if (idx === currentIndex && !audio.paused) {
        audio.pause();
        setPlayUI(false);
      } else {
        loadTrack(idx, true);
      }
    });
  });

  audio.addEventListener('loadedmetadata', () => {
    durationEl.textContent = formatTime(audio.duration);
  });

  audio.addEventListener('timeupdate', () => {
    if (!isSeeking && Number.isFinite(audio.duration) && audio.duration > 0) {
      scrubber.value = (audio.currentTime / audio.duration) * 100;
      currentEl.textContent = formatTime(audio.currentTime);
    }
  });

  audio.addEventListener('ended', () => {
    const next = (currentIndex + 1) % TRACKS.length;
    loadTrack(next, true);
  });

  scrubber.addEventListener('input', () => {
    isSeeking = true;
    if (Number.isFinite(audio.duration) && audio.duration > 0) {
      const target = (Number(scrubber.value) / 100) * audio.duration;
      currentEl.textContent = formatTime(target);
    }
  });

  scrubber.addEventListener('change', () => {
    if (Number.isFinite(audio.duration) && audio.duration > 0) {
      audio.currentTime = (Number(scrubber.value) / 100) * audio.duration;
    }
    isSeeking = false;
  });

  if (volumeSlider) {
    volumeSlider.addEventListener('input', () => {
      audio.volume = Number(volumeSlider.value);
    });
  }

  loadTrack(0, false);
}

/* Copy Dedicated Server Command */
function initServerCopy() {
  const copyBtn = document.getElementById('copy-server-cmd');
  const cmdText = document.getElementById('server-cmd-text');
  if (!copyBtn || !cmdText) return;

  copyBtn.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(cmdText.textContent.trim());
      copyBtn.textContent = 'Copied!';
      copyBtn.classList.add('is-copied');
      window.setTimeout(() => {
        copyBtn.textContent = 'Copy command';
        copyBtn.classList.remove('is-copied');
      }, 2000);
    } catch {
      copyBtn.textContent = 'Select & copy';
    }
  });
}

/* Subtle OS Detection for Download Card Highlight */
function initOsHighlight() {
  const ua = navigator.userAgent.toLowerCase();
  const winCard = document.getElementById('download-card-windows');
  const linCard = document.getElementById('download-card-linux');
  const winTag = document.getElementById('tag-windows');
  const linTag = document.getElementById('tag-linux');

  if (ua.includes('win') && winCard) {
    winCard.classList.add('is-recommended');
    if (winTag) winTag.textContent = 'Recommended for Your System · Windows x86_64';
  } else if (ua.includes('linux') && !ua.includes('android') && linCard) {
    linCard.classList.add('is-recommended');
    if (linTag) linTag.textContent = 'Recommended for Your System · Linux x86_64';
  }
}

/* Subtle Scroll Reveal */
function initScrollReveal() {
  const elements = document.querySelectorAll('.reveal');
  if (!elements.length) return;

  if (!('IntersectionObserver' in window)) {
    elements.forEach((el) => el.classList.add('is-visible'));
    return;
  }

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          observer.unobserve(entry.target);
        }
      });
    },
    { threshold: 0.12 }
  );

  elements.forEach((el) => observer.observe(el));
}
