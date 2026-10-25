// scratch files of the shader-layout scan live under build/pkg_tmp/shader/
const path = require('path');
const fs = require('fs');
const ROOT = path.resolve(__dirname, '..', '..');
const DIR = path.join(ROOT, 'build', 'pkg_tmp', 'shader');
fs.mkdirSync(DIR, { recursive: true });
module.exports = { W: (name) => path.join(DIR, name), ROOT_SLASH: ROOT.replace(/\\/g, '/') + '/' };
