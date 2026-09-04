const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

// Runtime-only class lets AppKit own the actual desktop blur while keeping the
// same tint, border, typography and motion CSS as the standalone preview.
document.body.classList.add('tauri-runtime');

const latencyEl = document.getElementById('latency');
const unitEl = document.getElementById('unit');
const trendEl = document.getElementById('trend');
const targetEl = document.getElementById('target');
const floatingEl = document.getElementById('floating');
const statusDotEl = document.getElementById('statusDot');

let config = null;
let previousMs = null;
let lastValueKey = '';
let lastVisualState = null;
let lastVisualClasses = '';
let latestSnapshot = null;
let readyApplied = false;

function restartAnimationClass(element, className) {
  element.classList.remove(className);
  requestAnimationFrame(() => element.classList.add(className));
}

function classForLatency(ms) {
  if (!config || ms == null) return 'normal';
  const t = config.thresholds;
  if (ms >= t.criticalMs) return 'critical';
  if (ms >= t.highMs) return 'high';
  if (ms >= t.warningMs) return 'warning';
  return 'normal';
}

function trendMark(ms) {
  if (previousMs == null || ms == null) return '';
  const delta = ms - previousMs;
  if (delta >= 30) return '↑↑';
  if (delta >= 10) return '↑';
  if (delta <= -30) return '↓↓';
  if (delta <= -10) return '↓';
  return '';
}

function normalizedSize(size) {
  return ['compact', 'standard', 'large'].includes(size) ? size : 'standard';
}

function applyFloatingPreferences() {
  if (!config) return;

  const opacity = Number(config.floatingOpacity ?? 0.82);
  const fontSize = Number(config.floatingFontSize ?? 42);
  const size = normalizedSize(config.floatingSize);

  const normalizedOpacity = Math.max(0.70, Math.min(1, opacity));
  // At the default 0.82 these values are exactly the same as the bundled
  // preview. The native material provides blur only; CSS owns the visible tint.
  const scale = normalizedOpacity / 0.82;
  const clampAlpha = value => Math.max(0.04, Math.min(0.92, value));
  floatingEl.style.setProperty('--floating-opacity', String(normalizedOpacity));
  floatingEl.style.setProperty('--glass-alpha', clampAlpha(0.40 * scale).toFixed(3));
  floatingEl.style.setProperty('--glass-alpha-soft', clampAlpha(0.17 * scale).toFixed(3));
  floatingEl.style.setProperty('--glass-alpha-end', clampAlpha(0.17 * scale).toFixed(3));
  floatingEl.style.setProperty('--glass-border-alpha', clampAlpha(0.62 * scale).toFixed(3));
  floatingEl.style.setProperty('--sheen-primary-alpha', clampAlpha(0.42 * scale).toFixed(3));
  floatingEl.style.setProperty('--sheen-secondary-alpha', clampAlpha(0.14 * scale).toFixed(3));
  floatingEl.style.setProperty('--pointer-primary-alpha', clampAlpha(0.28 * scale).toFixed(3));
  floatingEl.style.setProperty('--pointer-secondary-alpha', clampAlpha(0.08 * scale).toFixed(3));
  if (config.mousePassthrough) floatingEl.classList.remove('is-hovered');
  floatingEl.style.setProperty('--floating-font-size', `${Math.max(30, Math.min(52, fontSize))}px`);

  floatingEl.classList.toggle('hide-target', config.floatingShowTarget === false);
  floatingEl.classList.toggle('hide-status-dot', config.floatingShowStatusDot === false);
  floatingEl.classList.toggle('hide-trend', config.floatingShowTrend !== true);

  floatingEl.classList.remove('size-compact', 'size-standard', 'size-large');
  floatingEl.classList.add(`size-${size}`);
}

function setValue(value, unit = '', trend = '') {
  const valueKey = `${value}|${unit}|${trend}`;
  if (valueKey === lastValueKey) return;

  latencyEl.textContent = value;
  unitEl.textContent = unit;
  trendEl.textContent = trend;
  restartAnimationClass(latencyEl, 'value-updated');
  lastValueKey = valueKey;
}

function setVisualState(...states) {
  const activeStates = states.filter(Boolean);
  const nextState = activeStates[0] || 'normal';
  const nextClasses = activeStates.join('|');
  if (nextClasses === lastVisualClasses) return;

  const preserved = [...floatingEl.classList].filter(name =>
    name.startsWith('size-') ||
    name === 'hide-target' ||
    name === 'hide-status-dot' ||
    name === 'hide-trend' ||
    name === 'is-hovered'
  );

  floatingEl.className = 'floating';
  preserved.forEach(name => floatingEl.classList.add(name));
  activeStates.forEach(name => floatingEl.classList.add(name));

  if (lastVisualState !== null && nextState !== lastVisualState) {
    restartAnimationClass(floatingEl, 'state-changed');
  }
  lastVisualState = nextState;
  lastVisualClasses = nextClasses;
}

function render(snapshot) {
  latestSnapshot = snapshot;

  const targetLabel = snapshot.targetName || snapshot.host || `${snapshot.host}:${snapshot.port}`;
  const targetTitle = `${snapshot.host || '--'}:${snapshot.port || '--'}`;
  const ariaLabel = `${targetLabel} 网络延迟状态`;
  const statusTitle = snapshot.status || 'unknown';

  if (targetEl.textContent !== targetLabel) targetEl.textContent = targetLabel;
  if (targetEl.title !== targetTitle) targetEl.title = targetTitle;
  if (floatingEl.getAttribute('aria-label') !== ariaLabel) floatingEl.setAttribute('aria-label', ariaLabel);
  if (statusDotEl.getAttribute('title') !== statusTitle) statusDotEl.setAttribute('title', statusTitle);

  if (snapshot.paused) {
    setVisualState('paused', 'status-text');
    setValue('Paused');
    previousMs = null;
    return;
  }

  if (!snapshot.enabled || snapshot.status === 'disabled') {
    setVisualState('disabled', 'status-text');
    setValue('Disabled');
    previousMs = null;
    return;
  }

  if (snapshot.currentMs != null) {
    const trend = config?.floatingShowTrend === true ? trendMark(snapshot.currentMs) : '';
    setVisualState(classForLatency(snapshot.currentMs));
    setValue(String(Math.round(snapshot.currentMs)), 'ms', trend);
    previousMs = snapshot.currentMs;
    return;
  }

  previousMs = null;

  switch (snapshot.status) {
    case 'timeout':
      setVisualState('timeout', 'status-text');
      setValue('Timeout');
      break;
    case 'refused':
      setVisualState('refused', 'status-text');
      setValue('Refused');
      break;
    case 'offline':
      setVisualState('offline', 'status-text');
      setValue('Offline');
      break;
    case 'dns_timeout':
      setVisualState('dns-error', 'status-text');
      setValue('DNS Timeout');
      break;
    case 'dns_error':
      setVisualState('dns-error', 'status-text');
      setValue('DNS Error');
      break;
    case 'stale':
      setVisualState('stale', 'status-text');
      setValue('Stale');
      break;
    case 'starting':
      setVisualState('starting', 'status-text');
      setValue('Starting');
      break;
    default:
      setVisualState('starting');
      setValue('--', 'ms');
      break;
  }
}

async function boot() {
  config = await invoke('get_config');
  applyFloatingPreferences();
  floatingEl.title = config.mousePassthrough
    ? '鼠标穿透已开启 · 请从菜单栏解除'
    : '拖动移动 · 双击打开设置';

  render(await invoke('get_snapshot'));
  if (!readyApplied) {
    readyApplied = true;
    requestAnimationFrame(() => document.body.classList.add('is-ready'));
  }

  await listen('latency-update', event => render(event.payload));
  await listen('config-update', event => {
    config = event.payload;
    applyFloatingPreferences();
    floatingEl.title = config.mousePassthrough
      ? '鼠标穿透已开启 · 请从菜单栏解除'
      : '拖动移动 · 双击打开设置';
    if (latestSnapshot) render(latestSnapshot);
  });

  // Material interaction: a restrained specular highlight follows the cursor.
  // It disappears automatically when native mouse passthrough is enabled.
  floatingEl.addEventListener('pointerenter', () => {
    if (!config?.mousePassthrough) floatingEl.classList.add('is-hovered');
  });

  let pointerFrame = 0;
  let pendingPointer = null;
  floatingEl.addEventListener('pointermove', event => {
    if (config?.mousePassthrough) return;
    pendingPointer = { x: event.clientX, y: event.clientY };
    if (pointerFrame) return;
    pointerFrame = requestAnimationFrame(() => {
      pointerFrame = 0;
      if (!pendingPointer) return;
      const rect = floatingEl.getBoundingClientRect();
      const x = ((pendingPointer.x - rect.left) / Math.max(rect.width, 1)) * 100;
      const y = ((pendingPointer.y - rect.top) / Math.max(rect.height, 1)) * 100;
      floatingEl.style.setProperty('--pointer-x', `${Math.max(0, Math.min(100, x)).toFixed(1)}%`);
      floatingEl.style.setProperty('--pointer-y', `${Math.max(0, Math.min(100, y)).toFixed(1)}%`);
      pendingPointer = null;
    });
  }, { passive: true });

  floatingEl.addEventListener('pointerleave', () => {
    floatingEl.classList.remove('is-hovered');
    floatingEl.style.setProperty('--pointer-x', '50%');
    floatingEl.style.setProperty('--pointer-y', '18%');
  });

  floatingEl.addEventListener('dblclick', async () => {
    if (config?.mousePassthrough) return;
    try {
      await invoke('show_settings');
    } catch (err) {
      console.error('打开设置失败:', err);
      targetEl.textContent = `设置打开失败: ${String(err)}`;
    }
  });
}

boot().catch(err => {
  setVisualState('offline', 'status-text');
  setValue('Error');
  targetEl.textContent = String(err);
});
