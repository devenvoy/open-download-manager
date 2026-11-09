import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

const pkg = JSON.parse(fs.readFileSync(path.join(rootDir, 'package.json'), 'utf8'));
const version = pkg.version || '0.2.0';
const baseName = pkg.name || 'open-download-manager';

const distReleaseDir = path.join(rootDir, 'dist', 'release');
fs.mkdirSync(distReleaseDir, { recursive: true });

// Identify local architecture
const arch = process.arch === 'arm64' ? 'arm64' : 'x64';
const platform = process.platform === 'darwin' ? 'mac' : (process.platform === 'win32' ? 'win' : 'lin');

console.log(`Packaging local release assets for: ${baseName} v${version} (${platform}_${arch})...\n`);

// 1. Extensions
const extDist = path.join(rootDir, 'dist', 'extension');
const chromeZip = path.join(extDist, `${baseName}_${version}_ext_chrome.zip`);
const firefoxXpi = path.join(extDist, `${baseName}_${version}_ext_firefox.xpi`);
const firefoxZip = path.join(extDist, `${baseName}_${version}_ext_firefox.zip`);

if (fs.existsSync(chromeZip)) {
  fs.copyFileSync(chromeZip, path.join(distReleaseDir, `${baseName}_${version}_ext_chrome.zip`));
}
if (fs.existsSync(firefoxXpi)) {
  fs.copyFileSync(firefoxXpi, path.join(distReleaseDir, `${baseName}_${version}_ext_firefox.xpi`));
}
if (fs.existsSync(firefoxZip)) {
  fs.copyFileSync(firefoxZip, path.join(distReleaseDir, `${baseName}_${version}_ext_firefox.zip`));
}

// 2. Tauri macOS bundles (check arm64, x64, and default target folders)
const targetSearchList = [
  { dir: path.join(rootDir, 'target', 'release', 'bundle'), arch: process.arch === 'arm64' ? 'arm64' : 'x64' },
  { dir: path.join(rootDir, 'target', 'aarch64-apple-darwin', 'release', 'bundle'), arch: 'arm64' },
  { dir: path.join(rootDir, 'target', 'x86_64-apple-darwin', 'release', 'bundle'), arch: 'x64' },
  { dir: path.join(rootDir, 'src-tauri', 'target', 'release', 'bundle'), arch: process.arch === 'arm64' ? 'arm64' : 'x64' },
  { dir: path.join(rootDir, 'src-tauri', 'target', 'aarch64-apple-darwin', 'release', 'bundle'), arch: 'arm64' },
  { dir: path.join(rootDir, 'src-tauri', 'target', 'x86_64-apple-darwin', 'release', 'bundle'), arch: 'x64' },
];

for (const { dir, arch: itemArch } of targetSearchList) {
  if (!fs.existsSync(dir)) continue;

  const dmgDir = path.join(dir, 'dmg');
  const macosAppDir = path.join(dir, 'macos');

  if (fs.existsSync(dmgDir)) {
    const dmgs = fs.readdirSync(dmgDir).filter(f => f.endsWith('.dmg'));
    for (const dmg of dmgs) {
      const srcDmg = path.join(dmgDir, dmg);
      const destDmg = path.join(distReleaseDir, `${baseName}_${version}_mac_${itemArch}.dmg`);
      fs.copyFileSync(srcDmg, destDmg);
    }
  }

  if (fs.existsSync(macosAppDir)) {
    const apps = fs.readdirSync(macosAppDir).filter(f => f.endsWith('.app'));
    for (const app of apps) {
      const tarGz = path.join(distReleaseDir, `${baseName}_${version}_mac_${itemArch}.app.tar.gz`);
      try {
        execSync(`tar -czf "${tarGz}" -C "${macosAppDir}" "${app}"`);
      } catch (e) {
        console.warn(`Failed to tar app: ${e.message}`);
      }
    }
  }
}

// 3. Generate SHA-256 Checksums
const releaseFiles = fs.readdirSync(distReleaseDir).filter(f => !f.endsWith('_checksums.txt') && !f.endsWith('.DS_Store'));
const checksums = [];

for (const file of releaseFiles) {
  const filePath = path.join(distReleaseDir, file);
  const data = fs.readFileSync(filePath);
  const hash = crypto.createHash('sha256').update(data).digest('hex');
  const sizeMb = (data.length / (1024 * 1024)).toFixed(2);
  checksums.push({ file, hash, sizeMb, bytes: data.length });
}

const checksumFile = path.join(distReleaseDir, `${baseName}_${version}_checksums.txt`);
const checksumText = checksums.map(c => `${c.hash}  ${c.file}`).join('\n') + '\n';
fs.writeFileSync(checksumFile, checksumText);

console.log('Release Assets Successfully Assembled in: ' + distReleaseDir);
console.log('='.repeat(70));
for (const c of checksums) {
  console.log(`- ${c.file.padEnd(46)} (${c.sizeMb.padStart(6)} MB)`);
  console.log(`  SHA256: ${c.hash}`);
}
console.log(`- ${path.basename(checksumFile).padEnd(46)} (Checksums file)`);
console.log('='.repeat(70));
