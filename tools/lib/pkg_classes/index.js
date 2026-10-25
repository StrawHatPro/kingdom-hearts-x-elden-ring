// Native serializers per class family. Each module registers codecs with ../package.js.
const fs = require('fs');
const path = require('path');
for (const f of fs.readdirSync(__dirname).sort()) if (f !== 'index.js' && f.endsWith('.js')) require(path.join(__dirname, f));
