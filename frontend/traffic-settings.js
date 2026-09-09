let trafficControlsInstalled = false;

function installTrafficControls() {
  if (trafficControlsInstalled) return;
  const section = [...document.querySelectorAll('section.card')]
    .find(card => card.querySelector('h2')?.textContent?.includes('悬浮窗与启动'));
  if (!section) return;

  const anchor = section.querySelector('.compact-grid');
  if (!anchor) return;

  anchor.insertAdjacentHTML('beforebegin', `
    <label class="toggle-row" id="trafficToggleRow">
      <span>
        <b>显示网络流量</b>
        <small>在延时数字下方显示当前 Mac 实时下载 / 上传速度</small>
      </span>
      <input id="floatingShowTraffic" type="checkbox" />
    </label>
    <label id="trafficInterfaceRow">流量接口
      <input id="trafficInterface" placeholder="auto / en0 / utun3" spellcheck="false" autocomplete="off" />
      <small>推荐使用 auto：跟随当前默认路由接口，避免 VPN 与物理网卡重复统计。</small>
    </label>
  `);

  trafficControlsInstalled = true;

  const showTraffic = document.getElementById('floatingShowTraffic');
  const trafficInterface = document.getElementById('trafficInterface');
  showTraffic?.addEventListener('change', syncTrafficControlsToConfig);
  trafficInterface?.addEventListener('input', syncTrafficControlsToConfig);
  trafficInterface?.addEventListener('change', syncTrafficControlsToConfig);
  document.getElementById('save')?.addEventListener('click', syncTrafficControlsToConfig, true);
}

function renderTrafficControls(nextConfig) {
  installTrafficControls();
  const showTraffic = document.getElementById('floatingShowTraffic');
  const trafficInterface = document.getElementById('trafficInterface');
  if (!showTraffic || !trafficInterface) return;

  showTraffic.checked = nextConfig?.floatingShowTraffic !== false;
  trafficInterface.value = nextConfig?.trafficInterface || 'auto';
  trafficInterface.disabled = !showTraffic.checked;
}

function syncTrafficControlsToConfig() {
  const showTraffic = document.getElementById('floatingShowTraffic');
  const trafficInterface = document.getElementById('trafficInterface');
  if (!showTraffic || !trafficInterface) return;

  trafficInterface.disabled = !showTraffic.checked;
  if (typeof config === 'undefined' || !config) return;

  config.floatingShowTraffic = showTraffic.checked;
  config.trafficInterface = (trafficInterface.value || 'auto').trim().toLowerCase();
}

async function bootTrafficSettings() {
  installTrafficControls();
  const initialConfig = await invoke('get_config');
  renderTrafficControls(initialConfig);

  await listen('config-update', event => renderTrafficControls(event.payload));
}

bootTrafficSettings().catch(err => {
  console.error('流量设置初始化失败:', err);
});
