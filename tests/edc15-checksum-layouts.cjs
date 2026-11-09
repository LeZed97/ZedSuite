const fs=require('fs'),vm=require('vm'),assert=require('assert');
const root=require('path').resolve(__dirname,'..')+'/';
const ts=require('typescript');
const moduleCache=new Map();
function mod(path){
 path=path.replace(/\\/g,'/');if(moduleCache.has(path))return moduleCache.get(path);
 const m={exports:{}};moduleCache.set(path,m.exports);
 const localRequire=spec=>spec.startsWith('.')?mod(require('path').posix.normalize(require('path').posix.join(require('path').posix.dirname(path),spec))+'.ts'):require(spec);
 vm.runInNewContext(ts.transpileModule(fs.readFileSync(root+path,'utf8')+'\nexport { tdi41v2ChecksumSearch as fixtureCompact };',{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2020}}).outputText,{exports:m.exports,module:m,require:localRequire});return m.exports;
}
const checksum=mod('src/lib/ecu/bosch/checksums/edc15p-checksum.ts');
const signature=Buffer.from('67ffffffffffff56342e31','hex');
const unknown=checksum.verifyEDC15PChecksum(Array(0x80000).fill(0));assert.equal(unknown.result,'TYPE_ERROR');
assert.equal(checksum.verifyEDC15PChecksum([0,0]).result,'TYPE_ERROR');console.log('PASS unsupported buffers retain TYPE_ERROR');
const data=Buffer.alloc(0x80000,0xc3);signature.copy(data,0x70001);
for(const p of [0x13ffc,0x70b7c,0x7bffc])data.writeUInt32LE(0,p);
const fixed=checksum.correctEDC15PChecksum(Array.from(data));assert.equal(fixed.info.variant,'tdi41');
let v=checksum.verifyEDC15PChecksum(fixed.correctedData);assert.equal(v.result,'OK');assert.equal(v.found,3);assert.equal(v.matched,3);
fixed.correctedData[0x78000]=42;const next=checksum.correctEDC15PChecksum(fixed.correctedData);assert.equal(next.info.variant,'tdi41');
const allowed=new Set([0x13ffc,0x70b7c,0x7bffc].flatMap(p=>[p,p+1,p+2,p+3]));next.correctedData.forEach((value,i)=>{if(value!==fixed.correctedData[i])assert(allowed.has(i));});
v=checksum.verifyEDC15PChecksum(next.correctedData);assert.equal(v.result,'OK');assert.equal(v.matched,3);console.log('PASS legacy single-bank selection and isolated checksum correction');
const compact=Buffer.alloc(0x80000,0xc3);signature.copy(compact,0x58001);signature.copy(compact,0x70001);
for(const p of [0x10000,0x14000,0x58000,0x58b80,0x70000,0x70b80])compact[p]=0;
for(const p of [0x13ffc,0x57ffc,0x58b7c,0x63ffc,0x70b7c,0x7bffc])compact.writeUInt32LE(0,p);
checksum.fixtureCompact(compact,compact.length);
const cf=checksum.correctEDC15PChecksum(Array.from(compact));assert.equal(cf.info.variant,'tdi41v2');v=checksum.verifyEDC15PChecksum(cf.correctedData);assert.equal(v.result,'OK');assert.equal(v.found,6);assert.equal(v.matched,6);console.log('PASS empty compact region excluded from counts');
