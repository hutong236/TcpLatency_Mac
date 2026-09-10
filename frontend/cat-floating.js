const catStageEl = document.getElementById('networkCat');

let catConfig = null;
let latestCatSnapshot = null;
let latestTrafficSnapshot = null;
let lastCatState = '';
let lastTrafficLevel = -1;
let lastHealthState = '';
let lastTheme = '';
let customObjectUrl = null;
let customAssetPath = '';

const CAT_THEMES = new Set(['default', 'pixel', 'cyber', 'custom']);

function defaultCatMarkup() {
  return `
    <svg class="cat-svg" viewBox="0 0 72 44" focusable="false" aria-hidden="true">
      <path class="cat-tail" d="M17 24 C7 22, 5 13, 12 9 C16 7, 18 11, 15 14 C12 16, 13 19, 19 19" fill="none" stroke="currentColor" stroke-width="4.2" stroke-linecap="round" />
      <ellipse class="cat-body" cx="34" cy="24" rx="16" ry="9.5" fill="currentColor" opacity=".92" />
      <circle class="cat-head" cx="51" cy="18" r="8.5" fill="currentColor" />
      <path class="cat-ear" d="M45 13 L46.5 6.5 L51 11.5 Z" fill="currentColor" />
      <path class="cat-ear" d="M52 11.3 L57 7 L58.5 14 Z" fill="currentColor" />
      <circle class="cat-eye" cx="54.4" cy="17.2" r="1" fill="rgba(255,255,255,.88)" />
      <path class="cat-leg back" d="M27 29 L23 37" fill="none" stroke="currentColor" stroke-width="3.6" stroke-linecap="round" />
      <path class="cat-leg front" d="M39 29 L44 36" fill="none" stroke="currentColor" stroke-width="3.6" stroke-linecap="round" />
    </svg>`;
}

function pixelCatMarkup() {
  return `
    <svg class="cat-svg cat-pixel-svg" viewBox="0 0 72 44" focusable="false" aria-hidden="true" shape-rendering="crispEdges">
      <g class="cat-tail"><rect x="7" y="20" width="12" height="4"/><rect x="5" y="15" width="4" height="7"/></g>
      <g class="cat-body"><rect x="18" y="17" width="30" height="16"/><rect x="14" y="21" width="6" height="8"/></g>
      <g class="cat-head"><rect x="44" y="11" width="17" height="17"/><rect x="46" y="7" width="5" height="6"/><rect x="55" y="7" width="5" height="6"/></g>
      <rect class="cat-eye" x="55" y="16" width="2" height="2" fill="var(--cat-eye-color, white)"/>
      <rect class="cat-leg back" x="23" y="31" width="5" height="9"/>
      <rect class="cat-leg front" x="39" y="31" width="5" height="9"/>
    </svg>`;
}

function cyberCatMarkup() {
  return `
    <svg class="cat-svg cat-cyber-svg" viewBox="0 0 72 44" focusable="false" aria-hidden="true">
      <path class="cyber-trace" d="M3 31 H14 L18 27 H27" fill="none" />
      <path class="cat-tail" d="M19 25 C9 25 7 16 13 11" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
      <path class="cat-body" d="M18 25 Q22 15 36 15 H45 Q50 16 53 24 Q47 31 34 32 H23 Z" fill="none" stroke="currentColor" stroke-width="2" />
      <path class="cat-head" d="M44 17 L47 9 L52 14 L59 10 L61 20 Q60 27 52 28 Q45 26 44 17Z" fill="none" stroke="currentColor" stroke-width="2" />
      <circle class="cyber-core" cx="34" cy="23" r="3.2" />
      <circle class="cat-eye" cx="56" cy="18" r="1.3" />
      <path class="cat-leg back" d="M25 31 L22 38" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
      <path class="cat-leg front" d="M42 30 L47 37" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
    </svg>`;
}

function sharedCatMarkup() {
  return `
    <i class="cat-packet p1"></i>
    <i class="cat-packet p2"></i>
    <i class="cat-packet p3"></i>
    <span class="cat-health" aria-hidden="true"></span>`;
}

function revokeCustomObjectUrl() {
  if (customObjectUrl) URL.revokeObjectURL(customObjectUrl);
  customObjectUrl = null;
  customAssetPath = '';
}

function bytesToBlob(asset) {
  const bytes = new Uint8Array(asset.data || []);
  return new Blob([bytes], { type: asset.mimeType || 'application/octet-stream' });
}

async function renderCustomTheme(path) {
  if (!path) {
    catStageEl.innerHTML = `${defaultCatMarkup()}${sharedCatMarkup()}`;
    catStageEl.dataset.theme = 'default';
    return;
  }
  if (customObjectUrl && customAssetPath === path) {
    catStageEl.innerHTML = `<img class="cat-custom-image" alt="" />${sharedCatMarkup()}`;
    catStageEl.querySelector('.cat-custom-image').src = customObjectUrl;
    await applyCustomAnimationMode();
    return;
  }

  revokeCustomObjectUrl();
  const asset = await invoke('load_cat_asset', { path });
  customObjectUrl = URL.createObjectURL(bytesToBlob(asset));
  customAssetPath = path;
  catStageEl.innerHTML = `<img class="cat-custom-image" alt="" />${sharedCatMarkup()}`;
  catStageEl.querySelector('.cat-custom-image').src = customObjectUrl;
  await applyCustomAnimationMode();
}

async function applyCustomAnimationMode() {
  if (catStageEl.dataset.theme !== 'custom' || !customObjectUrl) return;
  const img = catStageEl.querySelector('.cat-custom-image');
  if (!img) return;
  if (catConfig?.networkCatAnimationEnabled !== false) {
    if (img.src !== customObjectUrl) img.src = customObjectUrl;
    return;
  }

  if (!img.complete) {
    await new Promise(resolve => {
      img.addEventListener('load', resolve, { once: true });
      img.addEventListener('error', resolve, { once: true });
    });
  }
  if (!img.naturalWidth || !img.naturalHeight) return;
  const canvas = document.createElement('canvas');
  const scale = Math.min(1, 256 / Math.max(img.naturalWidth, img.naturalHeight));
  canvas.width = Math.max(1, Math.round(img.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(img.naturalHeight * scale));
  canvas.getContext('2d')?.drawImage(img, 0, 0, canvas.width, canvas.height);
  try {
    img.src = canvas.toDataURL('image/png');
  } catch (_) {
    // SVG or decoder edge cases can refuse canvas export; CSS motion still stops.
  }
}

async function applyTheme() {
  if (!catStageEl || !catConfig) return;
  const requested = String(catConfig.networkCatTheme || 'default').toLowerCase();
  const theme = CAT_THEMES.has(requested) ? requested : 'default';
  const customPath = String(catConfig.networkCatCustomAsset || '').trim();
  const themeKey = theme === 'custom' ? `custom:${customPath}` : theme;
  if (themeKey === lastTheme) {
    if (theme === 'custom') await applyCustomAnimationMode();
    return;
  }

  try {
    if (theme === 'custom') {
      catStageEl.dataset.theme = 'custom';
      await renderCustomTheme(customPath);
      catStageEl.dataset.theme = customPath ? 'custom' : 'default';
    } else {
      revokeCustomObjectUrl();
      const markup = theme === 'pixel'
        ? pixelCatMarkup()
        : theme === 'cyber'
          ? cyberCatMarkup()
          : defaultCatMarkup();
      catStageEl.innerHTML = `${markup}${sharedCatMarkup()}`;
      catStageEl.dataset.theme = theme;
    }
    lastTheme = themeKey;
  } catch (err) {
    console.error('自定义网络猫素材加载失败:', err);
    revokeCustomObjectUrl();
    catStageEl.innerHTML = `${defaultCatMarkup()}${sharedCatMarkup()}`;
    catStageEl.dataset.theme = 'default';
    lastTheme = 'fallback';
    catStageEl.title = `自定义网络猫素材加载失败: ${String(err)}`;
  }
}

function catStateForSnapshot(snapshot) {
  if (!snapshot) return 'starting';
  if (snapshot.paused) return 'paused';
  if (!snapshot.enabled || snapshot.status === 'disabled') return 'disabled';
  if (snapshot.currentMs != null) {
    const thresholds = catConfig?.thresholds;
    if (thresholds) {
      if (snapshot.currentMs >= thresholds.criticalMs) return 'critical';
      if (snapshot.currentMs >= thresholds.highMs) return 'high';
      if (snapshot.currentMs >= thresholds.warningMs) return 'warning';
    }
    return 'normal';
  }
  return snapshot.status || 'starting';
}

function catCycleForLatency(ms) {
  const value = Math.max(0, Number(ms || 0));
  if (value <= 20) return 360;
  if (value <= 50) return 360 + ((value - 20) / 30) * 120;
  if (value <= 100) return 480 + ((value - 50) / 50) * 180;
  if (value <= 200) return 660 + ((value - 100) / 100) * 260;
  return Math.min(1250, 920 + ((value - 200) / 300) * 330);
}

function trafficLevel(bytesPerSecond) {
  const value = Math.max(0, Number(bytesPerSecond || 0));
  if (value < 1024) return 0;
  if (value < 100_000) return 1;
  if (value < 5_000_000) return 2;
  return 3;
}

function combinedHealth(snapshot, traffic) {
  if (!snapshot) return 'idle';
  if (snapshot.paused || snapshot.status === 'starting' || snapshot.status === 'stale') return 'idle';
  if (!snapshot.enabled || snapshot.status === 'disabled') return 'idle';
  if (snapshot.currentMs == null || snapshot.status !== 'ok') return 'offline';

  const thresholds = catConfig?.thresholds || { warningMs: 50, highMs: 100, criticalMs: 200 };
  const failure = Number(snapshot.failurePercent || 0);
  const jitter = Number(snapshot.jitterMs || 0);
  const totalTraffic = Number(traffic?.downloadBytesPerSec || 0) + Number(traffic?.uploadBytesPerSec || 0);

  if (snapshot.currentMs >= thresholds.criticalMs || failure >= 20) return 'critical';
  if (jitter >= Math.max(25, snapshot.currentMs * 0.65) || failure >= 5) return 'unstable';
  if (snapshot.currentMs >= thresholds.warningMs) return 'slow';
  if (totalTraffic >= 5_000_000) return 'busy';
  if (snapshot.currentMs < thresholds.warningMs * 0.5 && failure < 1 && jitter < 10) return 'excellent';
  return 'good';
}

function healthLabel(health) {
  return {
    excellent: '快',
    good: '稳',
    busy: '忙',
    unstable: '抖',
    slow: '慢',
    critical: '危',
    offline: '断',
    idle: '待',
  }[health] || '稳';
}

function applyHealth() {
  if (!catStageEl) return;
  const health = combinedHealth(latestCatSnapshot, latestTrafficSnapshot);
  if (health !== lastHealthState) {
    catStageEl.dataset.health = health;
    lastHealthState = health;
  }
  const label = catStageEl.querySelector('.cat-health');
  if (label) label.textContent = healthLabel(health);
  const ms = latestCatSnapshot?.currentMs == null ? '--' : `${Math.round(latestCatSnapshot.currentMs)}ms`;
  const total = Number(latestTrafficSnapshot?.downloadBytesPerSec || 0) + Number(latestTrafficSnapshot?.uploadBytesPerSec || 0);
  catStageEl.title = `网络状态 ${healthLabel(health)} · 延迟 ${ms} · 实时流量 ${(total / 1024).toFixed(total < 1024 * 1024 ? 0 : 1)} KB/s`;
}

function applyCatVisibilityAndMotion() {
  if (!catStageEl || !catConfig) return;
  const enabled = catConfig.networkCatEnabled !== false;
  catStageEl.hidden = !enabled;
  catStageEl.classList.toggle('cat-static', catConfig.networkCatAnimationEnabled === false);
}

function applyCatSnapshot(snapshot) {
  if (!catStageEl) return;
  latestCatSnapshot = snapshot;
  const state = catStateForSnapshot(snapshot);
  if (state !== lastCatState) {
    catStageEl.dataset.state = state;
    lastCatState = state;
  }

  if (snapshot?.currentMs != null) {
    const cycle = Math.round(catCycleForLatency(snapshot.currentMs));
    catStageEl.style.setProperty('--cat-cycle', `${cycle}ms`);
    catStageEl.style.setProperty('--packet-cycle', `${Math.max(620, Math.round(cycle * 1.55))}ms`);
  } else {
    catStageEl.style.setProperty('--cat-cycle', '1100ms');
    catStageEl.style.setProperty('--packet-cycle', '1500ms');
  }
  applyHealth();
}

function applyCatTraffic(snapshot) {
  if (!catStageEl || !snapshot) return;
  latestTrafficSnapshot = snapshot;
  if (snapshot.status !== 'unavailable' && snapshot.status !== 'disabled') {
    const total = Number(snapshot.downloadBytesPerSec || 0) + Number(snapshot.uploadBytesPerSec || 0);
    const level = trafficLevel(total);
    if (level !== lastTrafficLevel) {
      catStageEl.dataset.traffic = String(level);
      lastTrafficLevel = level;
    }
  }
  applyHealth();
}

async function applyCatConfig(nextConfig) {
  catConfig = nextConfig;
  applyCatVisibilityAndMotion();
  await applyTheme();
  if (latestCatSnapshot) applyCatSnapshot(latestCatSnapshot);
  if (latestTrafficSnapshot) applyCatTraffic(latestTrafficSnapshot);
}

async function bootNetworkCat() {
  if (!catStageEl) return;
  await applyCatConfig(await invoke('get_config'));
  applyCatSnapshot(await invoke('get_snapshot'));

  await listen('latency-update', event => applyCatSnapshot(event.payload));
  await listen('traffic-update', event => applyCatTraffic(event.payload));
  await listen('config-update', event => {
    applyCatConfig(event.payload).catch(err => console.error('网络猫配置应用失败:', err));
  });
}

window.addEventListener('beforeunload', revokeCustomObjectUrl);

bootNetworkCat().catch(err => {
  console.error('网络猫初始化失败:', err);
  if (catStageEl) catStageEl.dataset.state = 'offline';
});
