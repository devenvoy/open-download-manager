// Created: 2026-11-07
// Reason: feat(extension): improve video format picker UI, desktop handoff focus, and media tools handling (fixes #5)
// Last updated: 2026-11-07
// Reason: feat(extension): improve video format picker UI, desktop handoff focus, and media tools handling (fixes #5)
// Description: Format presentation utilities formatting bitrates, resolution badges, and quality labels for UI.

(function (root) {
  'use strict';

  function getQualityBadge(item) {
    const rawQuality = String(item.quality || '').trim();
    const format = String(item.format || '').toUpperCase();
    const audioState = String(item.audioState || '');
    const isAudio = audioState === 'Audio only' || format === 'MP3' || format === 'M4A' || rawQuality.toLowerCase() === 'audio';

    if (isAudio) {
      return { label: 'AUDIO', tier: 'badge-audio' };
    }

    const q = rawQuality.toLowerCase();
    const is60 = /60(?:fps)?/i.test(rawQuality);
    const suffix = is60 ? '60' : '';

    if (q.includes('2160') || q.includes('4k') || q.includes('3840')) {
      return { label: '4K' + suffix, tier: 'badge-4k' };
    }
    if (q.includes('1440') || q.includes('2k')) {
      return { label: '1440p' + suffix, tier: 'badge-2k' };
    }
    if (q.includes('1080')) {
      return { label: '1080p' + suffix, tier: 'badge-fhd' };
    }
    if (q.includes('720')) {
      return { label: '720p' + suffix, tier: 'badge-hd' };
    }
    if (q.includes('480')) {
      return { label: '480p', tier: 'badge-sd' };
    }
    if (q.includes('360')) {
      return { label: '360p', tier: 'badge-sd' };
    }
    if (q.includes('240')) {
      return { label: '240p', tier: 'badge-sd' };
    }
    if (q.includes('144')) {
      return { label: '144p', tier: 'badge-sd' };
    }

    if (rawQuality) {
      return { label: rawQuality.slice(0, 10), tier: 'badge-neutral' };
    }

    return { label: format || 'VIDEO', tier: 'badge-neutral' };
  }

  function formatBitrate(bitrate) {
    const n = Number(bitrate);
    if (!Number.isFinite(n) || n <= 0) return '';
    if (n >= 1000000) {
      return (n / 1000000).toFixed(1).replace(/\.0$/, '') + ' Mbps';
    }
    return Math.round(n / 1000) + ' kbps';
  }

  function getFormatLabel(item) {
    if (item.format) return String(item.format).toUpperCase();
    if (item.url) {
      if (/\.m3u8(?:[?#]|$)/i.test(item.url)) return 'HLS';
      const match = /\.(mp4|webm|mp3|m4a|mov|ogg|mkv)(?:[?#]|$)/i.exec(item.url);
      if (match) return match[1].toUpperCase();
    }
    return 'MEDIA';
  }

  function getAudioIndicator(item) {
    const state = item.audioState;
    if (state === 'Video + audio') return 'Video + Audio';
    if (state === 'Audio only') return 'Audio Only';
    if (state === 'Video only (silent)') return 'Silent';
    if (state === 'Audio not verified') return 'Unverified';
    return state || '';
  }

  function partitionChoices(choices) {
    const videoAudio = [];
    const audioOnly = [];
    const videoOnly = [];

    for (const item of choices) {
      if (item.audioState === 'Audio only' || item.format === 'MP3' || item.format === 'M4A') {
        audioOnly.push(item);
      } else if (item.audioState === 'Video only (silent)') {
        videoOnly.push(item);
      } else {
        videoAudio.push(item);
      }
    }

    const populated = [videoAudio, audioOnly, videoOnly].filter(function (list) {
      return list.length > 0;
    });
    const multiGroup = populated.length > 1;

    const groups = [];
    if (videoAudio.length) {
      groups.push({ title: multiGroup ? 'Video + Audio' : '', items: videoAudio });
    }
    if (audioOnly.length) {
      groups.push({ title: multiGroup ? 'Audio Only' : '', items: audioOnly });
    }
    if (videoOnly.length) {
      groups.push({ title: multiGroup ? 'Video Only (Silent)' : '', items: videoOnly });
    }

    return groups;
  }

  function createRowElement(doc, item, options) {
    const opts = options || {};
    const row = doc.createElement('button');
    row.className = 'row';
    row.type = 'button';
    row.disabled = !!item.unsupported;

    if (typeof opts.getTooltip === 'function') {
      row.title = opts.getTooltip(item.reason);
    }

    const badge = getQualityBadge(item);
    const format = getFormatLabel(item);
    const audioText = getAudioIndicator(item);
    const bitrateText = formatBitrate(item.bitrate);
    const title = opts.fallbackTitle || (item.asset ? 'Video ' + item.asset : item.title || 'Video');

    // Left column: metadata & title
    const rowLeft = doc.createElement('div');
    rowLeft.className = 'row-left';

    const rowTop = doc.createElement('div');
    rowTop.className = 'row-top';

    const badgeEl = doc.createElement('span');
    badgeEl.className = 'badge-quality ' + badge.tier;
    badgeEl.textContent = badge.label;
    rowTop.appendChild(badgeEl);

    const formatEl = doc.createElement('span');
    formatEl.className = 'tag-format';
    formatEl.textContent = format;
    rowTop.appendChild(formatEl);

    if (audioText) {
      const audioEl = doc.createElement('span');
      audioEl.className = 'meta-sub';
      audioEl.textContent = audioText;
      rowTop.appendChild(audioEl);
    }

    if (bitrateText) {
      const bitrateEl = doc.createElement('span');
      bitrateEl.className = 'meta-sub';
      bitrateEl.textContent = '· ' + bitrateText;
      rowTop.appendChild(bitrateEl);
    }

    rowLeft.appendChild(rowTop);

    const titleEl = doc.createElement('div');
    titleEl.className = 'row-title';
    titleEl.textContent = title;
    rowLeft.appendChild(titleEl);

    row.appendChild(rowLeft);

    // Right column: action indicator
    const rowRight = doc.createElement('div');
    rowRight.className = 'row-right';

    const actionEl = doc.createElement('span');
    actionEl.className = 'action-btn';
    actionEl.textContent = '↓';
    rowRight.appendChild(actionEl);

    row.appendChild(rowRight);

    if (typeof opts.onSelect === 'function') {
      row.onclick = function (event) {
        if (event && event.preventDefault) event.preventDefault();
        if (event && event.stopPropagation) event.stopPropagation();
        opts.onSelect(item, row);
      };
    }

    return row;
  }

  function populateChooser(chooser, choices, doc, options) {
    const opts = options || {};
    const groups = partitionChoices(choices);

    for (let g = 0; g < groups.length; g++) {
      const group = groups[g];
      if (group.title) {
        const header = doc.createElement('div');
        header.className = 'section-label';
        header.textContent = group.title;
        chooser.appendChild(header);
      }

      for (let i = 0; i < group.items.length; i++) {
        const item = group.items[i];
        const row = createRowElement(doc, item, {
          onSelect: opts.onSelect,
          getTooltip: opts.getTooltip,
          fallbackTitle: opts.fallbackTitle
        });
        chooser.appendChild(row);
      }
    }
  }

  root.odmFormatPresentation = {
    getQualityBadge: getQualityBadge,
    formatBitrate: formatBitrate,
    getFormatLabel: getFormatLabel,
    getAudioIndicator: getAudioIndicator,
    partitionChoices: partitionChoices,
    createRowElement: createRowElement,
    populateChooser: populateChooser
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = root.odmFormatPresentation;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
