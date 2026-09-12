import * as T from './vendor/three.module.min.js';

// Original articulated geometry, built locally. No downloaded character assets,
// texture requests, model loaders, or per-frame geometry allocations are needed.
export function createCat() {
  const root = new T.Group();
  const body = new T.Group(); root.add(body);
  const sphere = new T.SphereGeometry(1, 20, 14);
  const materials = {
    fur: new T.MeshStandardMaterial({ color: 0xe9a15e, roughness: .9 }),
    cream: new T.MeshStandardMaterial({ color: 0xfff2d9, roughness: .95 }),
    stripe: new T.MeshStandardMaterial({ color: 0xbd7040, roughness: .95 }),
    pink: new T.MeshStandardMaterial({ color: 0xe69490, roughness: .9 }),
    dark: new T.MeshStandardMaterial({ color: 0x283933, roughness: .45 }),
    white: new T.MeshBasicMaterial({ color: 0xffffff }),
    teal: new T.MeshStandardMaterial({ color: 0x43a89c, roughness: .65 }),
    gold: new T.MeshStandardMaterial({ color: 0xf8d785, roughness: .45, metalness: .25 }),
  };
  function ellipsoid(parent, mat, x, y, z, sx, sy, sz) {
    const mesh = new T.Mesh(sphere, materials[mat]);
    mesh.position.set(x, y, z); mesh.scale.set(sx, sy, sz); parent.add(mesh); return mesh;
  }
  function tube(parent, points, radius, mat, segments = 12) {
    const curve = new T.CatmullRomCurve3(points.map(p => new T.Vector3(...p)));
    const mesh = new T.Mesh(new T.TubeGeometry(curve, segments, radius, 8, false), materials[mat]);
    parent.add(mesh); return mesh;
  }
  ellipsoid(body, 'fur', 0, .7, 0, .43, .57, .33);
  ellipsoid(body, 'cream', 0, .65, .273, .29, .4, .075);
  for (const side of [-1, 1]) {
    ellipsoid(body, 'fur', side * .34, .29, -.04, .23, .28, .26);
    ellipsoid(body, 'cream', side * .36, .095, .1, .22, .095, .23);
  }
  const legs = [-1, 1].map(side => {
    const leg = new T.Group(); leg.position.set(side * .215, .55, .25); body.add(leg);
    ellipsoid(leg, 'fur', 0, -.2, 0, .125, .29, .13);
    ellipsoid(leg, 'cream', 0, -.43, .075, .15, .105, .19);
    return leg;
  });
  const collar = new T.Mesh(new T.TorusGeometry(.295, .047, 8, 30), materials.teal);
  collar.rotation.x = Math.PI / 2; collar.position.set(0, 1.06, .015); body.add(collar);
  ellipsoid(body, 'gold', 0, .99, .33, .065, .075, .028);

  const head = new T.Group(); head.position.y = 1.43; body.add(head);
  ellipsoid(head, 'fur', 0, 0, 0, .59, .47, .425);
  // Broad cheeks and a warm white muzzle keep the face legible at small sizes.
  ellipsoid(head, 'cream', -.15, -.19, .355, .185, .14, .105);
  ellipsoid(head, 'cream', .15, -.19, .355, .185, .14, .105);
  ellipsoid(head, 'pink', 0, -.135, .452, .067, .045, .032);
  tube(head, [[0, -.17, .452], [0, -.235, .456], [-.075, -.27, .439]], .012, 'dark', 8);
  tube(head, [[0, -.235, .455], [.075, -.27, .439]], .012, 'dark', 6);
  const eyes = [-1, 1].map(side => {
    const eye = new T.Group(); eye.position.set(side * .235, .035, .362); head.add(eye);
    ellipsoid(eye, 'dark', 0, 0, 0, .093, .115, .055);
    ellipsoid(eye, 'white', -.024, .037, .048, .024, .028, .013);
    ellipsoid(eye, 'white', .025, -.035, .05, .009, .01, .007);
    return eye;
  });
  const ears = [-1, 1].map(side => {
    const ear = new T.Group(); ear.position.set(side * .36, .3, -.015); head.add(ear);
    const shape = new T.Shape(); shape.moveTo(-.19, 0); shape.lineTo(.19, 0); shape.lineTo(0, .44); shape.closePath();
    const outer = new T.Mesh(new T.ExtrudeGeometry(shape, { depth: .1, bevelEnabled: true, bevelThickness: .045, bevelSize: .045, bevelSegments: 2, steps: 1 }), materials.fur);
    ear.add(outer);
    const inset = new T.Mesh(new T.ShapeGeometry(shape), materials.pink);
    inset.position.set(0, .045, .147); inset.scale.set(.62, .66, 1); ear.add(inset);
    ear.rotation.z = -side * .16;
    return ear;
  });
  for (const [x, angle] of [[-.18, -.25], [0, 0], [.18, .25]]) {
    const stripe = ellipsoid(head, 'stripe', x, .29, .296, .04, .125, .018); stripe.rotation.z = angle;
  }
  for (const side of [-1, 1]) {
    for (const y of [-.1, -.2]) {
      tube(head, [[side * .35, y, .35], [side * .52, y + .01, .37], [side * .68, y + .035, .35]], .008, 'stripe', 6);
    }
  }
  const tail = new T.Group(); tail.position.set(-.28, .33, -.21); body.add(tail);
  tube(tail, [[0, 0, 0], [-.33, .1, -.05], [-.58, .47, -.07], [-.61, .83, -.04], [-.48, .97, 0]], .09, 'fur');
  tube(tail, [[-.61, .78, -.05], [-.59, .89, -.025], [-.48, .97, 0]], .092, 'cream', 8);
  const packet = new T.Mesh(new T.BoxGeometry(.25, .2, .17), materials.teal);
  packet.position.set(0, .53, .57); body.add(packet);
  const band = new T.Mesh(new T.BoxGeometry(.04, .21, .18), materials.gold); packet.add(band);
  packet.visible = false;

  const weights = { busy: 0, sad: 0, sleep: 0 };
  function pose(state, seconds, dt, gaze, now, motion) {
    const blend = motion ? 1 - Math.exp(-dt * 7) : 1;
    const targets = {
      busy: state.kind === 'busy' ? 1 : 0,
      sad: ['failure', 'critical', 'slow', 'unstable'].includes(state.kind) ? 1 : 0,
      sleep: ['paused', 'disabled', 'idle', 'stale'].includes(state.kind) ? 1 : 0,
    };
    for (const key of Object.keys(weights)) weights[key] += (targets[key] - weights[key]) * blend;
    const { busy, sad, sleep } = weights;
    const t = motion ? seconds : 0;
    const gait = Math.sin(t * 8);
    const recovery = motion && state.recovering ? Math.max(0, Math.min(1, (now - state.recoveryStart) / 1200)) : 1;
    const hop = recovery < 1 ? Math.sin(recovery * Math.PI) * .27 : 0;
    root.position.y = hop;
    root.rotation.y = -.1 + (motion ? gaze.x * .17 : 0);
    body.scale.y = 1 - sleep * .16 + (motion ? Math.sin(t * 2) * .012 : 0);
    body.position.y = busy * Math.abs(gait) * .035;
    head.rotation.z = sad * -.18 + (motion ? Math.sin(t * 1.2) * .035 : 0);
    head.rotation.x = sleep * .24 + sad * .1 + (motion ? gaze.y * .07 : 0);
    head.rotation.y = motion ? gaze.x * .18 : 0;
    head.position.y = 1.43 - sleep * .12;
    const blink = motion && t % 5.2 > 4.98 ? .1 : 1;
    for (const eye of eyes) eye.scale.y = Math.max(.08, blink * (1 - sleep * .9 - sad * .23));
    ears.forEach((ear, i) => { ear.rotation.z = (i === 0 ? 1 : -1) * (.16 + sad * .42 + sleep * .13); });
    legs.forEach((leg, i) => { leg.rotation.x = motion ? busy * gait * (i === 0 ? .38 : -.38) : 0; });
    tail.rotation.z = (motion ? Math.sin(t * (busy > .5 ? 4 : 1.7)) * .12 : 0) - sad * .3;
    packet.visible = busy > .5;
    packet.rotation.z = motion ? Math.sin(t * 4) * .07 : 0;
  }
  return { root, pose };
}
