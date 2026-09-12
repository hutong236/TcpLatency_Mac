import test from 'node:test';
import assert from 'node:assert/strict';
import { PetStateMachine, classifyPet, currentLatency, validTraffic, rateText, isPetMode } from '../frontend/pet-state.mjs';

const config = { thresholds: { warningMs: 50, highMs: 100, criticalMs: 200 }, floatingShowTraffic: true };
const sample = (timestampMs, extra = {}) => ({ targetId: 'a', host: 'example.com', port: 443, enabled: true,
  status: 'ok', currentMs: 23, timestampMs, jitterMs: 1, failurePercent: 0, ...extra });
const traffic = (extra = {}) => ({ status: 'ok', enabled: true, timestampMs: 1000, downloadBytesPerSec: 8e6, uploadBytesPerSec: 0, ...extra });

test('legacy config stays in HUD mode and cat disable overrides 3D mode', () => {
  assert.equal(isPetMode({}), false);
  assert.equal(isPetMode({ floatingDisplayMode: 'pet3d' }), true);
  assert.equal(isPetMode({ floatingDisplayMode: 'pet3d', networkCatEnabled: false }), false);
});
test('TCP error labels describe the target failure, not a global internet outage', () => {
  for (const [status, label] of [['timeout', '目标超时'], ['refused', '连接被拒绝'], ['dns_error', 'DNS 解析失败'], ['dns_timeout', 'DNS 超时']]) {
    const s = sample(1, { status, currentMs: 23 });
    assert.equal(classifyPet(s, null, config).label, label);
    assert.equal(currentLatency(s), '--');
  }
});
test('paused, stale, and disabled samples never display an old RTT', () => {
  for (const override of [{ paused: true }, { status: 'stale' }, { enabled: false }]) {
    assert.equal(currentLatency(sample(1, override)), '--');
    assert.equal(classifyPet(sample(1, override), null, config).alarm, false);
  }
  assert.equal(currentLatency(sample(1, { currentMs: 0 })), '0');
});
test('one high sample does not change pose; repeated traffic events cannot count as probes', () => {
  const machine = new PetStateMachine();
  machine.update(sample(1), null, config, 1);
  const high = sample(2, { currentMs: 90 });
  assert.equal(machine.update(high, null, config, 2).kind, 'normal');
  for (let i = 0; i < 10; i++) assert.equal(machine.update(high, traffic(), config, 3 + i).kind, 'normal');
  assert.equal(machine.update(sample(3, { currentMs: 90 }), null, config, 14).kind, 'slow');
});
test('failure is immediate and recovery requires two healthy samples, then expires', () => {
  const machine = new PetStateMachine();
  assert.equal(machine.update(sample(1, { status: 'timeout' }), null, config, 1000).kind, 'failure');
  assert.equal(machine.update(sample(2), null, config, 2000).recovering, false);
  assert.equal(machine.update(sample(3), null, config, 3000).recovering, true);
  assert.equal(machine.update(sample(3), null, config, 4900).recovering, false);
});
test('changing target or endpoint never celebrates a different target as recovered', () => {
  for (const change of [{ targetId: 'b' }, { host: 'different.test' }, { port: 80 }]) {
    const machine = new PetStateMachine();
    machine.update(sample(1, { status: 'timeout' }), null, config, 1);
    const next = machine.update(sample(2, change), null, config, 2);
    assert.equal(next.recovering, false); assert.equal(next.kind, 'normal');
  }
});
test('alternating high/normal samples do not accumulate toward a false transition', () => {
  const machine = new PetStateMachine(); machine.update(sample(1), null, config, 1);
  for (let i = 2; i < 20; i++) assert.equal(machine.update(sample(i, { currentMs: i % 2 ? 23 : 90 }), null, config, i).kind, 'normal');
});
test('failure interrupts recovery and pause does not produce recovery animation', () => {
  const machine = new PetStateMachine();
  machine.update(sample(1, { status: 'timeout' }), null, config, 1);
  machine.update(sample(2), null, config, 2); machine.update(sample(3), null, config, 3);
  assert.equal(machine.update(sample(4, { status: 'dns_error' }), null, config, 4).recovering, false);
  machine.update(sample(5, { paused: true }), null, config, 5);
  machine.update(sample(6), null, config, 6);
  assert.equal(machine.update(sample(7), null, config, 7).recovering, false);
});
test('traffic controls activity only when fresh, available, and enabled', () => {
  assert.equal(classifyPet(sample(1), traffic(), config, 1001).kind, 'busy');
  for (const t of [traffic({ status: 'unavailable' }), traffic({ enabled: false }), traffic({ timestampMs: 0 })]) {
    assert.equal(classifyPet(sample(1), t, config, 6000).kind, 'normal');
  }
  assert.equal(classifyPet(sample(1), traffic(), { ...config, floatingShowTraffic: false }, 1001).kind, 'normal');
  assert.equal(validTraffic(null), false);
});
test('high traffic cannot mask high RTT, jitter, or target errors', () => {
  assert.equal(classifyPet(sample(1, { currentMs: 250 }), traffic(), config, 1001).kind, 'critical');
  assert.equal(classifyPet(sample(1, { jitterMs: 30 }), traffic(), config, 1001).kind, 'unstable');
  assert.equal(classifyPet(sample(1, { status: 'timeout' }), traffic(), config, 1001).kind, 'failure');
});
test('rate labels include units and reject invalid values', () => {
  assert.equal(rateText(0), '0 B/s'); assert.equal(rateText(12_500_000), '12.5 MB/s');
  assert.equal(rateText(-1), '--'); assert.equal(rateText(NaN), '--');
});
