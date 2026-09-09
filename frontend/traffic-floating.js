const trafficRowEl = document.getElementById('trafficRow');
const downloadRateEl = document.getElementById('downloadRate');
const uploadRateEl = document.getElementById('uploadRate');
let lastTrafficKey = '';

function compactRate(bytesPerSecond) {
  const value = Number(bytesPerSecond || 0);
  if (value >= 1_000_000_000) {
    const scaled = value / 1_000_000_000;
    return `${scaled < 10 ? scaled.toFixed(1) : Math.round(scaled)}G`;
  }
  if (value >= 1_000_000) {
    const scaled = value / 1_000_000;
    return `${scaled < 10 ? scaled.toFixed(1) : Math.round(scaled)}M`;
  }
  if (value >= 1_000) {
    const scaled = value / 1_000;
    return `${scaled < 10 ? scaled.toFixed(1) : Math.round(scaled)}K`;
  }
  return `${Math.round(value)}B`;
}

function fullRate(bytesPerSecond) {
  const value = Number(bytesPerSecond || 0);
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(2)} GB/s`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)} MB/s`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)} KB/s`;
  return `${Math.round(value)} B/s`;
}

function applyTrafficVisibility(nextConfig) {
  if (!trafficRowEl) return;
  trafficRowEl.hidden = nextConfig?.floatingShowTraffic === false;
}

function renderTraffic(snapshot) {
  if (!trafficRowEl || !downloadRateEl || !uploadRateEl) return;

  if (!snapshot?.enabled || snapshot.status === 'disabled') {
    const key = 'disabled';
    if (key !== lastTrafficKey) {
      downloadRateEl.textContent = '--';
      uploadRateEl.textContent = '--';
      trafficRowEl.title = '网络流量显示已关闭';
      lastTrafficKey = key;
    }
    return;
  }

  if (snapshot.status === 'unavailable') {
    const interfaceName = snapshot.interfaceName || 'auto';
    const key = `unavailable|${interfaceName}`;
    if (key !== lastTrafficKey) {
      downloadRateEl.textContent = '--';
      uploadRateEl.textContent = '--';
      trafficRowEl.title = `无法读取网络接口 ${interfaceName}`;
      lastTrafficKey = key;
    }
    return;
  }

  const download = Number(snapshot.downloadBytesPerSec || 0);
  const upload = Number(snapshot.uploadBytesPerSec || 0);
  const interfaceName = snapshot.interfaceName || 'auto';
  const downloadText = snapshot.status === 'starting' ? '--' : compactRate(download);
  const uploadText = snapshot.status === 'starting' ? '--' : compactRate(upload);
  const key = `${downloadText}|${uploadText}|${interfaceName}|${snapshot.status}`;
  if (key === lastTrafficKey) return;

  downloadRateEl.textContent = downloadText;
  uploadRateEl.textContent = uploadText;
  trafficRowEl.title = snapshot.status === 'starting'
    ? `正在建立 ${interfaceName} 流量基线…`
    : `下载 ${fullRate(download)} · 上传 ${fullRate(upload)} · ${interfaceName}`;
  lastTrafficKey = key;
}

async function bootTrafficHud() {
  if (!trafficRowEl) return;
  const initialConfig = await invoke('get_config');
  applyTrafficVisibility(initialConfig);

  await listen('traffic-update', event => renderTraffic(event.payload));
  await listen('config-update', event => applyTrafficVisibility(event.payload));
}

bootTrafficHud().catch(err => {
  if (!trafficRowEl) return;
  downloadRateEl.textContent = '--';
  uploadRateEl.textContent = '--';
  trafficRowEl.title = `流量监测初始化失败: ${String(err)}`;
});
