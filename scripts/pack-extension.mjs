import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');
const extDir = path.join(rootDir, 'extension');
const distDir = path.join(rootDir, 'dist', 'extension');
const chromeDist = path.join(distDir, 'chrome');
const firefoxDist = path.join(distDir, 'firefox');

// 1. Clean dist folder
fs.rmSync(distDir, { recursive: true, force: true });
fs.mkdirSync(chromeDist, { recursive: true });
fs.mkdirSync(firefoxDist, { recursive: true });

// 2. Identify files to copy
const files = fs.readdirSync(extDir);

for (const file of files) {
  if (file.endsWith('.test.cjs')) continue;

  const srcPath = path.join(extDir, file);
  const stat = fs.statSync(srcPath);

  if (stat.isDirectory()) {
    // Copy icons directory
    fs.cpSync(srcPath, path.join(chromeDist, file), { recursive: true });
    fs.cpSync(srcPath, path.join(firefoxDist, file), { recursive: true });
  } else {
    // Chrome build: include manifest.json, exclude manifest.firefox.json
    if (file !== 'manifest.firefox.json') {
      fs.copyFileSync(srcPath, path.join(chromeDist, file));
    }
    // Firefox build: exclude manifest.json, use manifest.firefox.json as manifest.json
    if (file === 'manifest.firefox.json') {
      fs.copyFileSync(srcPath, path.join(firefoxDist, 'manifest.json'));
    } else if (file !== 'manifest.json') {
      fs.copyFileSync(srcPath, path.join(firefoxDist, file));
    }
  }
}

console.log('✓ Cleaned extension folders prepared:');
console.log(`  - Chrome/Edge/Brave/Opera: ${chromeDist}`);
console.log(`  - Firefox: ${firefoxDist}`);

// 3. Zip archives adhering to appName_version_platform_variant format
const pkg = JSON.parse(fs.readFileSync(path.join(rootDir, 'package.json'), 'utf8'));
const version = pkg.version || '0.2.0';
const baseName = pkg.name || 'open-download-manager';

const chromeZipFormatted = path.join(distDir, `${baseName}_${version}_ext_chrome.zip`);
const firefoxXpiFormatted = path.join(distDir, `${baseName}_${version}_ext_firefox.xpi`);
const firefoxZipFormatted = path.join(distDir, `${baseName}_${version}_ext_firefox.zip`);

const chromeZipLegacy = path.join(distDir, 'open-download-manager-chrome.zip');
const firefoxXpiLegacy = path.join(distDir, 'open-download-manager-firefox.xpi');

try {
  // Use zip to package extensions cleanly without launching desktop browsers
  execSync(`cd "${chromeDist}" && zip -r "${chromeZipFormatted}" . -x "*.DS_Store"`, { stdio: 'pipe' });
  execSync(`cd "${firefoxDist}" && zip -r "${firefoxXpiFormatted}" . -x "*.DS_Store"`, { stdio: 'pipe' });
  
  // Copy to formatted firefox zip and legacy filenames
  fs.copyFileSync(firefoxXpiFormatted, firefoxZipFormatted);
  fs.copyFileSync(chromeZipFormatted, chromeZipLegacy);
  fs.copyFileSync(firefoxXpiFormatted, firefoxXpiLegacy);

  console.log('✓ Release packages generated:');
  console.log(`  - Chrome/Edge/Brave: ${chromeZipFormatted}`);
  console.log(`  - Firefox XPI:       ${firefoxXpiFormatted}`);
  console.log(`  - Firefox ZIP:       ${firefoxZipFormatted}`);
} catch (e) {
  console.warn('Note: zip command failed or not found. Folders are available directly.', e.message);
}

console.log('\nExtension packs successfully created in: ' + distDir);
