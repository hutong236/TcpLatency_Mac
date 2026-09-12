// Reproducible WebView UI regression check. Synthetic IPC is installed only by
// this runner; production HTML never loads a mock or a preview-only code path.
import { chromium } from 'playwright';
import { createServer } from 'node:http';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';

const root = fileURLToPath(new URL('../frontend/', import.meta.url));
const output = fileURLToPath(new URL('../test-results/', import.meta.url));
await mkdir(output, { recursive: true });
const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, 'http://localhost').pathname;
  if (pathname === '/favicon.ico') { response.writeHead(204); response.end(); return; }
  const file = path.resolve(root, `.${pathname}`);
  if (!file.startsWith(root)) { response.writeHead(403); response.end(); return; }
  try {
    const body = await readFile(file);
    const ext = path.extname(file);
    response.writeHead(200, { 'Content-Type': ({ '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css' })[ext] || 'text/plain' });
    response.end(body);
  } catch { response.writeHead(404); response.end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const address = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
const context = await browser.newContext({ viewport: { width: 280, height: 304 }, deviceScaleFactor: 2, colorScheme: 'light' });
await context.addInitScript(() => {
  let config = JSON.parse(sessionStorage.getItem('config') || 'null') || {
    targets: [{ id: 'internet', name: 'Internet', host: '1.1.1.1', port: 443, intervalMs: 1000, timeoutMs: 2000, addressFamily: 'auto', enabled: true }],
    activeTargetId: 'internet', showFloating: true, mousePassthrough: false, autostart: false,
    notificationsEnabled: false, notifyRecovery: true, notifyConsecutiveHigh: 3, notifyConsecutiveFailure: 3,
    notificationCooldownSec: 300, thresholds: { warningMs: 50, highMs: 100, criticalMs: 200 },
    floatingDisplayMode: 'pet3d', floatingSize: 'standard', floatingBackgroundMode: 'glass', floatingOpacity: .82,
    floatingFontSize: 42, floatingShowTarget: true, floatingShowTraffic: true, floatingShowStatusDot: true,
    networkCatEnabled: true, networkCatAnimationEnabled: true, networkCatTheme: 'default', networkCatCustomAsset: '',
    pet3dPowerSaver: true, trafficInterface: 'auto', uiVersion: 9,
  };
  let snapshot = { targetId: 'internet', targetName: 'Internet', host: '1.1.1.1', port: 443, enabled: true,
    status: 'ok', currentMs: 23, jitterMs: 2, failurePercent: 0, paused: false, timestampMs: Date.now(), sampleAgeMs: 0 };
  const events = new Map(), calls = [];
  const emit = (name, payload) => { for (const fn of events.get(name) || []) fn({ payload: structuredClone(payload) }); };
  const save = () => { sessionStorage.setItem('config', JSON.stringify(config)); emit('config-update', config); };
  window.qa = {
    calls, getConfig: () => config,
    config: patch => { Object.assign(config, patch); save(); },
    sample: patch => { snapshot = { ...snapshot, timestampMs: snapshot.timestampMs + 1, ...patch }; emit('latency-update', snapshot); },
    traffic: patch => emit('traffic-update', { enabled: true, status: 'ok', interfaceName: 'en0', timestampMs: Date.now(), downloadBytesPerSec: 850000, uploadBytesPerSec: 130000, ...patch }),
  };
  window.__TAURI__ = {
    core: { invoke: async (name, args) => {
      calls.push({ name, args });
      if (name === 'get_config') return structuredClone(config);
      if (name === 'get_snapshot') return structuredClone(snapshot);
      if (name === 'get_all_snapshots') return [structuredClone(snapshot)];
      if (name === 'get_history') return [];
      if (name === 'is_paused') return false;
      if (name === 'save_config') { config = structuredClone(args.config); save(); return config; }
      if (name === 'set_mouse_passthrough') { config.mousePassthrough = args.enabled; save(); return args.enabled; }
      if (name === 'show_settings') return;
      throw Error(`Unexpected mock IPC: ${name}`);
    } },
    event: { listen: async (name, fn) => { if (!events.has(name)) events.set(name, new Set()); events.get(name).add(fn); return () => events.get(name).delete(fn); } },
  };
});
const page = await context.newPage();
const errors = [], externalRequests = [], checks = [];
page.on('pageerror', error => errors.push(String(error)));
page.on('request', request => { if (!request.url().startsWith(address)) externalRequests.push(request.url()); });
const expect = async (name, fn) => { await fn(); checks.push(name); console.log(`PASS ${name}`); };
const attr = (name, value) => page.waitForFunction(([name, value]) => document.getElementById('pet3d').dataset[name] === value, [name, value]);
const setConfig = patch => page.evaluate(patch => window.qa.config(patch), patch);
const sample = patch => page.evaluate(patch => { window.qa.sample(patch); window.qa.sample(patch); }, patch);
const screenshot = async name => {
  await page.waitForTimeout(250); // Allow pose blending and the first GPU frame.
  return page.screenshot({ path: path.join(output, `${name}.png`), omitBackground: true });
};

try {
  await page.goto(`${address}/index.html`);
  await expect('real 3D canvas and live text render', async () => {
    await attr('renderer', 'ready'); assert.equal(await page.title(), 'TCP Latency · Glass HUD');
    assert.equal(await page.locator('#pet3dStage canvas').count(), 1);
    assert.equal(await page.locator('#pet3dFallback').isVisible(), false);
    assert.equal(await page.locator('#pet3dLatency').innerText(), '23');
    assert.equal(await page.locator('#floating').isVisible(), false);
    await page.evaluate(() => window.qa.traffic({}));
    assert.equal(await page.locator('#pet3dDownload').innerText(), '850 KB/s');
    await screenshot('3d-normal');
  });
  await expect('traffic, latency, failure, recovery and pause states', async () => {
    await page.evaluate(() => window.qa.traffic({ downloadBytesPerSec: 12.5e6 }));
    await attr('pose', 'busy'); await screenshot('3d-busy');
    await sample({ currentMs: 160 }); await attr('pose', 'slow');
    await sample({ status: 'timeout', currentMs: null }); await attr('pose', 'failure');
    assert.equal(await page.locator('#pet3dStatus').innerText(), '目标超时');
    assert.equal(await page.locator('#pet3dLatency').innerText(), '--'); await screenshot('3d-timeout');
    await sample({ status: 'ok', currentMs: 23 }); await attr('recovering', 'true');
    await sample({ paused: true }); await attr('pose', 'paused');
    assert.equal(await page.locator('#pet3dLatency').innerText(), '--');
    await sample({ paused: false, status: 'stale' }); await attr('pose', 'stale');
    await sample({ status: 'ok', currentMs: 23 });
    await page.evaluate(() => window.qa.traffic({ status: 'unavailable' }));
    assert.equal(await page.locator('#pet3dDownload').innerText(), '--');
  });
  await expect('three sizes fit, including a long target and maximum font', async () => {
    for (const [size, width, height] of [['compact', 228, 250], ['standard', 280, 304], ['large', 336, 356]]) {
      await page.setViewportSize({ width, height });
      await setConfig({ floatingSize: size, floatingFontSize: 52 });
      await sample({ targetName: 'Very long monitoring target name that must be ellipsized' });
      const metrics = await page.evaluate(() => {
        const stage = document.getElementById('pet3dStage').getBoundingClientRect();
        return { stageHeight: stage.height, width: document.body.scrollWidth, height: document.body.scrollHeight, innerWidth, innerHeight };
      });
      assert.ok(metrics.stageHeight > 60 && metrics.width <= metrics.innerWidth && metrics.height <= metrics.innerHeight, `${size} fits: ${JSON.stringify(metrics)}`);
      await screenshot(`3d-${size}`);
    }
  });
  await expect('disabled animation and Reduce Motion render a stable frame', async () => {
    await setConfig({ networkCatAnimationEnabled: false }); await attr('motion', 'static');
    // Compare the same state across time; this catches a hidden animation loop.
    const first = await page.locator('#pet3dStage').screenshot();
    await page.waitForTimeout(300);
    assert.ok(first.equals(await page.locator('#pet3dStage').screenshot()));
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await setConfig({ networkCatAnimationEnabled: true }); await attr('motion', 'static');
    await page.emulateMedia({ reducedMotion: 'no-preference' }); await attr('motion', 'animated');
  });
  await expect('hidden and 2D modes release the WebGL canvas', async () => {
    await setConfig({ showFloating: false }); await attr('renderer', 'stopped');
    assert.equal(await page.locator('#pet3dStage canvas').count(), 0);
    await setConfig({ showFloating: true }); await attr('renderer', 'ready');
    await setConfig({ floatingDisplayMode: 'hud' });
    assert.equal(await page.locator('#pet3dStage canvas').count(), 0);
    assert.equal(await page.locator('#floating').isVisible(), true);
    await setConfig({ floatingDisplayMode: 'pet3d', networkCatEnabled: false });
    assert.equal(await page.locator('#floating').isVisible(), true);
    await setConfig({ networkCatEnabled: true }); await attr('renderer', 'ready');
  });
  await expect('graphics context loss preserves numbers and supports retry', async () => {
    await page.evaluate(() => document.querySelector('#pet3dStage canvas').getContext('webgl2').getExtension('WEBGL_lose_context').loseContext());
    await attr('renderer', 'fallback');
    assert.equal(await page.locator('#pet3dFallback').isVisible(), true);
    await sample({ currentMs: 31 });
    assert.equal(await page.locator('#pet3dLatency').innerText(), '31'); await screenshot('3d-fallback');
    await page.getByRole('button', { name: '重试', exact: true }).click(); await attr('renderer', 'ready');
  });
  await expect('settings and lock buttons invoke the existing native commands', async () => {
    await page.getByRole('button', { name: '打开设置', exact: true }).click();
    assert.ok(await page.evaluate(() => window.qa.calls.some(call => call.name === 'show_settings')));
    await page.getByRole('button', { name: '锁定鼠标穿透', exact: true }).click();
    assert.equal(await page.evaluate(() => window.qa.getConfig().mousePassthrough), true);
    await setConfig({ mousePassthrough: false });
    await page.emulateMedia({ colorScheme: 'dark' }); await screenshot('3d-dark');
  });
  await expect('settings save and reload preserve mode and HUD background', async () => {
    await page.setViewportSize({ width: 760, height: 860 }); await page.goto(`${address}/settings.html`);
    await page.waitForFunction(() => document.getElementById('floatingDisplayMode').value === 'pet3d');
    assert.equal(await page.locator('#networkCatTheme').isDisabled(), true);
    await page.locator('#floatingDisplayMode').selectOption('hud');
    assert.equal(await page.locator('#networkCatTheme').isDisabled(), false);
    await page.locator('#floatingBackgroundMode').selectOption('solid');
    await page.locator('#save').click();
    await page.waitForFunction(() => window.qa.getConfig().floatingBackgroundMode === 'solid');
    await page.locator('#floatingDisplayMode').selectOption('pet3d');
    await page.locator('#pet3dPowerSaver').uncheck();
    await page.locator('#save').click();
    await page.waitForFunction(() => window.qa.getConfig().pet3dPowerSaver === false);
    await page.reload();
    await page.waitForFunction(() => document.getElementById('floatingDisplayMode').value === 'pet3d');
    assert.equal(await page.locator('#pet3dPowerSaver').isChecked(), false);
    assert.equal(await page.locator('#floatingBackgroundMode').inputValue(), 'solid');
    assert.equal(await page.locator('#floatingBackgroundMode').isDisabled(), true);
  });
  assert.deepEqual(errors, []); assert.deepEqual(externalRequests, []);
  console.log('PASS no uncaught errors or external asset requests');
} catch (error) {
  await screenshot('failure').catch(() => {});
  console.error(await page.locator('body').innerText().catch(() => 'No page'));
  throw error;
} finally {
  await writeFile(path.join(output, 'checks.json'), JSON.stringify({ checks, errors, externalRequests }, null, 2));
  await browser.close(); server.close();
}
