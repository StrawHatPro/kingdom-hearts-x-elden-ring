// usage: node config_extract.js  -> sead_config.bin (the scnf blob from [SQEXSEAD] ConfigString in WindowsEngine.ini; 3 decimal digits per byte)
const fs=require('fs'),path=require('path');const R=path.resolve(__dirname,'../../../..');
const ini=fs.readFileSync(path.join(R,'assets/KINGDOM HEARTS III/Config/Windows/WindowsEngine.ini'),'latin1');
const m=/^ConfigString=(\d+)/m.exec(ini);const s=m[1];const b=Buffer.alloc(Math.floor(s.length/3));for(let i=0;i<b.length;i++)b[i]=+s.substr(i*3,3);
fs.writeFileSync(path.join(__dirname,'sead_config.bin'),b);console.log('bytes',b.length,'magic',b.toString('latin1',0,4));
