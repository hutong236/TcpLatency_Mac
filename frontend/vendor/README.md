# Bundled dependency

Three.js r180 (npm version 0.180.0), https://github.com/mrdoob/three.js/tree/r180.
The unmodified `build/three.module.min.js` and `build/three.core.min.js` are
distributed under the MIT license in `three-LICENSE.txt`.

Git blob identifiers:
- three.module.min.js: 20d3c112761a519c7780a5ccc9f72a40937fa83b
- three.core.min.js: 70c40977daa0f6e1e312c6b58e9bfe49cb5a87a4

Both modules are local and lazily imported by the 3D pet only. No CDN or runtime
network connection is required. The articulated cat geometry and animation
code are original project assets (`../cat-model.mjs`), under the project license.
