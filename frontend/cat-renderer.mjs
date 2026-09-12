import * as T from './vendor/three.module.min.js';
import { createCat } from './cat-model.mjs';

export class CatRenderer {
  constructor(host, onFailure) {
    this.host = host;
    this.onFailure = onFailure;
    this.disposed = false;
    this.frame = 0;
    this.timer = 0;
    this.motion = false;
    this.powerSaver = true;
    this.gaze = { x: 0, y: 0 };
    this.state = { kind: 'idle', recovering: false };
    this.last = 0;
    this.elapsed = 0;
    this.renderer = new T.WebGLRenderer({ alpha: true, antialias: true, powerPreference: 'low-power' });
    this.renderer.setClearColor(0x000000, 0);
    this.renderer.outputColorSpace = T.SRGBColorSpace;
    this.renderer.toneMapping = T.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.25;
    this.canvas = this.renderer.domElement;
    this.canvas.setAttribute('data-tauri-drag-region', '');
    this.canvas.setAttribute('aria-hidden', 'true');
    this.lost = event => { event.preventDefault(); this.fail('图形渲染已暂停'); };
    this.canvas.addEventListener('webglcontextlost', this.lost);
    try {
      this.scene = new T.Scene();
      this.camera = new T.PerspectiveCamera(33, 1, .1, 30);
      this.camera.position.set(2.1, 1.95, 4.8);
      this.camera.lookAt(0, 1.04, 0);
      this.scene.add(new T.HemisphereLight(0xfff7e5, 0x879aa7, 2.6));
      const key = new T.DirectionalLight(0xffeccf, 3.2); key.position.set(-3, 5, 4); this.scene.add(key);
      const rim = new T.DirectionalLight(0xd2efff, 1.8); rim.position.set(3, 2, -2); this.scene.add(rim);
      this.cat = createCat(); this.scene.add(this.cat.root);
      // A baked contact shadow avoids a per-frame shadow-map pass.
      const shadowCanvas = document.createElement('canvas'); shadowCanvas.width = shadowCanvas.height = 64;
      const ctx = shadowCanvas.getContext('2d');
      const gradient = ctx.createRadialGradient(32, 32, 2, 32, 32, 31);
      gradient.addColorStop(0, 'rgba(36,44,40,.26)'); gradient.addColorStop(1, 'rgba(36,44,40,0)');
      ctx.fillStyle = gradient; ctx.fillRect(0, 0, 64, 64);
      this.shadowTexture = new T.CanvasTexture(shadowCanvas);
      const shadow = new T.Mesh(new T.PlaneGeometry(2.5, 1.7), new T.MeshBasicMaterial({ map: this.shadowTexture, transparent: true, depthWrite: false }));
      shadow.rotation.x = -Math.PI / 2; shadow.position.y = .012; this.scene.add(shadow);
      host.append(this.canvas);
      this.resizeObserver = new ResizeObserver(() => { this.resize(); if (!this.motion) this.draw(); });
      this.resizeObserver.observe(host);
      this.resize();
      this.draw();
      if (this.disposed) throw new Error('无法绘制 3D 小猫');
    } catch (error) { this.dispose(); throw error; }
  }
  resize() {
    if (this.disposed) return;
    const width = Math.max(1, this.host.clientWidth), height = Math.max(1, this.host.clientHeight);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, this.powerSaver ? 1.5 : 2));
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height; this.camera.updateProjectionMatrix();
  }
  configure(motion, powerSaver) {
    const changed = this.motion !== motion || this.powerSaver !== powerSaver;
    this.motion = motion; this.powerSaver = powerSaver;
    if (!changed || this.disposed) return;
    this.stop(); this.resize(); this.last = 0;
    if (motion) this.tick(performance.now()); else this.draw();
  }
  setState(state) {
    const changed = state.kind !== this.state.kind;
    this.state = state;
    if (!this.motion && changed) this.draw();
  }
  look(x, y) { this.gaze.x = x; this.gaze.y = y; }
  draw(dt = 1) {
    if (this.disposed) return;
    try {
      this.cat.pose(this.state, this.elapsed, dt, this.gaze, Date.now(), this.motion);
      this.renderer.render(this.scene, this.camera);
    } catch (error) { this.fail(String(error)); }
  }
  tick(timestamp) {
    if (this.disposed || !this.motion) return;
    const dt = this.last ? Math.min(.1, (timestamp - this.last) / 1000) : 0;
    this.last = timestamp; this.elapsed += dt;
    this.draw(dt);
    if (this.disposed || !this.motion) return;
    // Schedule at 15/30 Hz instead of waking the WebView at display refresh rate.
    this.timer = setTimeout(() => {
      this.timer = 0;
      this.frame = requestAnimationFrame(time => { this.frame = 0; this.tick(time); });
    }, 1000 / (this.powerSaver ? 15 : 30));
  }
  stop() { clearTimeout(this.timer); cancelAnimationFrame(this.frame); this.timer = this.frame = 0; }
  fail(reason) { if (!this.disposed) { this.dispose(); this.onFailure(reason); } }
  dispose() {
    if (this.disposed) return;
    this.disposed = true; this.stop(); this.resizeObserver?.disconnect();
    this.canvas.removeEventListener('webglcontextlost', this.lost);
    const geometries = new Set(), materials = new Set();
    this.scene?.traverse(object => {
      if (object.geometry) geometries.add(object.geometry);
      if (object.material) for (const mat of [].concat(object.material)) materials.add(mat);
    });
    geometries.forEach(geometry => geometry.dispose());
    materials.forEach(material => material.dispose());
    this.shadowTexture?.dispose();
    this.renderer.dispose(); this.renderer.forceContextLoss(); this.canvas.remove();
  }
}
