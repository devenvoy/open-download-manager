(() => {
  const api = globalThis.browser || chrome;
  const seen = new WeakSet();
  const records = new Set();
  let pending = false;
  let stopped = false, scanFrame, mutationObserver, scrollTimer, scrolling=false;
  const listeners = new AbortController();
  const resizeObserver = new ResizeObserver(() => position());
  const direct = value => { try { const u = new URL(value, location.href); return /^https?:$/.test(u.protocol) ? u.href : null; } catch { return null; } };
  const PANEL_CSS = ':host{font:12px -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;color:#0f172a}.bar{display:flex;justify-content:flex-end;align-items:stretch}.download,.dismiss{font:12px -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;background:#f8fafc;color:#0f172a;border:1px solid #cbd5e1;cursor:pointer;box-shadow:0 1px 3px rgba(0,0,0,0.1);height:27px;box-sizing:border-box}.download{border-radius:4px 0 0 4px;padding:3px 10px;font-weight:500}.dismiss{width:24px;padding:0;border-left:0;border-radius:0 4px 4px 0;font-size:15px;line-height:22px;color:#64748b}.download:hover,.dismiss:hover{background:#f1f5f9}.dismiss:hover{color:#b91c1c}.download:disabled{opacity:.6;cursor:not-allowed}.chooser{position:fixed;background:#ffffff;color:#0f172a;border:1px solid #cbd5e1;border-radius:6px;width:min(420px,calc(100vw - 12px));box-shadow:0 4px 16px rgba(0,0,0,0.15),0 1px 3px rgba(0,0,0,0.08);max-height:min(360px,60vh);overflow-y:auto;overflow-x:hidden;box-sizing:border-box;font-family:inherit;z-index:2147483647}.chooser p,.hint{margin:0;padding:8px 12px;font-size:11px;font-weight:500;color:#475569;border-bottom:1px solid #f1f5f9;line-height:1.4}.section-label{padding:6px 12px 4px;font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:0.05em;color:#64748b;background:#f8fafc;border-bottom:1px solid #f1f5f9}.row{display:flex;align-items:center;justify-content:space-between;width:100%;text-align:left;background:transparent;border:0;border-bottom:1px solid #f1f5f9;padding:7px 12px;cursor:pointer;box-sizing:border-box;font-family:inherit;color:inherit;transition:background .08s ease}.row:hover:not(:disabled),.row:focus-visible{background:#f1f5f9;outline:1px solid #0284c7;outline-offset:-1px}.row:disabled{opacity:.45;cursor:not-allowed}.row-left{display:flex;flex-direction:column;gap:3px;min-width:0;flex:1}.row-top{display:flex;align-items:center;flex-wrap:wrap;gap:6px}.badge-quality{display:inline-block;padding:1px 5px;font-size:10px;font-weight:700;border-radius:3px;line-height:1.3;text-transform:uppercase;background:#f1f5f9;color:#334155;border:1px solid #cbd5e1}.badge-4k,.badge-2k,.badge-fhd{background:#e0f2fe;color:#0369a1;border-color:#7dd3fc}.badge-hd{background:#f0fdf4;color:#15803d;border-color:#86efac}.badge-audio{background:#fef3c7;color:#92400e;border-color:#fde68a}.tag-format{font-size:11px;font-weight:600;color:#0f172a}.meta-sub{font-size:11px;color:#64748b}.row-title{font-size:11px;color:#64748b;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:340px}.row-right{display:flex;align-items:center;margin-left:8px;flex-shrink:0}.action-btn{font-size:13px;color:#64748b;font-weight:600}.row:hover:not(:disabled) .action-btn,.row:focus-visible .action-btn{color:#0284c7}@media(prefers-color-scheme:dark){:host{color:#f4f4f5}.download,.dismiss{background:#27272a;color:#f4f4f5;border-color:#3f3f46;box-shadow:0 1px 3px rgba(0,0,0,0.3)}.download:hover,.dismiss:hover{background:#3f3f46}.dismiss{color:#a1a1aa}.dismiss:hover{color:#f87171}.chooser{background:#18181b;color:#f4f4f5;border-color:#27272a;box-shadow:0 8px 24px rgba(0,0,0,0.45)}.chooser p,.hint{color:#a1a1aa;border-bottom-color:#27272a}.section-label{color:#71717a;background:#202023;border-bottom-color:#27272a}.row{border-bottom-color:#27272a;color:#f4f4f5}.row:hover:not(:disabled),.row:focus-visible{background:#27272a;outline-color:#38bdf8}.row-title{color:#a1a1aa}.badge-quality{background:#27272a;color:#e4e4e7;border-color:#3f3f46}.badge-fhd,.badge-4k,.badge-2k{background:#082f49;color:#7dd3fc;border-color:#0369a1}.badge-hd{background:#052e16;color:#86efac;border-color:#166534}.badge-audio{background:#451a03;color:#fcd34d;border-color:#92400e}.tag-format{color:#f4f4f5}.meta-sub{color:#71717a}.action-btn{color:#71717a}.row:hover:not(:disabled) .action-btn,.row:focus-visible .action-btn{color:#38bdf8}}';
  function shutdown() {
    if(stopped)return;
    stopped=true;
    listeners.abort();
    clearTimeout(scrollTimer);
    mutationObserver?.disconnect();resizeObserver.disconnect();
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
  function extractVideoId(video) {
    try {
      const pageUrl = new URL(location.href);
      const vParam = pageUrl.searchParams.get('v');
      if (vParam && /^[\w-]{11}$/.test(vParam)) return vParam;
      const shortsMatch = /^\/shorts\/([\w-]{11})/.exec(pageUrl.pathname);
      if (shortsMatch) return shortsMatch[1];
      const container = typeof video.closest === 'function' ? video.closest('#video-preview, ytd-video-preview, ytd-rich-item-renderer, yt-lockup-view-model, ytd-thumbnail, ytd-video-renderer, ytd-compact-video-renderer') : null;
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
  function extractVideoTitle(video) {
    try {
      const container = typeof video.closest === 'function' ? video.closest('#video-preview, ytd-video-preview, ytd-rich-item-renderer, yt-lockup-view-model, ytd-thumbnail, ytd-video-renderer, ytd-compact-video-renderer') : null;
      if (container && typeof container.querySelector === 'function') {
        const titleEl = container.querySelector('#video-title, #title, h3, [title]');
        const text = titleEl?.getAttribute?.('title') || titleEl?.textContent?.trim?.();
        if (text && text.length > 1) return text;
      }
    } catch {}
    return document.title || 'Video';
  }
  function scheduleLinger(record, ms) {
    clearTimeout(record.lingerTimer);
    record.lingerTimer = setTimeout(() => {
      if (!record.mouseInside) {
        if (record.chooser) {
          record.chooser.remove();
          record.chooser = null;
          if (record.menuHost) { record.menuHost.remove(); record.menuHost = null; }
        }
        if (record.host) record.host.style.display = 'none';
        record.lingerTimer = null;
        position();
      }
    }, ms);
  }
  function prefetchFormats(record) {
    if (record.prefetchedChoices || record.isPrefetching) return;
    const videoId = record.videoId || extractVideoId(record.video);
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
        }
      }
    }).catch(() => {
      record.isPrefetching = false;
    });
  }
  function scan() {
    pending = false;
    if(!connected())return;
    for (const video of document.querySelectorAll('video')) {
      if (seen.has(video)) continue;
      seen.add(video);
      const host = document.createElement('div');
      host.style.cssText = 'position:fixed;z-index:2147483647;display:none;pointer-events:auto';
      const shadow = host.attachShadow({mode:'closed'});
      shadow.innerHTML = `<style>${PANEL_CSS}</style><div class="bar"><button class="download" type="button">↓ Download this video ▾</button><button class="dismiss" type="button" title="Hide this download panel" aria-label="Hide this download panel">×</button></div>`;
      const button = shadow.querySelector('.download');
      const dismiss = shadow.querySelector('.dismiss');
      const record = {
        video,
        host,
        shadow,
        dismissed: false,
        chooser: null,
        menuHost: null,
        videoId: extractVideoId(video),
        videoTitle: extractVideoTitle(video),
        prefetchedChoices: null,
        isPrefetching: false,
        lingerTimer: null,
        mouseInside: false,
        frozenChooserPos: null
      };
      records.add(record);
      resizeObserver.observe(video);
      record.intersectionObserver = new IntersectionObserver(() => position(), {threshold: [0, 0.25, 0.5, 0.75, 1]});
      record.intersectionObserver.observe(video);
      document.documentElement.appendChild(host);

      if (typeof host.addEventListener === 'function') {
        host.addEventListener('mouseenter', () => {
          record.mouseInside = true;
          clearTimeout(record.lingerTimer);
          prefetchFormats(record);
        });
        host.addEventListener('mouseleave', () => {
          record.mouseInside = false;
          scheduleLinger(record, record.chooser ? 20000 : 5000);
        });
      }

      const container = typeof video.closest === 'function' ? video.closest('#video-preview, ytd-video-preview, ytd-rich-item-renderer, yt-lockup-view-model, ytd-thumbnail, ytd-video-renderer, ytd-compact-video-renderer') : null;
      if (container && typeof container.addEventListener === 'function') {
        container.addEventListener('mouseenter', () => {
          record.mouseInside = true;
          clearTimeout(record.lingerTimer);
          if (!record.videoId) {
            record.videoId = extractVideoId(video);
            record.videoTitle = extractVideoTitle(video);
          }
          prefetchFormats(record);
          position();
        }, {signal: listeners.signal});
        container.addEventListener('mouseleave', () => {
          record.mouseInside = false;
          scheduleLinger(record, record.chooser ? 20000 : 5000);
        }, {signal: listeners.signal});
      }

      const report = () => {
        const urls = [video.currentSrc, video.src, ...Array.from(video.querySelectorAll('source'), source => source.src)].map(direct).filter(Boolean);
        if (urls.length) send({action:'remember', items: urls.map(url => ({url, title:document.title}))}).catch(() => {});
      };
      video.addEventListener('loadedmetadata', report, {signal:listeners.signal});
      record.sourceObserver=new MutationObserver(report);
      record.sourceObserver.observe(video,{attributes:true,attributeFilter:['src'],subtree:true,childList:true});
      report();if(stopped)return;
      dismiss.addEventListener('click',event=>{
        event.preventDefault();event.stopPropagation();
        record.dismissed=true;
        if(record.chooser){record.chooser.remove();record.chooser=null;}
        if(record.menuHost){record.menuHost.remove();record.menuHost=null;}
        shadow.querySelector('.chooser')?.remove();
        host.style.display='none';
      });
      button.addEventListener('click', async event => {
        event.preventDefault(); event.stopPropagation();
        clearTimeout(record.lingerTimer);
        record.mouseInside = true;
        const url = direct(video.currentSrc || video.src || video.querySelector('source')?.src);
        {
          const existingChooser = record.chooser || shadow.querySelector('.chooser');
          if(existingChooser) {
            existingChooser.remove();
            record.chooser = null;
            if (record.menuHost) { record.menuHost.remove(); record.menuHost = null; }
            position();
            return;
          }
          closeChoosers(record);
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

          chooser.addEventListener('mouseenter', () => {
            record.mouseInside = true;
            clearTimeout(record.lingerTimer);
          });
          chooser.addEventListener('mouseleave', () => {
            record.mouseInside = false;
            scheduleLinger(record, 20000);
          });

          const isContained = resolveParent(video) !== document.documentElement;
          let activeShadow = shadow;
          if (isContained) {
            const menuHost = document.createElement('div');
            menuHost.style.cssText = 'position:fixed;z-index:2147483647;top:0;left:0;pointer-events:auto;';
            const menuShadow = menuHost.attachShadow({mode:'closed'});
            menuShadow.innerHTML = `<style>${PANEL_CSS}</style>`;
            menuShadow.append(chooser);
            document.documentElement.appendChild(menuHost);
            record.menuHost = menuHost;
            record.menuShadow = menuShadow;
            activeShadow = menuShadow;
          } else {
            shadow.append(chooser);
          }

          chooser.addEventListener('keydown', event => {
            globalThis.odmChooserKeys.handleKey(event, chooser, activeShadow.activeElement, {
              close: () => {
                chooser.remove();
                record.chooser = null;
                if (record.menuHost) { record.menuHost.remove(); record.menuHost = null; }
                position();
                button.focus();
              }
            });
          });
          const hint = document.createElement('p');
          const renderChoices = (choices) => {
            hint.textContent = 'Choose a format to download';
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
                  const downloadPayload = {
                    ...item,
                    videoId,
                    canonicalUrl,
                    referer: canonicalUrl,
                    title,
                    startNow: false
                  };
                  const response = await send({action:'download', item: downloadPayload});
                  globalThis.odmMenuResult(response, {
                    accepted: () => {
                      chooser.remove();
                      record.chooser = null;
                      if (record.menuHost) { record.menuHost.remove(); record.menuHost = null; }
                      position();
                    },
                    rejected: message => { hint.textContent = friendlyError(message); }
                  });
                } catch(error) { hint.textContent = friendlyError(error.message); }
                row.disabled = !!item.unsupported;
              }
            });
            globalThis.odmChooserKeys.focusFirst(chooser);
            position();
          };

          if (record.prefetchedChoices && record.prefetchedChoices.length) {
            chooser.append(hint);
            renderChoices(record.prefetchedChoices);
            return;
          }

          hint.textContent = 'Checking available video formats…';
          chooser.append(hint);
          position();
          try {
            const videoId = record.videoId || extractVideoId(video);
            console.log('[ODM Content] Requesting listForFrame with videoId:', videoId, 'url:', url);
            const result=await send({action:'listForFrame', videoId});
            console.log('[ODM Content] listForFrame result:', result);
            if(!result?.ok)throw new Error(result?.error||'Could not list video formats.');
            const candidates=result?.items||[];
            const youtube=/(^|\.)youtube\.com$/.test(location.hostname);
            if(!youtube&&url&&!candidates.some(item=>item.url===url)) candidates.unshift({url,title:record.videoTitle||document.title});
            if (!candidates.length) { hint.textContent=result.notice || (youtube?'Play the video briefly, then reopen this menu. No complete video with audio has been captured yet.':'No direct media links detected. Play the video and try again.'); return; }
            const choices=globalThis.mediaChoices(candidates,{url,poster:video.poster});
            if(!choices.length){hint.textContent='No complete formats detected for this video yet. Play it briefly, then try again.';return;}
            record.prefetchedChoices = choices;
            renderChoices(choices);
          } catch(error) {hint.textContent=error.message;}
          return;
        }
      });
    }
    position();
  }
  function friendlyError(message) {
    if (/native messaging host not found/i.test(message || '')) return 'Chrome cannot find the desktop connector. Open Open Download Manager → Browser Integration → Refresh browsers, then reload this extension and page. Details: ' + message;
    if (/forbidden/i.test(message || '')) return 'Chrome native messaging permissions issue. Reload the extension at chrome://extensions and retry. Details: ' + message;
    return message;
  }
  function closeChoosers(except) {
    for(const record of records){
      if(record===except)continue;
      if(record.chooser){record.chooser.remove();record.chooser=null;}
      if(record.menuHost){record.menuHost.remove();record.menuHost=null;}
      if(record.shadow.querySelector('.chooser'))record.shadow.querySelector('.chooser').remove();
    }
    position();
  }
  function resolveParent(video) {
    const fullscreen = document.fullscreenElement;
    if (fullscreen && fullscreen !== video && typeof fullscreen.contains === 'function' && fullscreen.contains(video)) return fullscreen;
    try {
      const ytPreview = typeof video.closest === 'function' ? video.closest('#video-preview, ytd-video-preview, #inline-preview-player') : null;
      if (ytPreview && typeof ytPreview.appendChild === 'function') return ytPreview;
    } catch {}
    return document.documentElement;
  }
  function isPreview(video, rect) {
    try {
      if (typeof location !== 'undefined' && location.hostname && location.hostname.includes('youtube.com')) {
        const isMainWatchPlayer = location.pathname === '/watch' && typeof video.closest === 'function' && video.closest('#movie_player') && !video.closest('#inline-preview-player, [id*="preview"]');
        if (!isMainWatchPlayer) return true;
      }
      if (video && typeof video.closest === 'function' && video.closest('#inline-preview-player, ytd-inline-preview-renderer, ytd-thumbnail, ytd-video-preview, yt-lockup-view-model, ytd-rich-item-renderer, [id*="preview"], [class*="preview"], [data-preview]')) return true;
      if (video && video.parentElement && typeof video.parentElement.closest === 'function' && video.parentElement.closest('a[href], yt-lockup-view-model, ytd-rich-item-renderer, ytd-video-renderer')) return true;
      if (rect && rect.width > 0 && rect.width < 700 && rect.height > 0 && rect.height < 450) return true;
    } catch {}
    return false;
  }
  function position() {
    if(!connected())return;
    for (const record of records) {
      if (!record.video.isConnected) {
        if (record.chooser || record.shadow.querySelector('.chooser') || record.lingerTimer || record.mouseInside) continue;
        resizeObserver.unobserve(record.video); record.sourceObserver.disconnect(); record.intersectionObserver?.disconnect();
        clearTimeout(record.lingerTimer);
        record.host.remove();
        if (record.menuHost) { record.menuHost.remove(); record.menuHost = null; }
        records.delete(record);
        continue;
      }
      const parent = resolveParent(record.video);
      if(record.host.parentNode !== parent) parent.appendChild(record.host);
      const isContained = parent !== document.documentElement;
      if (isContained) {
        record.host.style.position = 'absolute';
        record.host.style.top = '6px';
        record.host.style.right = '8px';
        record.host.style.left = 'auto';
        record.host.style.zIndex = '2147483647';
        record.host.style.display = (record.dismissed && !record.chooser) || scrolling ? 'none' : 'block';
        const chooser = record.chooser || record.shadow.querySelector('.chooser');
        if (chooser) {
          const btn = record.shadow.querySelector('.download');
          let bRect = (btn && typeof btn.getBoundingClientRect === 'function') ? btn.getBoundingClientRect() : null;
          if (!bRect || (bRect.width === 0 && bRect.height === 0)) {
            bRect = record.frozenChooserPos;
          } else if (bRect.width > 0) {
            record.frozenChooserPos = bRect;
          }
          if (!bRect) bRect = { top: 0, bottom: 27, right: innerWidth - 8 };
          const chooserWidth = Math.min(420, innerWidth - 12);
          const chooserHeight = Math.min(chooser.scrollHeight || 360, Math.min(360, innerHeight * 0.6));
          chooser.style.position = 'fixed';
          chooser.style.width = `${chooserWidth}px`;
          const left = Math.max(6, Math.min(innerWidth - chooserWidth - 6, bRect.right - chooserWidth));
          chooser.style.left = `${left}px`;
          const below = bRect.bottom + 4;
          const above = bRect.top - chooserHeight - 4;
          chooser.style.top = `${below + chooserHeight <= innerHeight - 6 ? below : Math.max(6, above)}px`;
        }
        continue;
      }
      const rect = record.video.getBoundingClientRect();
      const width = record.host.offsetWidth || 180, height = record.host.offsetHeight || 27;
      const inside = isPreview(record.video, rect);
      const place = globalThis.odmPanelPosition.placement(rect, {width: innerWidth, height: innerHeight}, {width, height}, record.dismissed || scrolling, { inside });
      record.host.style.display = (record.chooser || record.lingerTimer || record.mouseInside) ? 'block' : (place ? 'block' : 'none');
      if(!place && !record.chooser && !record.lingerTimer && !record.mouseInside)continue;
      if (place) {
        record.host.style.top = `${place.top}px`;
        record.host.style.left = `${place.left}px`;
      }
      const chooser=record.chooser || record.shadow.querySelector('.chooser');
      if(chooser){
        const chooserWidth=Math.min(420,innerWidth-10), chooserHeight=Math.min(chooser.scrollHeight||360,Math.min(360,innerHeight*.6));
        let hostRect=record.host.getBoundingClientRect();
        if (!hostRect || (hostRect.width === 0 && hostRect.height === 0)) {
          hostRect = record.frozenChooserPos;
        } else if (hostRect.width > 0) {
          record.frozenChooserPos = hostRect;
        }
        if (!hostRect) hostRect = { top: 0, bottom: 27, right: innerWidth - 8 };
        chooser.style.left=`${Math.max(5,Math.min(innerWidth-chooserWidth-5,hostRect.right-chooserWidth))}px`;
        const below=hostRect.bottom+4, aboveMenu=hostRect.top-chooserHeight-4;
        chooser.style.top=`${below+chooserHeight<=innerHeight-5?below:Math.max(5,aboveMenu)}px`;
      }
    }
  }
  mutationObserver=new MutationObserver(() => { if (!stopped && !pending) { pending = true; scanFrame=requestAnimationFrame(scan); } });
  mutationObserver.observe(document.documentElement, {subtree:true, childList:true});
  document.addEventListener('pointerdown',event=>{
    const path=event.composedPath();
    const inside=[...records].find(record=>path.includes(record.host) || (record.menuHost && path.includes(record.menuHost)));
    if(!inside)closeChoosers();
  },{capture:true,signal:listeners.signal});
  document.addEventListener('keydown',event=>{if(event.key==='Escape')closeChoosers();},{capture:true,signal:listeners.signal});
  function onScroll(event) {
    // Scrolling the quality list itself must remain usable.
    if(event?.composedPath?.().some(node=>[...records].some(record=>node===record.host || (record.menuHost && node===record.menuHost))))return;
    scrolling=true;closeChoosers();clearTimeout(scrollTimer);
    scrollTimer=setTimeout(()=>{scrolling=false;position();},140);
  }
  addEventListener('scroll', onScroll, {capture:true,signal:listeners.signal}); addEventListener('resize', position, {signal:listeners.signal}); document.addEventListener('fullscreenchange',position,{signal:listeners.signal});
  function onMessage(message, sender, respond) {
    if (!connected() || sender.id !== api.runtime.id || message.action !== 'collectLinks') return;
    const items = Array.from(document.querySelectorAll('a[href]')).slice(0, 1000).map(a => ({url:direct(a.href), title:a.textContent.trim()})).filter(item => item.url);
    send({action:'remember', items:items.slice(0, 100)}).then(respond,error=>{try{respond({ok:false,error:error.message});}catch{}}).catch(()=>{});
    return true;
  }
  api.runtime.onMessage.addListener(onMessage);
  scan();
})();
