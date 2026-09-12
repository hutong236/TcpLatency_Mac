// Pure presentation rules: these never schedule probes or alter collected data.
export const FAILURE_LABELS = Object.freeze({
  timeout: '目标超时', refused: '连接被拒绝', offline: '目标不可达',
  dns_timeout: 'DNS 超时', dns_error: 'DNS 解析失败',
});

export function isPetMode(config) {
  return config?.floatingDisplayMode === 'pet3d' && config.networkCatEnabled !== false;
}

export function validTraffic(traffic, now = Date.now()) {
  return traffic?.enabled === true && traffic.status === 'ok'
    && Number.isFinite(traffic.timestampMs) && now - traffic.timestampMs < 5000;
}

export function rateText(bytes) {
  if (!Number.isFinite(bytes) || bytes < 0) return '--';
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB/s`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB/s`;
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(0)} KB/s`;
  return `${Math.round(bytes)} B/s`;
}

export function classifyPet(snapshot, traffic, config, now = Date.now()) {
  if (!snapshot) return { kind: 'idle', label: '等待采样', alarm: false };
  if (snapshot.paused) return { kind: 'paused', label: '已暂停', alarm: false };
  if (snapshot.enabled === false || snapshot.status === 'disabled') return { kind: 'disabled', label: '目标已停用', alarm: false };
  if (snapshot.status === 'stale') return { kind: 'stale', label: '数据已过期', alarm: false };
  if (FAILURE_LABELS[snapshot.status]) return { kind: 'failure', label: FAILURE_LABELS[snapshot.status], alarm: true };
  if (snapshot.status !== 'ok' || !Number.isFinite(snapshot.currentMs)) return { kind: 'idle', label: '等待采样', alarm: false };
  const { warningMs = 50, criticalMs = 200 } = config?.thresholds || {};
  if (snapshot.currentMs >= criticalMs) return { kind: 'critical', label: '延迟过高', alarm: true };
  if (snapshot.currentMs >= warningMs) return { kind: 'slow', label: '延迟偏高', alarm: true };
  if (snapshot.jitterMs >= Math.max(25, snapshot.currentMs * .65) || snapshot.failurePercent >= 5) {
    return { kind: 'unstable', label: '近期探测不稳定', alarm: true };
  }
  const busy = config?.floatingShowTraffic !== false && validTraffic(traffic, now)
    && traffic.downloadBytesPerSec + traffic.uploadBytesPerSec >= 5e6;
  return busy ? { kind: 'busy', label: '传输忙碌', alarm: false } : { kind: 'normal', label: '连接稳定', alarm: false };
}

export function currentLatency(snapshot) {
  return snapshot && !snapshot.paused && snapshot.enabled !== false && snapshot.status === 'ok'
    && Number.isFinite(snapshot.currentMs) ? String(Math.round(snapshot.currentMs)) : '--';
}

// Only distinct TCP samples advance hysteresis. Traffic events and repeated
// snapshots must not count as extra probes or manufacture a recovery.
export class PetStateMachine {
  constructor() { this.reset(); }
  reset() {
    this.identity = null;
    this.state = null;
    this.candidate = '';
    this.count = 0;
    this.sampleKey = null;
    this.recoveredUntil = 0;
  }
  update(snapshot, traffic, config, now = Date.now()) {
    const identity = snapshot ? `${snapshot.targetId}|${snapshot.host}|${snapshot.port}` : '';
    if (identity !== this.identity) { this.reset(); this.identity = identity; }
    const next = classifyPet(snapshot, traffic, config, now);
    const key = snapshot ? `${snapshot.timestampMs}|${snapshot.status}|${!!snapshot.paused}|${snapshot.enabled}` : '';
    const fresh = key !== this.sampleKey;
    this.sampleKey = key;
    const immediate = ['failure', 'paused', 'disabled', 'idle', 'stale'].includes(next.kind);
    const trafficOnly = this.state && !this.state.alarm && !next.alarm
      && ['normal', 'busy'].includes(this.state.kind) && ['normal', 'busy'].includes(next.kind);
    if (!this.state || immediate || trafficOnly) {
      this.state = next;
      this.count = 0;
      this.candidate = '';
      if (immediate) this.recoveredUntil = 0;
    } else if (next.kind === this.state.kind) {
      this.state = next;
      this.count = 0;
      this.candidate = '';
    } else if (fresh) {
      this.count = this.candidate === next.kind ? this.count + 1 : 1;
      this.candidate = next.kind;
      if (this.count >= 2) {
        if (this.state.alarm && !next.alarm) this.recoveredUntil = now + 1800;
        else this.recoveredUntil = 0;
        this.state = next;
        this.count = 0;
        this.candidate = '';
      }
    }
    return { ...this.state, label: next.label, currentKind: next.kind,
      recovering: this.recoveredUntil > now && !next.alarm,
      recoveryStart: this.recoveredUntil - 1800 };
  }
}
