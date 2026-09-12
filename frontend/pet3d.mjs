import { PetStateMachine, classifyPet, currentLatency, isPetMode, rateText, validTraffic } from './pet-state.mjs';

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;
const $ = id => document.getElementById(id);
const panel = $('pet3d');
const stage = $('pet3dStage');
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
const machine = new PetStateMachine();
let config, snapshot, traffic, state;
let renderer = null, loading = false, failed = false, epoch = 0;
let recoveryTimer = 0, staleTimer = 0;
let configRevision = 0, snapshotRevision = 0;
const unlisten = [];
let disposed = false;

function text(id, value) { if ($(id).textContent !== value) $(id).textContent = value; }
function active() { return !disposed && isPetMode(config) && config.showFloating !== false && !document.hidden; }
function stopRenderer() {
  epoch++;
  renderer?.dispose(); renderer = null;
  panel.dataset.renderer = 'stopped';
}
function fallback(reason) {
  failed = true; renderer = null;
  panel.dataset.renderer = 'fallback';
  $('pet3dFallback').hidden = false;
  $('pet3dRetry').hidden = false;
  text('pet3dNotice', '3D 暂不可用，已显示静态小猫');
  $('pet3dNotice').title = reason;
}

async function ensureRenderer() {
  if (renderer || loading || failed || !active()) return;
  const generation = epoch;
  loading = true; panel.dataset.renderer = 'loading';
  try {
    // The 720 KB engine is never imported in HUD mode. Everything is bundled
    // locally, so rendering continues when DNS or the network is unavailable.
    const { CatRenderer } = await import('./cat-renderer.mjs');
    if (generation !== epoch || !active()) return;
    renderer = new CatRenderer(stage, fallback);
    $('pet3dFallback').hidden = true;
    $('pet3dRetry').hidden = true;
    panel.dataset.renderer = 'ready';
    refreshMotion();
    if (state) renderer?.setState(state);
  } catch (error) {
    if (generation === epoch && active()) fallback(String(error));
  } finally {
    loading = false;
    if (!renderer && !failed && active()) ensureRenderer();
  }
}

function refreshMotion() {
  const motion = config?.networkCatAnimationEnabled !== false && !reducedMotion.matches;
  panel.dataset.motion = motion ? 'animated' : 'static';
  renderer?.configure(motion, config?.pet3dPowerSaver !== false);
  if (!failed) text('pet3dNotice', config?.mousePassthrough ? '已锁定 · 从菜单栏解除' : '拖动移动 · 双击设置');
}

function renderInformation() {
  if (!config) return;
  const now = Date.now();
  state = machine.update(snapshot, traffic, config, now);
  panel.dataset.state = state.currentKind;
  panel.dataset.pose = state.kind;
  panel.dataset.recovering = String(state.recovering);
  const latency = currentLatency(snapshot);
  text('pet3dLatency', latency);
  text('pet3dUnit', latency === '--' ? '' : 'ms');
  const live = classifyPet(snapshot, traffic, config, now);
  text('pet3dStatus', state.recovering ? '已恢复' : live.label);
  text('pet3dTarget', snapshot?.targetName || snapshot?.host || '等待目标');
  $('pet3dTarget').title = snapshot ? `${snapshot.host}:${snapshot.port}` : '';
  $('pet3dTarget').hidden = config.floatingShowTarget === false;
  $('pet3dDot').hidden = config.floatingShowStatusDot === false;
  $('pet3dTraffic').hidden = config.floatingShowTraffic === false;
  const hasTraffic = validTraffic(traffic, now);
  text('pet3dDownload', hasTraffic ? rateText(traffic.downloadBytesPerSec) : '--');
  text('pet3dUpload', hasTraffic ? rateText(traffic.uploadBytesPerSec) : '--');
  $('pet3dTraffic').title = `本机接口 ${traffic?.interfaceName || 'auto'} · 实时下载 / 上传`;
  renderer?.setState(state);
  clearTimeout(recoveryTimer);
  if (state.recovering && active()) recoveryTimer = setTimeout(renderInformation, Math.max(1, machine.recoveredUntil - now + 10));
}

function reconcile() {
  const pet = isPetMode(config);
  $('floating').hidden = pet;
  panel.hidden = !pet;
  panel.dataset.size = config?.floatingSize || 'standard';
  panel.style.setProperty('--pet-font-size', `${Math.max(30, Math.min(52, config?.floatingFontSize || 42))}px`);
  panel.classList.toggle('pet-locked', !!config?.mousePassthrough);
  $('pet3dLock').disabled = !!config?.mousePassthrough;
  refreshMotion();
  renderInformation();
  clearTimeout(staleTimer);
  if (!active()) { stopRenderer(); clearTimeout(recoveryTimer); return; }
  ensureRenderer();
  armTrafficExpiry();
}

function armTrafficExpiry() {
  clearTimeout(staleTimer);
  if (!active() || !validTraffic(traffic)) return;
  staleTimer = setTimeout(() => { renderInformation(); }, Math.max(1, traffic.timestampMs + 5005 - Date.now()));
}

async function showSettings(event) {
  if (config?.mousePassthrough || (event.type === 'dblclick' && event.target.closest('button'))) return;
  try { await invoke('show_settings'); } catch (error) { text('pet3dNotice', `打开设置失败：${String(error)}`); }
}

async function boot() {
  unlisten.push(await listen('config-update', event => {
    configRevision++;
    const previousTarget = config?.activeTargetId;
    const wasPet = isPetMode(config);
    config = event.payload;
    if (previousTarget && previousTarget !== config.activeTargetId && snapshot?.targetId !== config.activeTargetId) {
      snapshot = null; machine.reset();
    }
    if (wasPet !== isPetMode(config)) failed = false;
    reconcile();
  }));
  unlisten.push(await listen('latency-update', event => {
    snapshotRevision++;
    snapshot = event.payload; renderInformation();
  }));
  unlisten.push(await listen('traffic-update', event => {
    traffic = event.payload; renderInformation(); armTrafficExpiry();
  }));
  const configAtStart = configRevision, snapshotAtStart = snapshotRevision;
  const [initialConfig, initialSnapshot] = await Promise.all([invoke('get_config'), invoke('get_snapshot')]);
  if (configRevision === configAtStart) config = initialConfig;
  if (snapshotRevision === snapshotAtStart) snapshot = initialSnapshot;
  reconcile();
}

panel.addEventListener('dblclick', showSettings);
$('pet3dSettings').addEventListener('click', showSettings);
$('pet3dLock').addEventListener('click', async () => {
  try { await invoke('set_mouse_passthrough', { enabled: true }); }
  catch (error) { text('pet3dNotice', `锁定失败：${String(error)}`); }
});
$('pet3dRetry').addEventListener('click', () => {
  failed = false; $('pet3dRetry').hidden = true; ensureRenderer();
});
panel.addEventListener('pointermove', event => {
  if (config?.mousePassthrough || !renderer) return;
  const rect = stage.getBoundingClientRect();
  renderer.look(Math.max(-1, Math.min(1, (event.clientX - rect.left) / rect.width * 2 - 1)),
    Math.max(-1, Math.min(1, (event.clientY - rect.top) / rect.height * 2 - 1)));
}, { passive: true });
panel.addEventListener('pointerleave', () => renderer?.look(0, 0));
document.addEventListener('visibilitychange', reconcile);
reducedMotion.addEventListener('change', refreshMotion);
window.addEventListener('pagehide', () => {
  disposed = true; stopRenderer(); clearTimeout(staleTimer); clearTimeout(recoveryTimer);
  unlisten.splice(0).forEach(fn => fn());
  reducedMotion.removeEventListener('change', refreshMotion);
});
boot().catch(error => { fallback(String(error)); text('pet3dStatus', '暂时无法读取状态'); });
