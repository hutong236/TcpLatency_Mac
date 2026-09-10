const catStageEl = document.getElementById('networkCat');

let catConfig = null;
let lastCatState = '';
let lastTrafficLevel = -1;

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

function applyCatSnapshot(snapshot) {
  if (!catStageEl) return;
  const state = catStateForSnapshot(snapshot);
  if (state !== lastCatState) {
    catStageEl.dataset.state = state;
    lastCatState = state;
  }

  if (snapshot?.currentMs != null) {
    const cycle = Math.round(catCycleForLatency(snapshot.currentMs));
    catStageEl.style.setProperty('--cat-cycle', `${cycle}ms`);
    catStageEl.style.setProperty('--packet-cycle', `${Math.max(620, cycle * 1.55)}ms`);
  } else {
    catStageEl.style.setProperty('--cat-cycle', '1100ms');
    catStageEl.style.setProperty('--packet-cycle', '1500ms');
  }
}

function trafficLevel(bytesPerSecond) {
  const value = Math.max(0, Number(bytesPerSecond || 0));
  if (value < 1024) return 0;
  if (value < 100_000) return 1;
  if (value < 5_000_000) return 2;
  return 3;
}

function applyCatTraffic(snapshot) {
  if (!catStageEl || !snapshot || snapshot.status === 'unavailable' || snapshot.status === 'disabled') {
    return;
  }
  const total = Number(snapshot.downloadBytesPerSec || 0) + Number(snapshot.uploadBytesPerSec || 0);
  const level = trafficLevel(total);
  if (level === lastTrafficLevel) return;
  catStageEl.dataset.traffic = String(level);
  lastTrafficLevel = level;
}

async function bootNetworkCat() {
  if (!catStageEl) return;
  catConfig = await invoke('get_config');
  applyCatSnapshot(await invoke('get_snapshot'));

  await listen('latency-update', event => applyCatSnapshot(event.payload));
  await listen('traffic-update', event => applyCatTraffic(event.payload));
  await listen('config-update', event => {
    catConfig = event.payload;
  });
}

bootNetworkCat().catch(err => {
  console.error('小猫动画初始化失败:', err);
  if (catStageEl) catStageEl.dataset.state = 'offline';
});
