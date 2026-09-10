let trafficControlsInstalled = false;
let catControlsInstalled = false;

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

function installCatControls() {
  if (catControlsInstalled) return;
  const section = [...document.querySelectorAll('section.card')]
    .find(card => card.querySelector('h2')?.textContent?.includes('悬浮窗与启动'));
  const anchor = section?.querySelector('.compact-grid');
  if (!anchor) return;

  anchor.insertAdjacentHTML('beforebegin', `
    <div id="networkCatSettings" class="cat-settings-block">
      <label class="toggle-row">
        <span>
          <b>显示网络猫</b>
          <small>在延迟数字旁显示网络状态动画；关闭后不占用 HUD 空间</small>
        </span>
        <input id="networkCatEnabled" type="checkbox" />
      </label>
      <label class="toggle-row">
        <span>
          <b>网络猫动画</b>
          <small>关闭后保留静态主题；自定义 GIF/WebP 会冻结为当前帧</small>
        </span>
        <input id="networkCatAnimationEnabled" type="checkbox" />
      </label>
      <div class="grid two">
        <label>动画主题
          <select id="networkCatTheme">
            <option value="default">Default Cat · 极简轮廓</option>
            <option value="pixel">Pixel Cat · 8-bit 像素</option>
            <option value="cyber">Cyber Cat · 科技数据流</option>
            <option value="custom">Custom · 自定义素材</option>
          </select>
        </label>
        <label id="networkCatFileRow">选择自定义素材
          <input id="networkCatFile" type="file" accept=".svg,.png,.apng,.webp,.gif,image/svg+xml,image/png,image/webp,image/gif" />
          <small>支持 SVG / PNG / APNG / WebP / GIF，最大 8MB；会复制到应用配置目录。</small>
        </label>
      </div>
      <label id="networkCatPathRow">自定义素材路径
        <input id="networkCatCustomAsset" placeholder="选择文件后自动填写，也可手工输入绝对路径" spellcheck="false" autocomplete="off" />
      </label>
      <div class="target-actions">
        <button id="testCatAsset" class="secondary" type="button">测试素材</button>
        <span id="catAssetResult" class="test-result">网络猫融合状态会同时参考延迟、抖动、失败率和实时流量。</span>
      </div>
    </div>
  `);

  catControlsInstalled = true;
  for (const id of ['networkCatEnabled', 'networkCatAnimationEnabled', 'networkCatTheme', 'networkCatCustomAsset']) {
    document.getElementById(id)?.addEventListener('change', () => {
      syncCatControlsToConfig();
      updateCatControlState();
    });
  }
  document.getElementById('networkCatCustomAsset')?.addEventListener('input', syncCatControlsToConfig);
  document.getElementById('networkCatFile')?.addEventListener('change', importCatAsset);
  document.getElementById('testCatAsset')?.addEventListener('click', testCatAsset);
  document.getElementById('save')?.addEventListener('click', syncCatControlsToConfig, true);
}

function updateCatControlState() {
  const enabled = document.getElementById('networkCatEnabled');
  const animation = document.getElementById('networkCatAnimationEnabled');
  const theme = document.getElementById('networkCatTheme');
  const fileRow = document.getElementById('networkCatFileRow');
  const pathRow = document.getElementById('networkCatPathRow');
  const active = enabled?.checked !== false;
  if (animation) animation.disabled = !active;
  if (theme) theme.disabled = !active;
  const custom = active && theme?.value === 'custom';
  if (fileRow) fileRow.hidden = !custom;
  if (pathRow) pathRow.hidden = !custom;
}

function renderCatControls(nextConfig) {
  installCatControls();
  const enabled = document.getElementById('networkCatEnabled');
  const animation = document.getElementById('networkCatAnimationEnabled');
  const theme = document.getElementById('networkCatTheme');
  const path = document.getElementById('networkCatCustomAsset');
  if (!enabled || !animation || !theme || !path) return;

  enabled.checked = nextConfig?.networkCatEnabled !== false;
  animation.checked = nextConfig?.networkCatAnimationEnabled !== false;
  theme.value = nextConfig?.networkCatTheme || 'default';
  path.value = nextConfig?.networkCatCustomAsset || '';
  updateCatControlState();
}

function syncCatControlsToConfig() {
  if (typeof config === 'undefined' || !config) return;
  const enabled = document.getElementById('networkCatEnabled');
  const animation = document.getElementById('networkCatAnimationEnabled');
  const theme = document.getElementById('networkCatTheme');
  const path = document.getElementById('networkCatCustomAsset');
  if (!enabled || !animation || !theme || !path) return;

  config.networkCatEnabled = enabled.checked;
  config.networkCatAnimationEnabled = animation.checked;
  config.networkCatTheme = theme.value;
  config.networkCatCustomAsset = path.value.trim();
  config.uiVersion = 8;
}

async function importCatAsset(event) {
  const input = event.currentTarget;
  const file = input?.files?.[0];
  if (!file) return;
  const result = document.getElementById('catAssetResult');
  if (file.size > 8 * 1024 * 1024) {
    if (result) {
      result.className = 'test-result error';
      result.textContent = '素材超过 8MB，请压缩后再试。';
    }
    input.value = '';
    return;
  }

  try {
    if (result) {
      result.className = 'test-result';
      result.textContent = '正在导入网络猫素材…';
    }
    const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
    const savedPath = await invoke('save_cat_asset', { fileName: file.name, data: bytes });
    document.getElementById('networkCatCustomAsset').value = savedPath;
    document.getElementById('networkCatTheme').value = 'custom';
    syncCatControlsToConfig();
    updateCatControlState();
    if (result) {
      result.className = 'test-result success';
      result.textContent = `已导入 ${file.name} · ${(file.size / 1024).toFixed(file.size < 1024 * 1024 ? 0 : 1)} KB`;
    }
  } catch (err) {
    if (result) {
      result.className = 'test-result error';
      result.textContent = String(err);
    }
  } finally {
    input.value = '';
  }
}

async function testCatAsset() {
  const result = document.getElementById('catAssetResult');
  const path = document.getElementById('networkCatCustomAsset')?.value.trim();
  if (!path) {
    if (result) {
      result.className = 'test-result error';
      result.textContent = '请先选择或填写自定义素材路径。';
    }
    return;
  }
  try {
    if (result) {
      result.className = 'test-result';
      result.textContent = '正在读取素材…';
    }
    const asset = await invoke('load_cat_asset', { path });
    if (result) {
      result.className = 'test-result success';
      result.textContent = `素材可用 · ${asset.fileName} · ${asset.mimeType} · ${Math.round((asset.data?.length || 0) / 1024)} KB`;
    }
  } catch (err) {
    if (result) {
      result.className = 'test-result error';
      result.textContent = String(err);
    }
  }
}

async function bootTrafficSettings() {
  installTrafficControls();
  installCatControls();
  const eyebrow = document.querySelector('.eyebrow');
  if (eyebrow) eyebrow.textContent = 'TCP LATENCY · V0.11.0';
  const initialConfig = await invoke('get_config');
  renderTrafficControls(initialConfig);
  renderCatControls(initialConfig);

  await listen('config-update', event => {
    renderTrafficControls(event.payload);
    renderCatControls(event.payload);
  });
}

bootTrafficSettings().catch(err => {
  console.error('悬浮窗扩展设置初始化失败:', err);
});
