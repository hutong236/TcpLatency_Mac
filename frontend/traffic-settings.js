let trafficControlsInstalled = false;
let catControlsInstalled = false;

// Controls are present in settings.html; no runtime HTML injection or layout shift.
function installTrafficControls() {
  if (trafficControlsInstalled) return;
  const showTraffic = document.getElementById('floatingShowTraffic');
  const trafficInterface = document.getElementById('trafficInterface');
  if (!showTraffic || !trafficInterface) return;
  trafficControlsInstalled = true;
  showTraffic.addEventListener('change', syncTrafficControlsToConfig);
  trafficInterface.addEventListener('input', syncTrafficControlsToConfig);
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
  if (!document.getElementById('networkCatSettings')) return;
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
