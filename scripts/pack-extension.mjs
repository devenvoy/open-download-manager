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

// 3. Zip archives
const chromeZip = path.join(distDir, 'open-download-manager-chrome.zip');
const firefoxXpi = path.join(distDir, 'open-download-manager-firefox.xpi');

try {
  // Use zip if available
  execSync(`cd "${chromeDist}" && zip -r "${chromeZip}" . -x "*.DS_Store"`, { stdio: 'pipe' });
  execSync(`cd "${firefoxDist}" && zip -r "${firefoxXpi}" . -x "*.DS_Store"`, { stdio: 'pipe' });
  console.log('✓ Packages generated:');
  console.log(`  - Chrome Package:  ${chromeZip}`);
  console.log(`  - Firefox Add-on:  ${firefoxXpi}`);
} catch (e) {
  console.warn('Note: zip command failed or not found. Folders are available directly.');
}

// 4. Try packing .crx with Chrome if Chrome binary exists
const chromeExecutables = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  'google-chrome',
  'chromium',
  'chrome'
];

for (const exe of chromeExecutables) {
  try {
    const test = execSync(`which "${exe}" 2>/dev/null || [ -f "${exe}" ] && echo "found"`, { stdio: 'pipe' }).toString().trim();
    if (test === 'found' || test.length > 0) {
      execSync(`"${exe}" --pack-extension="${chromeDist}"`, { stdio: 'pipe' });
      // Chrome writes chrome.crx next to chromeDist (in distDir)
      const rawCrx = path.join(distDir, 'chrome.crx');
      const targetCrx = path.join(distDir, 'open-download-manager.crx');
      if (fs.existsSync(rawCrx)) {
        fs.renameSync(rawCrx, targetCrx);
        console.log(`  - Chrome CRX:      ${targetCrx}`);
      }
      break;
    }
  } catch {}
}

console.log('\nExtension packs successfully created in: ' + distDir);
