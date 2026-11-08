// Created: 2026-11-07
// Reason: feat(extension): improve video format picker UI, desktop handoff focus, and media tools handling (fixes #5)
// Last updated: 2026-11-07
// Reason: feat(extension): improve video format picker UI, desktop handoff focus, and media tools handling (fixes #5)
// Description: Unit tests verifying bitrate formatting, resolution badges, and quality labels in the extension UI.

const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

const context = { globalThis: {} };
vm.runInNewContext(fs.readFileSync(path.join(__dirname, 'format-presentation.js'), 'utf8'), context);
const pres = context.globalThis.odmFormatPresentation;

test('getQualityBadge maps resolutions and framerates cleanly without emojis', () => {
  assert.deepEqual(pres.getQualityBadge({ quality: '2160p 60fps' }), { label: '4K60', tier: 'badge-4k' });
  assert.deepEqual(pres.getQualityBadge({ quality: '4K' }), { label: '4K', tier: 'badge-4k' });
  assert.deepEqual(pres.getQualityBadge({ quality: '1440p' }), { label: '1440p', tier: 'badge-2k' });
  assert.deepEqual(pres.getQualityBadge({ quality: '1080p 60fps' }), { label: '1080p60', tier: 'badge-fhd' });
  assert.deepEqual(pres.getQualityBadge({ quality: '1080p' }), { label: '1080p', tier: 'badge-fhd' });
  assert.deepEqual(pres.getQualityBadge({ quality: '720p' }), { label: '720p', tier: 'badge-hd' });
  assert.deepEqual(pres.getQualityBadge({ quality: '480p' }), { label: '480p', tier: 'badge-sd' });
  assert.deepEqual(pres.getQualityBadge({ quality: '360p' }), { label: '360p', tier: 'badge-sd' });
  assert.deepEqual(pres.getQualityBadge({ quality: 'Audio', audioState: 'Audio only' }), { label: 'AUDIO', tier: 'badge-audio' });
  assert.deepEqual(pres.getQualityBadge({ format: 'MP3' }), { label: 'AUDIO', tier: 'badge-audio' });
  assert.deepEqual(pres.getQualityBadge({}), { label: 'VIDEO', tier: 'badge-neutral' });
});

test('formatBitrate converts raw bps into readable metrics', () => {
  assert.equal(pres.formatBitrate(4500000), '4.5 Mbps');
  assert.equal(pres.formatBitrate(2000000), '2 Mbps');
  assert.equal(pres.formatBitrate(128000), '128 kbps');
  assert.equal(pres.formatBitrate(64000), '64 kbps');
  assert.equal(pres.formatBitrate(0), '');
  assert.equal(pres.formatBitrate(null), '');
});

test('getFormatLabel recognizes format names and extension URLs', () => {
  assert.equal(pres.getFormatLabel({ format: 'mp4' }), 'MP4');
  assert.equal(pres.getFormatLabel({ url: 'https://example.com/stream.m3u8?token=xyz' }), 'HLS');
  assert.equal(pres.getFormatLabel({ url: 'https://example.com/clip.webm' }), 'WEBM');
  assert.equal(pres.getFormatLabel({ url: 'https://example.com/track.mp3' }), 'MP3');
  assert.equal(pres.getFormatLabel({ url: 'https://example.com/plain' }), 'MEDIA');
});

test('partitionChoices groups mixed media tracks into distinct sections', () => {
  const mixed = [
    { quality: '1080p', audioState: 'Video + audio' },
    { quality: '720p', audioState: 'Video + audio' },
    { quality: 'Audio', audioState: 'Audio only', format: 'MP3' },
    { quality: '1080p', audioState: 'Video only (silent)' }
  ];
  const groups = pres.partitionChoices(mixed);
  assert.equal(groups.length, 3);
  assert.equal(groups[0].title, 'Video + Audio');
  assert.equal(groups[0].items.length, 2);
  assert.equal(groups[1].title, 'Audio Only');
  assert.equal(groups[1].items.length, 1);
  assert.equal(groups[2].title, 'Video Only (Silent)');
  assert.equal(groups[2].items.length, 1);
});

test('partitionChoices does not add redundant section title for single category', () => {
  const uniform = [
    { quality: '1080p', audioState: 'Video + audio' },
    { quality: '720p', audioState: 'Video + audio' }
  ];
  const groups = pres.partitionChoices(uniform);
  assert.equal(groups.length, 1);
  assert.equal(groups[0].title, '');
  assert.equal(groups[0].items.length, 2);
});

test('createRowElement constructs accessible button with metadata and handles clicks', () => {
  function makeMockElement(tag) {
    return {
      tagName: tag,
      className: '',
      type: '',
      disabled: false,
      title: '',
      textContent: '',
      children: [],
      appendChild(child) {
        this.children.push(child);
        child.parentNode = this;
      }
    };
  }
  const mockDoc = { createElement: makeMockElement };

  let selectedItem = null;
  const item = {
    quality: '1080p',
    format: 'MP4',
    bitrate: 4500000,
    audioState: 'Video + audio',
    title: 'Example Video Title',
    reason: 'Verified stream'
  };

  const row = pres.createRowElement(mockDoc, item, {
    onSelect: sel => { selectedItem = sel; },
    getTooltip: reason => 'Safe: ' + reason
  });

  assert.equal(row.className, 'row');
  assert.equal(row.type, 'button');
  assert.equal(row.disabled, false);
  assert.equal(row.title, 'Safe: Verified stream');

  // Check row-left and row-right
  assert.equal(row.children.length, 2);
  const [rowLeft, rowRight] = row.children;
  assert.equal(rowLeft.className, 'row-left');
  assert.equal(rowRight.className, 'row-right');

  // Verify click handler
  row.onclick({ preventDefault() {}, stopPropagation() {} });
  assert.equal(selectedItem, item);
});
