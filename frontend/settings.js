const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const $ = id => document.getElementById(id);
let config;
let snapshots = new Map();
let chartQueued = false;
let targetRowsFrame = 0;

function fmt(value, suffix = ' ms') {
  return value == null ? '--' : `${Math.round(value * 10) / 10}${suffix}`;
}

function statusText(status, paused = false) {
  if (paused || status === 'paused') return 'Paused';
  if (status === 'ok') return 'OK';
  if (status === 'timeout') return 'Timeout';
  if (status === 'refused') return 'Refused';
  if (status === 'offline') return 'Offline';
  if (status === 'dns_timeout') return 'DNS Timeout';
  if (status === 'dns_error') return 'DNS Error';
  if (status === 'stale') return 'Stale';
  if (status === 'disabled') return 'Disabled';
  if (status === 'starting') return 'Starting';
  return '--';
}

function statusClass(snapshot) {
  if (!snapshot || snapshot.paused || snapshot.status === 'disabled' || snapshot.status === 'starting') return '';
  if (snapshot.status === 'stale') return 'warning';
  if (snapshot.status !== 'ok') return 'bad';
  if (snapshot.currentMs == null) return '';
  if (snapshot.currentMs >= config.thresholds.highMs) return 'bad';
  if (snapshot.currentMs >= config.thresholds.warningMs) return 'warning';
  return 'ok';
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = String(text ?? '');
  return div.innerHTML;
}

function uniqueId() {
  return `target-${Date.now()}-${Math.random().toString(16).slice(2, 7)}`;
}

function activeTarget() {
  return config.targets.find(t => t.id === config.activeTargetId) || config.targets[0];
}

function refreshTargetSelect() {
  $('activeTarget').innerHTML = config.targets
    .map(t => `<option value="${escapeHtml(t.id)}">${escapeHtml(t.name)} — ${escapeHtml(t.host)}:${t.port}${t.enabled ? '' : ' [停用]'}</option>`)
    .join('');
  $('activeTarget').value = config.activeTargetId;
  loadActiveTargetForm();
}

function loadActiveTargetForm() {
  const t = activeTarget();
  if (!t) return;
  $('name').value = t.name;
  $('host').value = t.host;
  $('port').value = t.port;
  $('intervalMs').value = t.intervalMs;
  $('timeoutMs').value = t.timeoutMs;
  $('targetEnabled').checked = t.enabled !== false;
  $('addressFamily').value = t.addressFamily || 'auto';
}

function updateActiveTargetFromForm() {
  const t = activeTarget();
  if (!t) return;
  t.name = $('name').value.trim();
  t.host = $('host').value.trim();
  t.port = Number($('port').value);
  t.intervalMs = Number($('intervalMs').value);
  t.timeoutMs = Number($('timeoutMs').value);
  t.enabled = $('targetEnabled').checked;
  t.addressFamily = $('addressFamily').value;
}

function formatAge(ms) {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)}s`;
  return `${Math.round(ms / 60000)}m`;
}

function updateFloatingRangeLabels() {
  const opacity = Number($('floatingOpacity')?.value ?? config?.floatingOpacity ?? 0.82);
  const fontSize = Number($('floatingFontSize')?.value ?? config?.floatingFontSize ?? 42);
  if ($('floatingOpacityValue')) $('floatingOpacityValue').textContent = `${Math.round(opacity * 100)}%`;
  if ($('floatingFontSizeValue')) $('floatingFontSizeValue').textContent = `${fontSize}px`;
}

async function testCurrentTarget() {
  updateActiveTargetFromForm();
  const target = { ...activeTarget() };
  if (!target) return;
  const button = $('testTarget');
  const resultEl = $('testResult');
  button.disabled = true;
  resultEl.className = 'test-result';
  resultEl.textContent = '正在解析 DNS 并建立 TCP 连接…';
  try {
    const result = await invoke('test_target', { target });
    const dns = result.dnsMs == null ? '--' : `${result.dnsMs.toFixed(1)}ms`;
    const tcp = result.latencyMs == null ? '--' : `${result.latencyMs.toFixed(1)}ms`;
    const addr = result.resolvedAddress || result.attemptedAddresses?.join(', ') || '--';
    if (result.status === 'ok') {
      resultEl.className = 'test-result success';
      resultEl.textContent = `OK · TCP ${tcp} · DNS ${dns} · ${addr}`;
    } else {
      resultEl.className = 'test-result error';
      resultEl.textContent = `${statusText(result.status)} · DNS ${dns} · ${addr} · ${result.error || '连接失败'}`;
    }
  } catch (err) {
    resultEl.className = 'test-result error';
    resultEl.textContent = String(err);
  } finally {
    button.disabled = false;
  }
}

function renderConfig() {
  refreshTargetSelect();
  $('showFloating').checked = config.showFloating;
  $('mousePassthrough').checked = config.mousePassthrough;
  $('floatingShowTarget').checked = config.floatingShowTarget !== false;
  $('floatingShowStatusDot').checked = config.floatingShowStatusDot !== false;
  $('floatingShowTrend').checked = config.floatingShowTrend === true;
  $('floatingSize').value = config.floatingSize || 'standard';
  $('floatingOpacity').value = config.floatingOpacity ?? 0.82;
  $('floatingFontSize').value = config.floatingFontSize ?? 42;
  updateFloatingRangeLabels();
  $('autostart').checked = config.autostart;
  $('notificationsEnabled').checked = config.notificationsEnabled;
  $('notifyRecovery').checked = config.notifyRecovery !== false;
  $('notifyHighCount').value = config.notifyConsecutiveHigh;
  $('notifyFailureCount').value = config.notifyConsecutiveFailure;
  $('notificationCooldown').value = config.notificationCooldownSec;
  $('warningMs').value = config.thresholds.warningMs;
  $('highMs').value = config.thresholds.highMs;
  $('criticalMs').value = config.thresholds.criticalMs;
  $('targetCount').textContent = `${config.targets.length} 个目标`;
  renderTargetRows();
  queueChart();
}

function renderSnapshot(s) {
  const currentLabel = s.paused ? 'Paused' : (s.currentMs == null ? statusText(s.status) : fmt(s.currentMs));
  $('liveBadge').textContent = currentLabel;
  $('current').textContent = currentLabel;
  $('avg').textContent = fmt(s.averageMs);
  $('min').textContent = fmt(s.minMs);
  $('max').textContent = fmt(s.maxMs);
  $('p95').textContent = fmt(s.p95Ms);
  $('jitter').textContent = fmt(s.jitterMs);
  $('failure').textContent = `${Math.round((s.failurePercent || 0) * 10) / 10}%`;
  $('dns').textContent = fmt(s.dnsMs);
  $('chartTitle').textContent = `${s.targetName || '当前目标'} · 最近 60 秒`;
  const resolved = s.resolvedAddress ? ` → ${s.resolvedAddress}` : '';
  const age = s.sampleAgeMs != null ? ` · ${formatAge(s.sampleAgeMs)}前` : '';
  $('chartSub').textContent = `${s.host || '--'}:${s.port || '--'}${resolved} · ${statusText(s.status, s.paused)}${age}`;
}

function renderTargetRows() {
  const rows = config.targets.map(target => {
    const s = snapshots.get(target.id) || {
      targetId: target.id,
      targetName: target.name,
      host: target.host,
      port: target.port,
      enabled: target.enabled,
      status: target.enabled ? 'starting' : 'disabled',
      currentMs: null,
      averageMs: null,
      failurePercent: 0,
      paused: false,
    };
    const cls = statusClass(s);
    const current = s.currentMs == null ? statusText(s.status, s.paused) : fmt(s.currentMs);
    const status = statusText(s.status, s.paused);
    const active = target.id === config.activeTargetId ? ' active' : '';
    return `<tr data-target-id="${escapeHtml(target.id)}" class="${active.trim()}">
      <td><span class="target-name"><i class="target-dot ${cls}"></i>${escapeHtml(target.name)}</span></td>
      <td>${escapeHtml(target.host)}:${target.port}</td>
      <td>${escapeHtml(current)}</td>
      <td>${escapeHtml(fmt(s.averageMs))}</td>
      <td>${Math.round((s.failurePercent || 0) * 10) / 10}%</td>
      <td><span class="status-chip ${cls}">${escapeHtml(status)}</span></td>
    </tr>`;
  }).join('');
  $('targetRows').innerHTML = rows || '<tr><td colspan="6">暂无目标</td></tr>';

  for (const row of $('targetRows').querySelectorAll('[data-target-id]')) {
    row.addEventListener('click', async () => {
      const id = row.dataset.targetId;
      if (!id || id === config.activeTargetId) return;
      updateActiveTargetFromForm();
      config.activeTargetId = id;
      await save(false, false);
      loadActiveTargetForm();
      const snapshot = snapshots.get(id);
      if (snapshot) renderSnapshot(snapshot);
      renderTargetRows();
      queueChart(true);
    });
  }
}

function queueTargetRows() {
  if (targetRowsFrame) return;
  targetRowsFrame = requestAnimationFrame(() => {
    targetRowsFrame = 0;
    renderTargetRows();
  });
}

function chartColors() {
  const style = getComputedStyle(document.documentElement);
  return {
    accent: style.getPropertyValue('--accent').trim(),
    danger: style.getPropertyValue('--danger').trim(),
    muted: style.getPropertyValue('--muted').trim(),
    grid: style.getPropertyValue('--canvas-grid').trim(),
    text: style.getPropertyValue('--text').trim(),
  };
}

async function drawHistory() {
  chartQueued = false;
  if (!config?.activeTargetId) return;
  const points = await invoke('get_history', { targetId: config.activeTargetId });
  const canvas = $('historyChart');
  const wrap = canvas.parentElement;
  const rect = wrap.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.max(1, Math.round(rect.width * dpr));
  canvas.height = Math.max(1, Math.round(rect.height * dpr));
  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, rect.width, rect.height);

  $('chartEmpty').style.display = points.length ? 'none' : 'grid';
  if (!points.length) return;

  const colors = chartColors();
  const width = rect.width;
  const height = rect.height;
  const left = 38;
  const right = 10;
  const top = 10;
  const bottom = 22;
  const plotW = Math.max(1, width - left - right);
  const plotH = Math.max(1, height - top - bottom);
  const now = Date.now();
  const start = now - 60000;
  const successful = points.filter(p => p.latencyMs != null).map(p => p.latencyMs);
  const rawMax = successful.length ? Math.max(...successful) : 50;
  const yMax = Math.max(50, Math.ceil(rawMax / 25) * 25);

  ctx.font = '9px -apple-system, BlinkMacSystemFont, sans-serif';
  ctx.textBaseline = 'middle';
  ctx.strokeStyle = colors.grid;
  ctx.fillStyle = colors.muted;
  ctx.lineWidth = 1;

  for (let i = 0; i <= 4; i++) {
    const y = top + (plotH * i / 4);
    const value = Math.round(yMax * (1 - i / 4));
    ctx.beginPath();
    ctx.moveTo(left, y);
    ctx.lineTo(width - right, y);
    ctx.stroke();
    ctx.fillText(`${value}`, 6, y);
  }

  ctx.fillText('60s', left, height - 9);
  ctx.fillText('now', width - right - 18, height - 9);

  const xFor = ts => left + Math.max(0, Math.min(1, (Number(ts) - start) / 60000)) * plotW;
  const yFor = ms => top + (1 - Math.min(1, ms / yMax)) * plotH;

  ctx.strokeStyle = colors.accent;
  ctx.lineWidth = 1.8;
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  let drawing = false;
  ctx.beginPath();
  for (const p of points) {
    if (p.latencyMs == null) {
      drawing = false;
      continue;
    }
    const x = xFor(p.timestampMs);
    const y = yFor(p.latencyMs);
    if (!drawing) {
      ctx.moveTo(x, y);
      drawing = true;
    } else {
      ctx.lineTo(x, y);
    }
  }
  ctx.stroke();

  ctx.strokeStyle = colors.danger;
  ctx.lineWidth = 1.5;
  for (const p of points.filter(p => p.latencyMs == null)) {
    const x = xFor(p.timestampMs);
    const y = top + plotH - 4;
    ctx.beginPath();
    ctx.moveTo(x - 2.5, y - 2.5);
    ctx.lineTo(x + 2.5, y + 2.5);
    ctx.moveTo(x + 2.5, y - 2.5);
    ctx.lineTo(x - 2.5, y + 2.5);
    ctx.stroke();
  }
}

function queueChart(force = false) {
  if (chartQueued && !force) return;
  chartQueued = true;
  setTimeout(() => drawHistory().catch(err => console.error(err)), force ? 0 : 150);
}

async function loadSnapshots() {
  const list = await invoke('get_all_snapshots');
  snapshots = new Map(list.map(s => [s.targetId, s]));
  renderTargetRows();
}

async function boot() {
  config = await invoke('get_config');
  renderConfig();
  $('paused').checked = await invoke('is_paused');

  await loadSnapshots();
  const first = await invoke('get_snapshot');
  snapshots.set(first.targetId, first);
  renderSnapshot(first);
  renderTargetRows();
  queueChart(true);

  $('activeTarget').addEventListener('change', async () => {
    updateActiveTargetFromForm();
    config.activeTargetId = $('activeTarget').value;
    await save(false, false);
    loadActiveTargetForm();
    const snapshot = snapshots.get(config.activeTargetId);
    if (snapshot) renderSnapshot(snapshot);
    queueChart(true);
  });

  for (const id of ['name', 'host', 'port', 'intervalMs', 'timeoutMs', 'addressFamily', 'targetEnabled']) {
    $(id).addEventListener('change', updateActiveTargetFromForm);
  }

  $('addTarget').addEventListener('click', () => {
    updateActiveTargetFromForm();
    const id = uniqueId();
    config.targets.push({
      id,
      name: 'New Target',
      host: '127.0.0.1',
      port: 443,
      intervalMs: 1000,
      timeoutMs: 2000,
      enabled: true,
      addressFamily: 'auto',
    });
    config.activeTargetId = id;
    refreshTargetSelect();
    renderTargetRows();
  });

  $('deleteTarget').addEventListener('click', () => {
    if (config.targets.length <= 1) {
      showMessage('至少保留一个监测目标', true);
      return;
    }
    const id = config.activeTargetId;
    config.targets = config.targets.filter(t => t.id !== id);
    snapshots.delete(id);
    config.activeTargetId = config.targets[0].id;
    refreshTargetSelect();
    renderTargetRows();
    queueChart(true);
  });

  $('paused').addEventListener('change', async () => {
    await invoke('set_paused', { paused: $('paused').checked });
  });

  $('mousePassthrough').addEventListener('change', async () => {
    try {
      await invoke('set_mouse_passthrough', { enabled: $('mousePassthrough').checked });
    } catch (err) {
      $('mousePassthrough').checked = !($('mousePassthrough').checked);
      showMessage(String(err), true);
    }
  });

  $('testTarget').addEventListener('click', testCurrentTarget);

  for (const id of ['floatingOpacity', 'floatingFontSize']) {
    $(id).addEventListener('input', updateFloatingRangeLabels);
  }

  $('save').addEventListener('click', () => save(true, true));

  // The active probe emits both target-update (for the table) and
  // latency-update (for the active summary/HUD). Table refreshes are batched
  // to one animation frame so simultaneous multi-target updates coalesce.
  await listen('latency-update', event => {
    const s = event.payload;
    const previous = snapshots.get(s.targetId);
    snapshots.set(s.targetId, s);
    renderSnapshot(s);
    if (previous?.paused !== s.paused) queueTargetRows();
    queueChart();
  });

  await listen('target-update', event => {
    const s = event.payload;
    snapshots.set(s.targetId, s);
    queueTargetRows();
  });

  await listen('targets-update', event => {
    snapshots = new Map(event.payload.map(s => [s.targetId, s]));
    const active = snapshots.get(config.activeTargetId);
    if (active) renderSnapshot(active);
    queueTargetRows();
  });

  await listen('config-update', event => {
    config = event.payload;
    renderConfig();
  });

  window.addEventListener('resize', () => queueChart(true));
}

async function save(showSuccess = true, updateForm = true) {
  if (updateForm) updateActiveTargetFromForm();
  config.showFloating = $('showFloating').checked;
  config.mousePassthrough = $('mousePassthrough').checked;
  config.floatingShowTarget = $('floatingShowTarget').checked;
  config.floatingShowStatusDot = $('floatingShowStatusDot').checked;
  config.floatingShowTrend = $('floatingShowTrend').checked;
  config.floatingSize = $('floatingSize').value;
  config.floatingOpacity = Number($('floatingOpacity').value);
  config.floatingFontSize = Number($('floatingFontSize').value);
  config.uiVersion = 7;
  config.autostart = $('autostart').checked;
  config.notificationsEnabled = $('notificationsEnabled').checked;
  config.notifyRecovery = $('notifyRecovery').checked;
  config.notifyConsecutiveHigh = Number($('notifyHighCount').value);
  config.notifyConsecutiveFailure = Number($('notifyFailureCount').value);
  config.notificationCooldownSec = Number($('notificationCooldown').value);
  config.thresholds.warningMs = Number($('warningMs').value);
  config.thresholds.highMs = Number($('highMs').value);
  config.thresholds.criticalMs = Number($('criticalMs').value);

  try {
    config = await invoke('save_config', { config });
    renderConfig();
    await loadSnapshots();
    const active = await invoke('get_snapshot');
    snapshots.set(active.targetId, active);
    renderSnapshot(active);
    queueChart(true);
    if (showSuccess) showMessage('已保存');
    return true;
  } catch (err) {
    showMessage(String(err), true);
    return false;
  }
}

function showMessage(text, error = false) {
  $('message').textContent = text;
  $('message').className = error ? 'error' : '';
  clearTimeout(showMessage.timer);
  showMessage.timer = setTimeout(() => { $('message').textContent = ''; }, 3500);
}

boot().catch(err => showMessage(String(err), true));
