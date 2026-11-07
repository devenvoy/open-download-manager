(() => {
  const api = globalThis.browser || chrome;
  const seen = new WeakSet();
  const records = new Set();
  let pending = false;
  let stopped = false, scanFrame, scrollTimer, scrolling=false;
  const mutationObservers = new Set();
  const observedRoots = new WeakSet();
  const listeners = new AbortController();
  const resizeObserver = new ResizeObserver(() => position());
  const direct = value => { try { const u = new URL(value, location.href); return /^https?:$/.test(u.protocol) ? u.href : null; } catch { return null; } };
  const PANEL_CSS = ':host{font:12px -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;color:#0f172a}.bar{display:flex;justify-content:flex-end;align-items:stretch}.download,.dismiss{font:12px -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;background:#f8fafc;color:#0f172a;border:1px solid #cbd5e1;cursor:pointer;box-shadow:0 1px 3px rgba(0,0,0,0.1);height:27px;box-sizing:border-box}.download{border-radius:4px 0 0 4px;padding:3px 10px;font-weight:500}.dismiss{width:24px;padding:0;border-left:0;border-radius:0 4px 4px 0;font-size:15px;line-height:22px;color:#64748b}.download:hover,.dismiss:hover{background:#f1f5f9}.dismiss:hover{color:#b91c1c}.download:disabled{opacity:.6;cursor:not-allowed}.chooser{position:fixed;background:#ffffff;color:#0f172a;border:1px solid #cbd5e1;border-radius:6px;width:min(420px,calc(100vw - 12px));box-shadow:0 4px 16px rgba(0,0,0,0.15),0 1px 3px rgba(0,0,0,0.08);max-height:min(360px,60vh);overflow-y:auto;overflow-x:hidden;box-sizing:border-box;font-family:inherit;z-index:2147483647}.chooser p,.hint{margin:0;padding:8px 12px;font-size:11px;font-weight:500;color:#475569;border-bottom:1px solid #f1f5f9;line-height:1.4}.section-label{padding:6px 12px 4px;font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:0.05em;color:#64748b;background:#f8fafc;border-bottom:1px solid #f1f5f9}.row{display:flex;align-items:center;justify-content:space-between;width:100%;text-align:left;background:transparent;border:0;border-bottom:1px solid #f1f5f9;padding:7px 12px;cursor:pointer;box-sizing:border-box;font-family:inherit;color:inherit;transition:background .08s ease}.row:hover:not(:disabled),.row:focus-visible{background:#f1f5f9;outline:1px solid #0284c7;outline-offset:-1px}.row:disabled{opacity:.45;cursor:not-allowed}.row-left{display:flex;flex-direction:column;gap:3px;min-width:0;flex:1}.row-top{display:flex;align-items:center;flex-wrap:wrap;gap:6px}.badge-quality{display:inline-block;padding:1px 5px;font-size:10px;font-weight:700;border-radius:3px;line-height:1.3;text-transform:uppercase;background:#f1f5f9;color:#334155;border:1px solid #cbd5e1}.badge-4k,.badge-2k,.badge-fhd{background:#e0f2fe;color:#0369a1;border-color:#7dd3fc}.badge-hd{background:#f0fdf4;color:#15803d;border-color:#86efac}.badge-audio{background:#fef3c7;color:#92400e;border-color:#fde68a}.tag-format{font-size:11px;font-weight:600;color:#0f172a}.meta-sub{font-size:11px;color:#64748b}.row-title{font-size:11px;color:#64748b;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:340px}.row-right{display:flex;align-items:center;margin-left:8px;flex-shrink:0}.action-btn{font-size:13px;color:#64748b;font-weight:600}.row:hover:not(:disabled) .action-btn,.row:focus-visible .action-btn{color:#0284c7}@media(prefers-color-scheme:dark){:host{color:#f4f4f5}.download,.dismiss{background:#27272a;color:#f4f4f5;border-color:#3f3f46;box-shadow:0 1px 3px rgba(0,0,0,0.3)}.download:hover,.dismiss:hover{background:#3f3f46}.dismiss{color:#a1a1aa}.dismiss:hover{color:#f87171}.chooser{background:#18181b;color:#f4f4f5;border-color:#27272a;box-shadow:0 8px 24px rgba(0,0,0,0.45)}.chooser p,.hint{color:#a1a1aa;border-bottom-color:#27272a}.section-label{color:#71717a;background:#202023;border-bottom-color:#27272a}.row{border-bottom-color:#27272a;color:#f4f4f5}.row:hover:not(:disabled),.row:focus-visible{background:#27272a;outline-color:#38bdf8}.row-title{color:#a1a1aa}.badge-quality{background:#27272a;color:#e4e4e7;border-color:#3f3f46}.badge-fhd,.badge-4k,.badge-2k{background:#082f49;color:#7dd3fc;border-color:#0369a1}.badge-hd{background:#052e16;color:#86efac;border-color:#166534}.badge-audio{background:#451a03;color:#fcd34d;border-color:#92400e}.tag-format{color:#f4f4f5}.meta-sub{color:#71717a}.action-btn{color:#71717a}.row:hover:not(:disabled) .action-btn,.row:focus-visible .action-btn{color:#38bdf8}}';
  function shutdown() {
    if(stopped)return;
    stopped=true;
    listeners.abort();
    clearTimeout(scrollTimer);
    for (const observer of mutationObservers) observer.disconnect();
    mutationObservers.clear();
    resizeObserver.disconnect();
    if(scanFrame!==undefined)cancelAnimationFrame(scanFrame);
    for(const record of records){
      record.sourceObserver?.disconnect();
      record.intersectionObserver?.disconnect();
      record.host.remove();
      if(record.menuHost)record.menuHost.remove();
    }
    records.clear();
    try{api.runtime.onMessage.removeListener(onMessage);}catch{}
  }
  function connected() {
    if(stopped)return false;
    try{if(api.runtime.id)return true;}catch{}
    shutdown();return false;
  }
  // sendMessage can throw synchronously after an extension reload, before a
  // caller's .catch exists. Always expose a promise and retire the stale script.
  async function send(message) {
    if(!connected())throw new Error('The extension was reloaded. Refresh this page to reconnect.');
    try{return await api.runtime.sendMessage(message);}
    catch(error){if(/extension context invalidated/i.test(error?.message||''))shutdown();throw error;}
  }
  function extractVideoId(node) {
    try {
      const el = node;
      if (el) {
        const link = el.querySelector?.('a#thumbnail, a[href*="/watch?v="], a[href*="/shorts/"]')
          || (typeof el.closest === 'function' ? el.closest('a[href*="/watch?v="], a[href*="/shorts/"]') : null);
        if (link?.href) {
          const u = new URL(link.href, location.href);
          const id = u.searchParams.get('v') || /^\/shorts\/([\w-]{11})/.exec(u.pathname)?.[1];
          if (id && /^[\w-]{11}$/.test(id)) return id;
        }
      }
      const pageUrl = new URL(location.href);
      const vParam = pageUrl.searchParams.get('v');
      if (vParam && /^[\w-]{11}$/.test(vParam)) return vParam;
      const shortsMatch = /^\/shorts\/([\w-]{11})/.exec(pageUrl.pathname);
      if (shortsMatch) return shortsMatch[1];
      const container = typeof node?.closest === 'function' ? node.closest('#video-preview, ytd-video-preview, ytd-rich-item-renderer, yt-lockup-view-model, ytd-thumbnail, ytd-video-renderer, ytd-compact-video-renderer') : null;
      if (container && typeof container.querySelector === 'function') {
        const link = container.querySelector('a[href*="/watch?v="], a[href*="/shorts/"]');
        if (link && link.href) {
          const u = new URL(link.href, location.href);
          const id = u.searchParams.get('v') || /^\/shorts\/([\w-]{11})/.exec(u.pathname)?.[1];
          if (id && /^[\w-]{11}$/.test(id)) return id;
        }
      }
    } catch {}
    return null;
  }
  function extractVideoTitle(node) {
    try {
      if (typeof document !== 'undefined') {
        const watchTitle = document.querySelector('ytd-watch-metadata #title h1 yt-formatted-string, #above-the-fold #title h1, h1.ytd-watch-metadata, h1.title');
        const watchText = watchTitle?.textContent?.trim();
        if (watchText && watchText.length > 1) return watchText;
      }

      const container = typeof node?.closest === 'function' ? node.closest('#video-preview, ytd-video-preview, ytd-rich-item-renderer, yt-lockup-view-model, ytd-thumbnail, ytd-video-renderer, ytd-compact-video-renderer') : null;
      const root = container || node;
      if (root && typeof root.querySelector === 'function') {
        const titleEl = root.querySelector('#video-title, #title, h3, [title]');
        const text = titleEl?.getAttribute?.('title') || titleEl?.textContent?.trim?.();
        if (text && text.length > 1) return text;
      }
      if (typeof document !== 'undefined' && document.title) {
        const cleanedDocTitle = document.title.replace(/^\(\d+\)\s*/, '').replace(/\s*-\s*YouTube$/i, '').trim();
        if (cleanedDocTitle && cleanedDocTitle.length > 1 && cleanedDocTitle.toLowerCase() !== 'youtube') {
          return cleanedDocTitle;
        }
      }
    } catch {}
    return (typeof document !== 'undefined' && document.title) || 'Video';
  }

  function isFeedPreview(video, card) {
    if (card) return true;
    try {
      if (typeof location !== 'undefined' && location.hostname && location.hostname.includes('youtube.com')) {
        const isMainWatchPlayer = location.pathname === '/watch' && typeof video?.closest === 'function' && video.closest('#movie_player') && !video.closest('#inline-preview-player, [id*="preview"]');
        if (isMainWatchPlayer) return false;
        return true;
      }
      if (video && typeof video.closest === 'function' && video.closest('#inline-preview-player, ytd-inline-preview-renderer, ytd-thumbnail, ytd-video-preview, yt-lockup-view-model, ytd-rich-item-renderer, [id*="preview"], [class*="preview"], [data-preview]')) return true;
      if (video && video.parentElement && typeof video.parentElement.closest === 'function' && video.parentElement.closest('a[href], yt-lockup-view-model, ytd-rich-item-renderer, ytd-video-renderer')) return true;
    } catch {}
    return false;
  }

  let activeChooserRecord = null;

  function getContainer(record) {
    if (record.container && record.container.isConnected) return record.container;
    const base = record.target || record.video;
    const c = typeof base?.closest === 'function' ? base.closest('#video-preview, ytd-video-preview, ytd-rich-item-renderer, yt-lockup-view-model, ytd-thumbnail, ytd-video-renderer, ytd-compact-video-renderer') : null;
    if (c) record.container = c;
    return c;
  }

  function scheduleLinger(record, ms = 10000) {
    clearTimeout(record.lingerTimer);
    record.lingerTimer = setTimeout(() => {
      record.lingerTimer = null;
      if (!record.hovered && activeChooserRecord !== record) {
        if (record.isFeed) {
          record.host.style.display = 'none';
          record.formatsReady = false;
        }
      }
      position();
    }, ms);
  }

  function prefetchFormats(record) {
    if (record.formatsReady || record.isPrefetching) return;
    const videoId = record.videoId || extractVideoId(record.target || record.video);
    if (!videoId) return;
    record.isPrefetching = true;
    send({action:'listForFrame', videoId}).then(result => {
      record.isPrefetching = false;
      if (result?.ok && Array.isArray(result.items) && result.items.length) {
        const candidates = result.items;
        const youtube = /(^|\.)youtube\.com$/.test(location.hostname);
        const url = direct(record.video?.currentSrc || record.video?.src);
        if (!youtube && url && !candidates.some(item => item.url === url)) {
          candidates.unshift({url, title: record.videoTitle || document.title});
        }
        const choices = globalThis.mediaChoices(candidates, {url, poster: record.video?.poster});
        if (choices.length) {
          record.prefetchedChoices = choices;
          record.formatsReady = true;
          if (record.hovered) {
            position();
          }
        }
      } else if (record.hovered && !record.formatsReady) {
        setTimeout(() => {
          if (record.hovered && !record.formatsReady) prefetchFormats(record);
        }, 600);
      }
    }).catch(() => {
      record.isPrefetching = false;
      if (record.hovered && !record.formatsReady) {
        setTimeout(() => {
          if (record.hovered && !record.formatsReady) prefetchFormats(record);
        }, 600);
      }
    });
  }

  function onEnter(record) {
    if (activeChooserRecord && activeChooserRecord !== record) return;
    record.hovered = true;
    clearTimeout(record.lingerTimer);
    record.lingerTimer = null;
    if (!record.videoId) {
      record.videoId = extractVideoId(record.target || record.video);
      record.videoTitle = extractVideoTitle(record.target || record.video);
    }
    prefetchFormats(record);
    if (!record.isFeed || record.formatsReady) {
      position();
    }
  }

  function onLeave(record) {
    record.hovered = false;
    if (activeChooserRecord === record) return;
    if (record.formatsReady) {
      scheduleLinger(record, 10000);
    } else {
      if (record.isFeed) {
        record.host.style.display = 'none';
      }
    }
  }

  function attachRecord({ video, card, target, videoId, videoTitle, canonicalUrl }) {
    const liveTarget = target || video || card;
    const host = document.createElement('div');
    host.style.cssText = 'position:fixed;z-index:2147483647;display:none;pointer-events:auto';
    const shadow = host.attachShadow({mode:'closed'});
    shadow.innerHTML = `<style>${PANEL_CSS}</style><div class="bar"><button class="download" type="button">↓ Download this video ▾</button><button class="dismiss" type="button" title="Hide this download panel" aria-label="Hide this download panel">×</button></div>`;
    const button = shadow.querySelector('.download');
    const dismiss = shadow.querySelector('.dismiss');
    const isFeed = isFeedPreview(video, card);
    const resolvedVideoId = videoId || extractVideoId(liveTarget);
    const resolvedVideoTitle = videoTitle || extractVideoTitle(liveTarget);
    const resolvedCanonical = canonicalUrl || (resolvedVideoId ? `https://www.youtube.com/watch?v=${resolvedVideoId}` : null);

    const record = {
      video: video || null,
      card: card || null,
      target: liveTarget,
      host,
      shadow,
      dismissed: false,
      chooser: null,
      menuHost: null,
      videoId: resolvedVideoId,
      videoTitle: resolvedVideoTitle,
      canonicalUrl: resolvedCanonical,
      prefetchedChoices: null,
      isPrefetching: false,
      lingerTimer: null,
      hovered: false,
      frozenChooserPos: null,
      formatsReady: false,
      savedPlace: null,
      container: card || (video ? getContainer({ video }) : null),
      isFeed
    };
    records.add(record);
    resizeObserver.observe(liveTarget);
    record.intersectionObserver = new IntersectionObserver(() => position(), {threshold: [0, 0.25, 0.5, 0.75, 1]});
    record.intersectionObserver.observe(liveTarget);
    document.documentElement.appendChild(host);

    if (typeof host.addEventListener === 'function') {
      host.addEventListener('mouseenter', () => onEnter(record));
      host.addEventListener('mouseleave', () => onLeave(record));
    }

    if (video && typeof video.addEventListener === 'function') {
      video.addEventListener('mouseenter', () => onEnter(record), {signal: listeners.signal});
      video.addEventListener('mouseleave', () => onLeave(record), {signal: listeners.signal});
    }

    if (card && typeof card.addEventListener === 'function') {
      card.addEventListener('mouseenter', () => onEnter(record), {signal: listeners.signal});
      card.addEventListener('mouseleave', () => onLeave(record), {signal: listeners.signal});
    }

    const container = record.container;
    if (container && container !== video && container !== card && typeof container.addEventListener === 'function') {
      container.addEventListener('mouseenter', () => onEnter(record), {signal: listeners.signal});
      container.addEventListener('mouseleave', () => onLeave(record), {signal: listeners.signal});
    }

    if (video) {
      const report = () => {
        const sourceTypes = new Map(Array.from(video.querySelectorAll?.('source') || [], source => [direct(source.src), source.type]));
        const urls = [video.currentSrc, video.src, ...Array.from(video.querySelectorAll?.('source') || [], source => source.src)].map(direct).filter(Boolean);
        if (urls.length) send({action:'remember', items: urls.map(url => ({
          url,
          title:document.title,
          media:true,
          mime:sourceTypes.get(url),
          quality:video.videoHeight ? `${video.videoHeight}p` : ''
        }))}).catch(() => {});
      };
      video.addEventListener?.('loadedmetadata', report, {signal:listeners.signal});
      record.sourceObserver=new MutationObserver(report);
      record.sourceObserver.observe(video,{attributes:true,attributeFilter:['src'],subtree:true,childList:true});
      report();
    }

    dismiss.addEventListener('click', event => {
      event.preventDefault(); event.stopPropagation();
      record.dismissed = true;
      if (record.chooser) { record.chooser.remove(); record.chooser = null; }
      if (record.menuHost) { record.menuHost.remove(); record.menuHost = null; }
      if (activeChooserRecord === record) activeChooserRecord = null;
      shadow.querySelector('.chooser')?.remove();
      host.style.display = 'none';
    });

    button.addEventListener('click', async event => {
      event.preventDefault(); event.stopPropagation();
      if (record.chooser || activeChooserRecord === record) {
        closeChoosers();
        return;
      }
      closeChoosers();
      clearTimeout(record.lingerTimer);
      record.lingerTimer = null;
      record.hovered = true;
      const url = direct(video?.currentSrc || video?.src || video?.querySelector?.('source')?.src);

      const initialRect = button.getBoundingClientRect();
      record.frozenChooserPos = {
        top: initialRect.top,
        bottom: initialRect.bottom,
        right: initialRect.right,
        left: initialRect.left,
        width: initialRect.width || 120,
        height: initialRect.height || 27,
      };
      const chooser = document.createElement('div');
      chooser.className = 'chooser';
      record.chooser = chooser;
      activeChooserRecord = record;

      shadow.append(chooser);

      chooser.addEventListener('keydown', event => {
        globalThis.odmChooserKeys.handleKey(event, chooser, shadow.activeElement, {
          close: () => {
            closeChoosers();
            button.focus();
          }
        });
      });

      const chooserWidth = Math.min(420, innerWidth - 12);
      chooser.style.position = 'fixed';
      chooser.style.width = `${chooserWidth}px`;
      const left = Math.max(6, Math.min(innerWidth - chooserWidth - 6, record.frozenChooserPos.right - chooserWidth));
      chooser.style.left = `${left}px`;
      const below = record.frozenChooserPos.bottom + 4;
      chooser.style.top = `${below}px`;

      const hint = document.createElement('p');
      const renderChoices = (choices, notice) => {
        hint.textContent = notice || 'Choose a format to download';
        globalThis.odmFormatPresentation.populateChooser(chooser, choices, document, {
          fallbackTitle: record.videoTitle || document.title || 'Video',
          getTooltip: globalThis.odmSafeTooltip,
          onSelect: async (item, row) => {
            row.disabled = true;
            hint.textContent = 'Opening download options…';
            try {
              const videoId = record.videoId || item.videoId;
              const canonicalUrl = item.canonicalUrl || (videoId ? `https://www.youtube.com/watch?v=${videoId}` : item.url);
              const title = record.videoTitle || item.title || document.title;
              const filenameFormat = item.outputFormat || String(item.format || '').toLowerCase() || /\.([a-z0-9]+)(?:[?#]|$)/i.exec(item.url)?.[1] || 'mp4';
              const downloadPayload = {
                ...item,
                videoId,
                canonicalUrl,
                referer: canonicalUrl,
                filename: item.filename || `${title}.${filenameFormat}`,
                title,
                startNow: false
              };
              const response = await send({action:'download', item: downloadPayload});
              globalThis.odmMenuResult(response, {
                accepted: () => {
                  closeChoosers();
                },
                rejected: message => { hint.textContent = friendlyError(message); }
              });
            } catch(error) { hint.textContent = friendlyError(error.message); }
            row.disabled = !!item.unsupported;
          }
        });
        const chooserHeight = Math.min(chooser.scrollHeight || 360, Math.min(360, innerHeight * 0.6));
        const above = record.frozenChooserPos.top - chooserHeight - 4;
        chooser.style.top = `${below + chooserHeight <= innerHeight - 6 ? below : Math.max(6, above)}px`;
        globalThis.odmChooserKeys.focusFirst(chooser);
      };

      if (record.prefetchedChoices && record.prefetchedChoices.length) {
        chooser.append(hint);
        renderChoices(record.prefetchedChoices);
        return;
      }

      hint.textContent = 'Checking available video formats…';
      chooser.append(hint);
      try {
        const videoId = record.videoId || extractVideoId(record.target || record.video);
        const result = await send({action:'listForFrame', videoId});
        if (!result?.ok) throw new Error(result?.error || 'Could not list video formats.');
        const candidates = result?.items || [];
        const youtube = /(^|\.)youtube\.com$/.test(location.hostname);
        if (!youtube && url && !candidates.some(item => item.url === url)) {
          candidates.unshift({url, title: record.videoTitle || document.title});
        }
        let choices = globalThis.mediaChoices(candidates, {url, poster: video?.poster});
        const needsExtractor = !youtube && !choices.some(choice => choice.audioState === 'Video + audio');
        let extractionNotice;
        if (needsExtractor) {
          hint.textContent = 'Looking for available formats…';
          try {
            const extracted = await send({action:'extractFormats'});
            if (!extracted?.ok) throw new Error(extracted?.error || 'Could not extract video formats.');
            const extractedChoices = globalThis.mediaChoices(extracted.items || [], {url, poster: video?.poster});
            if (extractedChoices.length) {
              choices = extractedChoices;
            } else if (choices.length) {
              extractionNotice = 'No formats were found by the page extractor; showing detected media links.';
            }
          } catch(error) {
            if (!choices.length) throw error;
            extractionNotice = `Could not inspect page formats: ${error.message}. Showing detected media links.`;
          }
        }
        if (!choices.length) {
          hint.textContent = result.notice || (youtube ? 'Play the video briefly, then reopen this menu. No complete video with audio has been captured yet.' : 'No downloadable formats found. This player may require sign-in, DRM, or a stream format the extractor does not support.');
          return;
        }
        record.prefetchedChoices = choices;
        renderChoices(choices, extractionNotice);
      } catch(error) {
        hint.textContent = error.message;
      }
    });
    return record;
  }

  function scheduleScan() {
    if (stopped || pending) return;
    pending = true;
    scanFrame = requestAnimationFrame(() => scan());
  }
  function observeRoot(root) {
    if (observedRoots.has(root)) return;
    observedRoots.add(root);
    const observer = new MutationObserver(scheduleScan);
    observer.observe(root, {subtree:true, childList:true});
    mutationObservers.add(observer);
  }
  function scan(root = document) {
    pending = false;
    if(!connected())return;

    const isYouTube = typeof location !== 'undefined' && location.hostname && location.hostname.includes('youtube.com');
    if (root === document && isYouTube) {
      const cardSelectors = 'ytd-rich-item-renderer, ytd-video-renderer, ytd-compact-video-renderer, ytd-grid-video-renderer, yt-lockup-view-model';
      for (const card of root.querySelectorAll(cardSelectors)) {
        if (seen.has(card)) continue;
        const link = card.querySelector('a#thumbnail, a[href*="/watch?v="], a[href*="/shorts/"]');
        if (!link?.href) continue;
        const u = new URL(link.href, location.href);
        const videoId = u.searchParams.get('v') || /^\/shorts\/([\w-]{11})/.exec(u.pathname)?.[1];
        if (!videoId || !/^[\w-]{11}$/.test(videoId)) continue;
        seen.add(card);

        const titleEl = card.querySelector('#video-title, #title, h3, [title]');
        const videoTitle = titleEl?.getAttribute?.('title') || titleEl?.textContent?.trim?.() || document.title || 'Video';
        const target = card.querySelector('ytd-thumbnail, #thumbnail') || card;
        attachRecord({ card, target, videoId, videoTitle, canonicalUrl: `https://www.youtube.com/watch?v=${videoId}` });
      }
    }

    for (const video of root.querySelectorAll('video')) {
      if (seen.has(video)) continue;
      if (isYouTube && typeof video.closest === 'function' && video.closest('#inline-preview-player, [id*="preview"], ytd-preview')) {
        continue;
      }
      seen.add(video);
      attachRecord({ video, target: video, videoId: extractVideoId(video), videoTitle: extractVideoTitle(video) });
    }

    for (const element of root.querySelectorAll('*')) {
      if (element.shadowRoot) {
        observeRoot(element.shadowRoot);
        scan(element.shadowRoot);
      }
    }
    position();
  }

  function friendlyError(message) {
    if (/native messaging host not found/i.test(message || '')) return 'Chrome cannot find the desktop connector. Open Open Download Manager → Browser Integration → Refresh browsers, then reload this extension and page. Details: ' + message;
    if (/forbidden/i.test(message || '')) return 'Chrome native messaging permissions issue. Reload the extension at chrome://extensions and retry. Details: ' + message;
    return message;
  }
  function closeChoosers(except) {
    if (activeChooserRecord && activeChooserRecord !== except) {
      if (activeChooserRecord.chooser) { activeChooserRecord.chooser.remove(); activeChooserRecord.chooser = null; }
      if (activeChooserRecord.menuHost) { activeChooserRecord.menuHost.remove(); activeChooserRecord.menuHost = null; }
      if (!activeChooserRecord.hovered && activeChooserRecord.isFeed) {
        activeChooserRecord.host.style.display = 'none';
      }
      activeChooserRecord = null;
    }
    for(const record of records){
      if(record===except)continue;
      if(record.chooser){record.chooser.remove();record.chooser=null;}
      if(record.menuHost){record.menuHost.remove();record.menuHost=null;}
      if(record.shadow.querySelector('.chooser'))record.shadow.querySelector('.chooser').remove();
    }
    position();
  }
  function resolveParent(node) {
    const fullscreen = document.fullscreenElement;
    if (fullscreen && fullscreen !== node && typeof fullscreen.contains === 'function' && fullscreen.contains(node)) return fullscreen;
    return document.documentElement;
  }
  function position() {
    if(!connected())return;
    for (const record of records) {
      const liveNode = record.target || record.video;
      if (!liveNode || !liveNode.isConnected) {
        if (record.chooser || record.lingerTimer || record.hovered || activeChooserRecord === record) continue;
        if (liveNode) {
          resizeObserver.unobserve(liveNode);
          record.intersectionObserver?.disconnect();
        }
        record.sourceObserver?.disconnect();
        clearTimeout(record.lingerTimer);
        record.host.remove();
        if (record.menuHost) { record.menuHost.remove(); record.menuHost = null; }
        records.delete(record);
        continue;
      }
      const parent = resolveParent(liveNode);
      if(record.host.parentNode !== parent) parent.appendChild(record.host);

      if (activeChooserRecord && activeChooserRecord !== record) {
        if (record.isFeed) {
          record.host.style.display = 'none';
          continue;
        }
      }

      let rect = liveNode.getBoundingClientRect();
      const container = getContainer(record);
      if ((rect.width <= 100 || rect.height <= 60) && container) {
        const cRect = container.getBoundingClientRect();
        if (cRect.width > 100 && cRect.height > 60) {
          rect = cRect;
        }
      }

      const width = record.host.offsetWidth || 180, height = record.host.offsetHeight || 27;
      const place = globalThis.odmPanelPosition.placement(rect, {width: innerWidth, height: innerHeight}, {width, height}, record.dismissed || scrolling);
      if (place) record.savedPlace = place;

      const activePlace = place || record.savedPlace;
      const isPreview = record.isFeed;
      const shouldShow = isPreview
        ? (Boolean(record.formatsReady) && (record.hovered || record.lingerTimer !== null || activeChooserRecord === record) && Boolean(activePlace) && !record.dismissed && !scrolling)
        : (Boolean(place) && !record.dismissed && !scrolling);

      record.host.style.display = shouldShow ? 'block' : 'none';
      if (!activePlace || !shouldShow) continue;

      record.host.style.top = `${activePlace.top}px`;
      record.host.style.left = `${activePlace.left}px`;
    }
  }
  observeRoot(document.documentElement);
  document.addEventListener('pointerdown',event=>{
    const path=event.composedPath();
    if (activeChooserRecord) {
      const insideHost = path.includes(activeChooserRecord.host);
      const insideMenu = activeChooserRecord.menuHost && path.includes(activeChooserRecord.menuHost);
      if (!insideHost && !insideMenu) {
        closeChoosers();
      }
      return;
    }
    const inside=[...records].find(record=>path.includes(record.host) || (record.menuHost && path.includes(record.menuHost)));
    if(!inside)closeChoosers();
  },{capture:true,signal:listeners.signal});
  document.addEventListener('keydown',event=>{if(event.key==='Escape')closeChoosers();},{capture:true,signal:listeners.signal});
  function onScroll(event) {
    if(event?.composedPath?.().some(node=>[...records].some(record=>node===record.host || (record.menuHost && node===record.menuHost))))return;
    scrolling=true;closeChoosers();clearTimeout(scrollTimer);
    scrollTimer=setTimeout(()=>{scrolling=false;position();},140);
  }
  addEventListener('scroll', onScroll, {capture:true,signal:listeners.signal}); addEventListener('resize', position, {signal:listeners.signal}); document.addEventListener('fullscreenchange',position,{signal:listeners.signal});
  document.addEventListener('playing', event => {
    if (event.target && event.target.tagName === 'VIDEO') {
      for (const record of records) {
        if (record.hovered && !record.formatsReady) {
          prefetchFormats(record);
        }
      }
    }
  }, { capture: true, passive: true, signal: listeners.signal });
  function onMessage(message, sender, respond) {
    if (!connected() || sender.id !== api.runtime.id || message.action !== 'collectLinks') return;
    const items = Array.from(document.querySelectorAll('a[href]')).slice(0, 1000).map(a => ({url:direct(a.href), title:a.textContent.trim()})).filter(item => item.url);
    send({action:'remember', items:items.slice(0, 100)}).then(respond,error=>{try{respond({ok:false,error:error.message});}catch{}}).catch(()=>{});
    return true;
  }
  api.runtime.onMessage.addListener(onMessage);
  scan();
})();
