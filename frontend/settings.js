const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const $ = id => document.getElementById(id);
let config;
let snapshots = new Map();
let chartTimer = 0;
let chartGeneration = 0;
let targetRowsFrame = 0;
let switchingTarget = false;
const MESSAGE_IDLE = '所有更改均保存在本机';

function setDirty(isDirty) {
  const saveButton = $('save');
  if (saveButton) saveButton.classList.toggle('is-dirty', isDirty);
  const message = $('message');
  if (message && !message.classList.contains('error')) {
    message.textContent = isDirty ? '有尚未保存的更改' : MESSAGE_IDLE;
    message.className = isDirty ? 'pending' : '';
  }
}

function validateTargetForm() {
  for (const id of ['name', 'host', 'port', 'intervalMs', 'timeoutMs', 'httpPath']) {
    const field = $(id);
    if (!field.value.trim() || !field.checkValidity()) {
      showMessage('请检查目标名称、地址及端口 / 时间范围', true);
      field.focus();
      return false;
    }
  }
  return true;
}

function fmt(value, suffix = ' ms') {
  return value == null ? '--' : `${Math.round(value * 10) / 10}${suffix}`;
}

function statusText(status, paused = false, mode = 'tcp') {
  if (paused || status === 'paused') return 'Paused';
  if (status === 'battery') return 'Battery';
  if (status === 'ok') return mode === 'tcp' ? 'TCP Connected'
    : mode === 'ssh' ? 'SSH Verified'
      : mode === 'https' ? 'HTTPS Verified' : 'HTTP Verified';
  if (status === 'http_error') return 'HTTP Error';
  if (status === 'protocol_error') return 'Protocol Error';
  if (status === 'invalid_target') return 'Invalid Target';
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
  if (!snapshot || snapshot.paused || snapshot.batteryPaused || snapshot.status === 'disabled' || snapshot.status === 'starting') return '';
  if (snapshot.status === 'stale') return 'warning';
  if (snapshot.status !== 'ok') return 'bad';
  if (snapshot.currentMs == null) return '';
  if (snapshot.currentMs >= config.thresholds.highMs) return 'bad';
  if (snapshot.currentMs >= config.thresholds.warningMs) return 'warning';
  return 'ok';
}

function escapeHtml(text) {
  // Safe for both text nodes and quoted HTML attributes in the target table.
  const entities = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
  return String(text ?? '').replace(/[&<>"']/g, char => entities[char]);
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

function updateProtocolFields() {
  const mode = $('probeMode').value;
  $('httpPathRow').hidden = mode !== 'http' && mode !== 'https';
  $('httpPath').disabled = false; // Persist path even when temporarily selecting TCP/SSH.
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
  $('probeMode').value = t.probeMode || 'tcp';
  $('httpPath').value = t.httpPath || '/';
  updateProtocolFields();
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
  t.probeMode = $('probeMode').value;
  t.httpPath = $('httpPath').value.trim();
}

function formatAge(ms) {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)}s`;
  return `${Math.round(ms / 60000)}m`;
}

function normalizedFloatingBackgroundMode(mode) {
  return ['glass', 'transparent', 'solid'].includes(mode) ? mode : 'glass';
}

function updateFloatingRangeLabels() {
  const opacityInput = $('floatingOpacity');
  const opacity = Number(opacityInput?.value ?? config?.floatingOpacity ?? 0.82);
  const fontSize = Number($('floatingFontSize')?.value ?? config?.floatingFontSize ?? 42);
  const backgroundMode = normalizedFloatingBackgroundMode(
    $('floatingBackgroundMode')?.value ?? config?.floatingBackgroundMode,
  );

  if (opacityInput) opacityInput.disabled = backgroundMode === 'transparent';
  if ($('floatingOpacityValue')) {
    $('floatingOpacityValue').textContent = backgroundMode === 'transparent'
      ? '纯透明'
      : `${Math.round(opacity * 100)}%`;
  }
  if ($('floatingFontSizeValue')) $('floatingFontSizeValue').textContent = `${fontSize}px`;
}

async function testCurrentTarget() {
  updateActiveTargetFromForm();
  const target = { ...activeTarget() };
  if (!target) return;
  const button = $('testTarget');
  const resultEl = $('testResult');
  button.disabled = true;
  button.setAttribute('aria-busy', 'true');
  resultEl.className = 'test-result';
  resultEl.textContent = '正在执行协议验证，请求可能经过 VPN / TUN…';
  try {
    const result = await invoke('test_target', { target });
    const dns = result.dnsMs == null ? '' : ` · DNS ${result.dnsMs.toFixed(1)}ms`;
    const total = result.latencyMs == null ? '--' : `${result.latencyMs.toFixed(1)}ms`;
    const tcp = result.tcpMs == null ? '' : ` · TCP ${result.tcpMs.toFixed(1)}ms`;
    const addr = result.resolvedAddress || result.attemptedAddresses?.join(', ') || '--';
    const interfaceName = result.routeInterface || '未知';
    const routeNote = interfaceName.startsWith('utun') ? ' · TUN/VPN 路径，结果可能受代理接管影响' : '';
    const detail = result.responseDetail || (result.probeMode === 'tcp' ? '仅验证 TCP 建连' : '');
    const prefix = statusText(result.status, false, result.probeMode);
    resultEl.className = result.status === 'ok' ? 'test-result success' : 'test-result error';
    resultEl.textContent = `${prefix} · ${result.status === 'ok' ? total : '未通过验证'}${tcp}${dns} · ${addr} · 路由 ${interfaceName}${routeNote}${detail ? ` · ${detail}` : ''}${result.error ? ` · ${result.error}` : ''}`;
  } catch (err) {
    resultEl.className = 'test-result error';
    resultEl.textContent = String(err);
  } finally {
    button.disabled = false;
    button.removeAttribute('aria-busy');
  }
}

function renderConfig() {
  refreshTargetSelect();
  $('showFloating').checked = config.showFloating;
  $('mousePassthrough').checked = config.mousePassthrough;
  $('floatingShowTarget').checked = config.floatingShowTarget !== false;
  $('floatingShowStatusDot').checked = config.floatingShowStatusDot !== false;
  $('floatingShowTrend').checked = config.floatingShowTrend === true;
  $('floatingBackgroundMode').value = normalizedFloatingBackgroundMode(config.floatingBackgroundMode);
  $('floatingSize').value = config.floatingSize || 'standard';
  $('floatingOpacity').value = config.floatingOpacity ?? 0.82;
  $('floatingFontSize').value = config.floatingFontSize ?? 42;
  updateFloatingRangeLabels();
  $('autostart').checked = config.autostart;
  $('batteryPauseOnBattery').checked = config.batteryPauseOnBattery === true;
  $('batteryPauseLowEnabled').checked = config.batteryPauseLowEnabled === true;
  $('batteryLowThreshold').value = config.batteryLowThresholdPercent ?? 20;
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

function renderBatteryStatus(b) {
  if (!b) return;
  const el = $('batteryStatus');
  if (!b.available) {
    el.textContent = '未读取到电源信息，省电暂停不可用';
    return;
  }
  const power = b.onAc ? '电源适配器供电' : '电池供电';
  const level = b.levelPercent == null ? '' : ` · 电量 ${b.levelPercent}%`;
  const guard = b.guardPaused ? ' · 省电暂停已生效' : '';
  el.textContent = `当前：${power}${level}${guard}`;
}

function renderSnapshot(s) {
  const currentLabel = s.batteryPaused ? 'Battery' : (s.paused ? 'Paused' : (s.currentMs == null ? statusText(s.status) : fmt(s.currentMs)));
  const badge = $('liveBadge');
  badge.textContent = currentLabel;
  badge.className = 'live-badge';
  const severity = statusClass(s);
  if (severity) badge.classList.add(severity);
  badge.setAttribute('aria-label', `当前主目标：${s.targetName || s.host || '--'}，${currentLabel}`);
  $('current').textContent = currentLabel;
  $('tcp').textContent = fmt(s.tcpMs);
  $('avg').textContent = fmt(s.averageMs);
  $('min').textContent = fmt(s.minMs);
  $('max').textContent = fmt(s.maxMs);
  $('p95').textContent = fmt(s.p95Ms);
  $('jitter').textContent = fmt(s.jitterMs);
  $('failure').textContent = `${Math.round((s.failurePercent || 0) * 10) / 10}%`;
  $('dns').textContent = fmt(s.dnsMs);
  $('chartTitle').textContent = `${s.targetName || '当前目标'} · 最近 60 秒`;
  const mode = (s.probeMode || 'tcp').toUpperCase();
  $('metricLegend').textContent = (mode === 'HTTP' || mode === 'HTTPS')
    ? `${mode} 响应头耗时（含 DNS、TCP 和必要的 TLS）`
    : mode === 'SSH'
      ? 'SSH 协议标识到达耗时（含 DNS、TCP）'
      : 'TCP 建连总耗时（含 DNS 与回退）';
  const resolved = s.resolvedAddress ? ` → ${s.resolvedAddress}` : '';
  const age = s.sampleAgeMs != null ? ` · ${formatAge(s.sampleAgeMs)}前` : '';
  const route = s.routeInterface ? ` · 接口 ${s.routeInterface}${s.routeInterface.startsWith('utun') ? '（TUN/VPN，结果可能经代理）' : ''}` : '';
  const protocol = s.responseDetail ? ` · ${s.responseDetail}` : '';
  $('chartSub').textContent = `${s.host || '--'}:${s.port || '--'}${resolved} · ${statusText(s.status, s.paused, s.probeMode)}${route}${protocol}${age}`;
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
      batteryPaused: false,
    };
    const cls = statusClass(s);
    const current = s.currentMs == null ? statusText(s.status, s.paused, s.probeMode) : fmt(s.currentMs);
    const status = statusText(s.status, s.paused, s.probeMode);
    const active = target.id === config.activeTargetId ? ' active' : '';
    return `<tr data-target-id="${escapeHtml(target.id)}" class="${active.trim()}">
      <td><span class="target-name"><button type="button" class="target-select" data-select-target="${escapeHtml(target.id)}" aria-label="设为主目标：${escapeHtml(target.name)}" aria-pressed="${target.id === config.activeTargetId}"><i class="target-dot ${cls}" aria-hidden="true"></i>${escapeHtml(target.name)}</button></span></td>
      <td>${escapeHtml(target.host)}:${target.port} <small>· ${escapeHtml((target.probeMode || 'tcp').toUpperCase())}</small></td>
      <td>${escapeHtml(current)}</td>
      <td>${escapeHtml(fmt(s.averageMs))}</td>
      <td>${Math.round((s.failurePercent || 0) * 10) / 10}%</td>
      <td><span class="status-chip ${cls}">${escapeHtml(status)}</span></td>
    </tr>`;
  }).join('');
  $('targetRows').innerHTML = rows || '<tr><td colspan="6">暂无目标</td></tr>';

  // Event delegation avoids rebuilding one listener for every target on each sample.

}

function queueTargetRows() {
  if (targetRowsFrame) return;
  targetRowsFrame = requestAnimationFrame(() => {
    targetRowsFrame = 0;
    renderTargetRows();
  });
}

function chartColors() {
  const style = getComputedStyle(document.querySelector('.summary-card'));
  return {
    accent: style.getPropertyValue('--chart-line').trim() || '#86bfff',
    danger: style.getPropertyValue('--chart-failure').trim() || '#ff8494',
    muted: style.getPropertyValue('--chart-label').trim() || '#9fb3ce',
    grid: style.getPropertyValue('--chart-grid').trim() || 'rgba(224,238,255,.12)',
  };
}

async function drawHistory() {
  const requestedTarget = config?.activeTargetId;
  if (!requestedTarget || document.hidden) return;
  const generation = ++chartGeneration;
  const points = await invoke('get_history', { targetId: requestedTarget });
  // An older asynchronous history response must never replace the new target.
  if (generation !== chartGeneration || config?.activeTargetId !== requestedTarget || document.hidden) return;
  const canvas = $('historyChart');
  const wrap = canvas.parentElement;
  const rect = wrap.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  const pixelWidth = Math.max(1, Math.round(rect.width * dpr));
  const pixelHeight = Math.max(1, Math.round(rect.height * dpr));
  // Resizing the canvas reallocates backing storage; do it only when needed.
  if (canvas.width !== pixelWidth) canvas.width = pixelWidth;
  if (canvas.height !== pixelHeight) canvas.height = pixelHeight;
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

  const runs = [];
  let currentRun = [];
  for (const point of points) {
    if (point.latencyMs == null) {
      if (currentRun.length) runs.push(currentRun);
      currentRun = [];
    } else {
      currentRun.push({ x: xFor(point.timestampMs), y: yFor(point.latencyMs) });
    }
  }
  if (currentRun.length) runs.push(currentRun);

  const shade = ctx.createLinearGradient(0, top, 0, top + plotH);
  shade.addColorStop(0, 'rgba(134,191,255,.22)');
  shade.addColorStop(1, 'rgba(134,191,255,0)');
  for (const run of runs) {
    if (run.length > 1) {
      ctx.beginPath();
      ctx.moveTo(run[0].x, top + plotH);
      for (const p of run) ctx.lineTo(p.x, p.y);
      ctx.lineTo(run[run.length - 1].x, top + plotH);
      ctx.closePath();
      ctx.fillStyle = shade;
      ctx.fill();
    }
    ctx.beginPath();
    run.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y));
    ctx.strokeStyle = colors.accent;
    ctx.lineWidth = 2;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();
  }
  const lastRun = runs[runs.length - 1];
  const latest = lastRun?.[lastRun.length - 1];
  if (latest) {
    ctx.beginPath();
    ctx.arc(latest.x, latest.y, 3.1, 0, 2 * Math.PI);
    ctx.fillStyle = colors.accent;
    ctx.fill();
    ctx.beginPath();
    ctx.arc(latest.x, latest.y, 5.8, 0, 2 * Math.PI);
    ctx.strokeStyle = 'rgba(134,191,255,.28)';
    ctx.lineWidth = 2;
    ctx.stroke();
  }

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
  if (chartTimer) {
    if (!force) return;
    clearTimeout(chartTimer);
  }
  chartTimer = setTimeout(() => {
    chartTimer = 0;
    drawHistory().catch(err => console.error('History chart:', err));
  }, force ? 0 : 150);
}

async function loadSnapshots() {
  const list = await invoke('get_all_snapshots');
  snapshots = new Map(list.map(s => [s.targetId, s]));
  renderTargetRows();
}

function setupSectionNavigation() {
  const links = [...document.querySelectorAll('.section-nav a[href^="#"]')];
  const setActive = id => {
    for (const link of links) {
      const active = link.getAttribute('href') === `#${id}`;
      link.classList.toggle('is-active', active);
      if (active) link.setAttribute('aria-current', 'location');
      else link.removeAttribute('aria-current');
    }
  };
  for (const link of links) {
    link.addEventListener('click', () => setActive(link.hash.slice(1)));
  }
  if (!('IntersectionObserver' in window)) {
    if (links.length) setActive(links[0].hash.slice(1));
    return;
  }
  const observer = new IntersectionObserver(entries => {
    const visible = entries.filter(entry => entry.isIntersecting)
      .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top);
    if (visible[0]) setActive(visible[0].target.id);
  }, { rootMargin: '-85px 0px -65% 0px', threshold: 0 });
  for (const link of links) {
    const section = document.getElementById(link.hash.slice(1));
    if (section) observer.observe(section);
  }
  if (links.length) setActive(links[0].hash.slice(1));
}

async function boot() {
  setupSectionNavigation();
  config = await invoke('get_config');
  renderConfig();
  $('paused').checked = await invoke('is_paused');
  renderBatteryStatus(await invoke('get_battery'));

  await loadSnapshots();
  const first = await invoke('get_snapshot');
  snapshots.set(first.targetId, first);
  renderSnapshot(first);
  renderTargetRows();
  queueChart(true);

  async function selectTarget(id) {
    if (!id || id === config.activeTargetId || switchingTarget) return;
    switchingTarget = true;
    const oldId = config.activeTargetId;
    updateActiveTargetFromForm();
    config.activeTargetId = id;
    try {
      const ok = await save(false, false);
      if (!ok) {
        config.activeTargetId = oldId;
        $('activeTarget').value = oldId;
        renderTargetRows();
        return;
      }
      const snapshot = snapshots.get(id);
      if (snapshot) renderSnapshot(snapshot);
      queueChart(true);
    } finally {
      switchingTarget = false;
    }
  }

  $('activeTarget').addEventListener('change', () => selectTarget($('activeTarget').value));
  $('targetRows').addEventListener('click', event => {
    const row = event.target.closest('[data-target-id]');
    if (row) selectTarget(row.dataset.targetId);
  });

  for (const id of ['name', 'host', 'port', 'intervalMs', 'timeoutMs', 'addressFamily', 'probeMode', 'httpPath', 'targetEnabled']) {
    $(id).addEventListener('change', updateActiveTargetFromForm);
  }

  $('probeMode').addEventListener('change', updateProtocolFields);

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
      probeMode: 'tcp',
      httpPath: '/',
    });
    config.activeTargetId = id;
    refreshTargetSelect();
    renderTargetRows();
    setDirty(true);
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
    setDirty(true);
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
  $('floatingBackgroundMode').addEventListener('change', updateFloatingRangeLabels);

  $('save').addEventListener('click', () => save(true, true));
  document.addEventListener('input', event => {
    if (!event.target.closest('input, select')) return;
    if (!['activeTarget', 'paused', 'mousePassthrough'].includes(event.target.id)) setDirty(true);
  });
  document.addEventListener('change', event => {
    const id = event.target.id;
    if (id && !['activeTarget', 'paused', 'mousePassthrough'].includes(id)) setDirty(true);
  });

  // The active probe emits both target-update (for the table) and
  // latency-update (for the active summary/HUD). Table refreshes are batched
  // to one animation frame so simultaneous multi-target updates coalesce.
  await listen('latency-update', event => {
    const s = event.payload;
    const previous = snapshots.get(s.targetId);
    snapshots.set(s.targetId, s);
    if (s.targetId === config.activeTargetId) {
      renderSnapshot(s);
      queueChart();
    }
    if (previous?.paused !== s.paused) queueTargetRows();
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

  await listen('battery-update', event => renderBatteryStatus(event.payload));

  window.addEventListener('resize', () => queueChart(true));
  document.addEventListener('visibilitychange', () => {
    if (!document.hidden) queueChart(true);
  });
}

async function save(showSuccess = true, updateForm = true) {
  if (updateForm && !validateTargetForm()) return false;
  if (updateForm) updateActiveTargetFromForm();
  const button = $('save');
  button.disabled = true;
  button.setAttribute('aria-busy', 'true');
  config.showFloating = $('showFloating').checked;
  config.mousePassthrough = $('mousePassthrough').checked;
  config.floatingShowTarget = $('floatingShowTarget').checked;
  config.floatingShowStatusDot = $('floatingShowStatusDot').checked;
  config.floatingShowTrend = $('floatingShowTrend').checked;
  config.floatingBackgroundMode = normalizedFloatingBackgroundMode($('floatingBackgroundMode').value);
  config.floatingSize = $('floatingSize').value;
  config.floatingOpacity = Number($('floatingOpacity').value);
  config.floatingFontSize = Number($('floatingFontSize').value);
  config.uiVersion = 8;
  config.autostart = $('autostart').checked;
  config.batteryPauseOnBattery = $('batteryPauseOnBattery').checked;
  config.batteryPauseLowEnabled = $('batteryPauseLowEnabled').checked;
  config.batteryLowThresholdPercent = Number($('batteryLowThreshold').value);
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
    setDirty(false);
    if (showSuccess) showMessage('配置已保存');
    return true;
  } catch (err) {
    showMessage(String(err), true);
    return false;
  } finally {
    button.disabled = false;
    button.removeAttribute('aria-busy');
  }
}

function showMessage(text, error = false) {
  $('message').textContent = text;
  $('message').className = error ? 'error' : 'success';
  clearTimeout(showMessage.timer);
  showMessage.timer = setTimeout(() => {
    $('message').textContent = error ? '请修正后重试' : MESSAGE_IDLE;
    $('message').className = error ? 'error' : '';
  }, 3500);
}

boot().catch(err => showMessage(String(err), true));
