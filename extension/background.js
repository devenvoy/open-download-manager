// Created: 2026-10-24
// Reason: Initial public release
// Last updated: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Description: Extension service worker managing browser download interception, context menus, and native messaging communication.

/* Downloads require extension UI messages; page player metadata is bounded and sanitized. */
if(typeof importScripts==='function') importScripts('media-policy.js','hls.js','youtube-media.js','capture-evaluator.js');
const api = globalThis.browser || chrome;
const HOST = 'com.open.downloadmanager';
const CACHE_KEY = 'mediaByTab';
const SABR_KEY = 'sabrByTab';
const HEARTBEAT_ALARM = 'odm-heartbeat';
// The heartbeat tells the desktop app that this browser is connected. It runs
// every two minutes while the desktop app is up and backs off to fifteen
// minutes while it is closed, so a closed app costs almost nothing.
const HEARTBEAT_PERIOD_MINUTES = 2;
const HEARTBEAT_BACKOFF_MINUTES = 15;
let serial = Promise.resolve();
let cachedCapturePolicy = null;
// The desktop app uses this to know the extension is really installed and
// enabled. Browser profile folders are unreadable for other apps on macOS,
// so the live signal is the only reliable proof.
function detectBrowser() {
  const ua = (globalThis.navigator && navigator.userAgent) || '';
  if (/Edg\//.test(ua)) return 'Edge';
  if (/OPR\/|Opera/.test(ua)) return 'Opera';
  if (/Brave\//.test(ua)) return 'Brave';
  if (/Firefox\//.test(ua)) return 'Firefox';
  if (/Chrome\//.test(ua)) return 'Chrome';
  return 'Unknown';
}
async function sendHeartbeat() {
  try {
    await nativeSend({action: 'heartbeat'});
    setHeartbeatPeriod(HEARTBEAT_PERIOD_MINUTES);
  } catch (error) {
    // The desktop app is closed: back off instead of retrying every minute.
    if (error && error.appRunning === false) setHeartbeatPeriod(HEARTBEAT_BACKOFF_MINUTES);
  }
}
function setHeartbeatPeriod(minutes) {
  try {
    if (!api.alarms) return;
    api.alarms.create(HEARTBEAT_ALARM, {periodInMinutes: minutes});
  } catch {}
}
function scheduleHeartbeat() {
  setHeartbeatPeriod(HEARTBEAT_PERIOD_MINUTES);
}
try {
  if (api.alarms && api.alarms.onAlarm) api.alarms.onAlarm.addListener(alarm => { if (alarm.name === HEARTBEAT_ALARM) sendHeartbeat(); });
} catch {}
async function getCapturePolicy() {
  if (cachedCapturePolicy) return cachedCapturePolicy;
  try {
    const data = await api.storage.local.get('capturePolicy');
    if (data?.capturePolicy) {
      cachedCapturePolicy = data.capturePolicy;
      return cachedCapturePolicy;
    }
  } catch {}
  return null;
}
async function refreshCapturePolicy() {
  try {
    const res = await nativeSend({ action: 'policy' });
    if (res?.policy) {
      cachedCapturePolicy = res.policy;
      await api.storage.local.set({ capturePolicy: res.policy });
    }
  } catch {}
}
// Credentials stay in worker memory and are used only for the selected observed URL.
const requestContext = new Map();
const youtubePlayers=new Map(), youtubePlayersByVideo=new Map(), youtubeObserved=new Map(), youtubeSabrObserved=new Map(), youtubeSabrSeed=new Map();
function rememberSabrCapture(tabId,seed){
  console.log('[ODM Background] Captured SABR session for tabId:', tabId, 'url:', seed.url);
  youtubeSabrObserved.set(tabId,seed);
  if(youtubeSabrObserved.size>50)youtubeSabrObserved.delete(youtubeSabrObserved.keys().next().value);
  updateSession(SABR_KEY,map=>{map[tabId]={url:seed.url,requestBody:seed.requestBody};}).catch(()=>{});
}
async function sabrCapture(tabId){
  const live=youtubeSabrObserved.get(tabId);
  if(live)return live;
  await serial;
  try{
    const data=await api.storage.session.get(SABR_KEY),stored=data[SABR_KEY]?.[tabId];
    if(stored&&typeof stored.url==='string'&&typeof stored.requestBody==='string'){
      const capture={...stored,tabId};youtubeSabrObserved.set(tabId,capture);return capture;
    }
  }catch{}
}
function clearSabrCapture(tabId){
  youtubeSabrObserved.delete(tabId);
  for(const [id,seed] of youtubeSabrSeed)if(seed.tabId===tabId)youtubeSabrSeed.delete(id);
  updateSession(SABR_KEY,map=>{delete map[tabId];}).catch(()=>{});
}
function bytesBase64(bytes){let binary='';const view=new Uint8Array(bytes);for(let i=0;i<view.length;i+=8192)binary+=String.fromCharCode(...view.subarray(i,i+8192));return btoa(binary);}
api.webRequest.onBeforeRequest?.addListener(details=>{
  const url=globalThis.youtubeMedia?.sabrUrl(details.url), raw=details.requestBody?.raw;
  if(!url||details.tabId<0||!Array.isArray(raw)||!raw.length)return;
  let size=0;for(const part of raw){if(!part.bytes)return;size+=part.bytes.byteLength;}if(!size||size>262144)return;
  const bytes=new Uint8Array(size);let offset=0;for(const part of raw){bytes.set(new Uint8Array(part.bytes),offset);offset+=part.bytes.byteLength;}
  youtubeSabrSeed.set(details.requestId,{tabId:details.tabId,url,requestBody:bytesBase64(bytes)});
  if(youtubeSabrSeed.size>100)youtubeSabrSeed.delete(youtubeSabrSeed.keys().next().value);
},{urls:['https://*.googlevideo.com/videoplayback*','https://googlevideo.com/videoplayback*']},['requestBody']);
const requestMethods=new Map();
function rememberRequest(details) {
  if(details.tabId<0 || !httpUrl(details.url))return;
  let tab=requestContext.get(details.tabId);
  if(!tab){tab=new Map();requestContext.set(details.tabId,tab);if(requestContext.size>50)requestContext.delete(requestContext.keys().next().value);}
  const headers={};
  for(const h of details.requestHeaders||[]) {
    const name=h.name.toLowerCase();
    if(['cookie','authorization','user-agent','referer','origin'].includes(name)&&typeof h.value==='string'&&h.value.length<=16384&&!/[\r\n]/.test(h.value))headers[name]=h.value;
  }
  const method=typeof details.method==='string'?details.method.toUpperCase():'GET';
  tab.set(details.url,{method,headers});if(tab.size>100)tab.delete(tab.keys().next().value);
  if(method!=='GET'&&method!=='HEAD'){
    requestMethods.set(details.url,method);
    if(requestMethods.size>200)requestMethods.delete(requestMethods.keys().next().value);
  } else requestMethods.delete(details.url);
}
// Chromium (which now also defines `browser`) withholds Cookie and Referer unless extraHeaders is requested; Firefox has no such option.
api.webRequest.onBeforeSendHeaders?.addListener(rememberRequest,{urls:['http://*/*','https://*/*']},['requestHeaders',...(api.webRequest.OnBeforeSendHeadersOptions?.EXTRA_HEADERS?['extraHeaders']:[])]);
function httpUrl(value) {
  try { const u = new URL(value); return ['http:', 'https:'].includes(u.protocol) && !u.username && !u.password ? u.href : null; } catch { return null; }
}
// A download can only be handed to the desktop when its URL can be fetched again.
// POST form downloads (Google Drive folder archives), blob/data downloads, and
// other browser-generated responses must stay with the browser.
function nonReplayableDownload(url, tabId) {
  const observed = tabId !== undefined && tabId >= 0 ? requestContext.get(tabId)?.get(url) : undefined;
  const method = observed?.method || requestMethods.get(url);
  if (method && method !== 'GET' && method !== 'HEAD') return true;
  try {
    const parsed = new URL(url);
    // Drive folder/zip downloads post hidden fields to this bare endpoint; the
    // POST body is invisible to extensions, so a GET replay returns HTML.
    if (parsed.hostname === 'drive.usercontent.google.com' && parsed.pathname === '/download' && !parsed.searchParams.has('id')) return true;
  } catch {}
  return false;
}
function isTorrentDownload(url, filename) {
  if (/\.torrent$/i.test(String(filename || '').trim())) return true;
  try { return /\.torrent$/i.test(new URL(url).pathname); } catch { return false; }
}
// Browsers cannot confirm from the service worker, so ask the content script
// that owns the tab. Without one, leave the download with the browser.
async function confirmTorrentHandoff(tabId) {
  if (tabId === undefined || tabId < 0 || typeof api.tabs?.sendMessage !== 'function') return false;
  try {
    const response = await api.tabs.sendMessage(tabId, {action: 'confirmTorrent', kind: 'file'});
    return response?.confirmed === true;
  } catch {
    return false;
  }
}
async function eraseBrowserDownload(item) {
  try {
    await api.downloads.cancel(item.id);
    if (typeof api.downloads.erase === 'function') {
      await api.downloads.erase({ id: item.id });
    }
  } catch {}
}
function unsupported(url, mime = '') {
  const isDash = globalThis.mediaPolicy ? globalThis.mediaPolicy.isDashMime(mime) : /dash\+xml/i.test(mime);
  return /\.mpd(?:[?#]|$)/i.test(url) || isDash;
}
function isFragment(url) { return /\.(m4s|ts|aac)(?:[?#]|$)/i.test(url) || /\/init[^/]*\.mp4(?:[?#]|$)/i.test(url); }
const hlsSeen=new Map(); let hlsActive=0;
async function discoverHls(tabId,url) {
  if(tabId<0 || hlsActive>=8)return;
  let seen=hlsSeen.get(tabId);if(!seen){seen=new Set();hlsSeen.set(tabId,seen);if(hlsSeen.size>50)hlsSeen.delete(hlsSeen.keys().next().value);}
  if(seen.has(url)||seen.size>=20)return;seen.add(url);hlsActive++;
  const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),10000);
  try {
    const response=await fetch(url,{signal:controller.signal,credentials:'include'});
    if(!response.ok)throw new Error('Manifest fetch failed.');
    const reader=response.body.getReader();let size=0;const chunks=[];
    while(true){const {done,value}=await reader.read();if(done)break;size+=value.length;if(size>1048576){await reader.cancel();throw new Error('Manifest too large.');}chunks.push(value);}
    const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
    const variants=globalThis.parseHlsMaster(new TextDecoder().decode(bytes),response.url||url);
    let title='';try{title=(await api.tabs.get(tabId)).title||'';}catch{}
    if(hlsSeen.get(tabId)===seen)await remember(tabId,variants.map(item=>({...item,title})));
  } catch {
    await remember(tabId,[{url,mime:'application/vnd.apple.mpegurl',format:'HLS',unsupported:true,reason:'Could not read the HLS playlist. The stream may require site authorization.'}]).catch(()=>{});
  } finally {clearTimeout(timer);hlsActive--;}
}
function nativeSend(payload) {
  return new Promise((resolve, reject) => {
    const message = Object.assign({}, payload, {browser: detectBrowser()});
    try {
      const manifest = api.runtime.getManifest && api.runtime.getManifest();
      if (manifest && manifest.version) message.extensionVersion = manifest.version;
    } catch {}
    const failure = (text, result) => {
      const error = new Error(text);
      if (result && typeof result.appRunning === 'boolean') error.appRunning = result.appRunning;
      reject(error);
    };
    // Chrome callbacks retain useful runtime.lastError diagnostics.
    if (!globalThis.browser) chrome.runtime.sendNativeMessage(HOST, message, result => {
      if (chrome.runtime.lastError) reject(new Error(chrome.runtime.lastError.message));
      else if (!result?.ok) failure(result?.error || 'The desktop app did not accept this download.', result);
      else resolve(result);
    });
    else browser.runtime.sendNativeMessage(HOST, message).then(result => result?.ok ? resolve(result) : failure(result?.error || 'Desktop rejected download.', result), reject);
  });
}
async function enqueue(item, tabId) {
  let sabr;
  if(item.sabrSelection){
    const player=(item.videoId&&youtubePlayersByVideo.get(item.videoId))||youtubePlayers.get(tabId), selection=item.sabrSelection;
    const videoItag=selection.videoItag===undefined?undefined:Number(selection.videoItag), audioItag=Number(selection.audioItag);
    if(!player||!Number.isSafeInteger(audioItag)||audioItag<=0||videoItag!==undefined&&(!Number.isSafeInteger(videoItag)||videoItag<=0))throw new Error('The captured YouTube session expired. Reopen the menu.');
    const audio=player.formats.find(f=>f.itag===audioItag&&/^audio\/mp4/i.test(f.mimeType));
    const video=videoItag===undefined?undefined:player.formats.find(f=>f.itag===videoItag&&/^video\/mp4/i.test(f.mimeType));
    const captured=await sabrCapture(tabId);
    const serverAbrStreamingUrl=captured?.url || player.sabrUrl;
    if(!audio||videoItag!==undefined&&!video||!globalThis.youtubeMedia.sabrUrl(serverAbrStreamingUrl)||!player.ustreamerConfig)throw new Error('The captured YouTube SABR session is incomplete. Reopen the menu.');
    // Native messaging is limited to 1 MiB. The desktop only needs the two
    // selected tracks, not every format advertised by YouTube's player.
    sabr={videoId:player.videoId,serverAbrStreamingUrl,videoPlaybackUstreamerConfig:player.ustreamerConfig,clientInfo:{clientName:player.clientName,clientVersion:player.clientVersion},formats:[video,audio].filter(Boolean),videoItag,audioItag,requestBody:captured?.requestBody};
  }
  const url = httpUrl(sabr?.serverAbrStreamingUrl||item.url);
  if (!url) throw new Error('Only direct HTTP and HTTPS links are supported.');
  if (unsupported(url, item.mime) || item.unsupported) throw new Error(item.reason || 'DASH streams are unsupported.');
  const policy = await getCapturePolicy();
  let startNow = item.startNow === true;
  if (globalThis.captureEvaluator && policy) {
    const outcome = globalThis.captureEvaluator.evaluateCapture(policy, url, item.filename);
    if (outcome.decision === 'excluded-host' || outcome.decision === 'excluded-address') {
      throw new Error(outcome.reason);
    }
    if (outcome.decision === 'confirmation-required') {
      startNow = false;
    }
  }
  const requestContextEntry=tabId !== undefined ? requestContext.get(tabId)?.get(url) : undefined;
  const requestHeaders=requestContextEntry?.headers;
  const effectiveReferer = item.canonicalUrl || (item.videoId ? `https://www.youtube.com/watch?v=${item.videoId}` : null) || item.referer || requestHeaders?.referer;
  return nativeSend({
    action: 'download',
    url,
    youtubeFormat: item.youtubeFormat,
    sabr,
    startNow,
    audioUrl: httpUrl(item.audioUrl) || undefined,
    outputFormat: item.outputFormat,
    filename: (typeof item.filename === 'string' && item.filename.trim()) ? item.filename.trim() : undefined,
    referer: httpUrl(effectiveReferer) || undefined,
    canonicalUrl: item.canonicalUrl || (item.videoId ? `https://www.youtube.com/watch?v=${item.videoId}` : undefined),
    videoId: item.videoId,
    requestHeaders
  });
}
function updateSession(key,change) {
  const next = serial.then(async () => {
    const data = await api.storage.session.get(key);
    const map = data[key] || {};
    await change(map);
    // Bound total tracked tabs as well as each tab's results.
    const keys = Object.keys(map);
    keys.slice(0, Math.max(0, keys.length - 50)).forEach(entry => delete map[entry]);
    await api.storage.session.set({[key]: map});
  });
  serial = next.catch(() => {});
  return next;
}
const updateCache=change=>updateSession(CACHE_KEY,change);
async function remember(tabId, items) {
  if (tabId < 0) return;
  await updateCache(map => {
    const prior = map[tabId] || [];
    for (const item of items.slice(0, 100)) {
      const url = httpUrl(item.url);
      if (!url || isFragment(url)) continue;
      const value={url, mime: String(item.mime || '').slice(0, 100), media:item.media===true, title: String(item.title || '').slice(0, 180), quality:String(item.quality||'').slice(0,40),bitrate:Number(item.bitrate)||0,format:String(item.format||'').slice(0,20),separateAudio:!!item.separateAudio,reason:String(item.reason||'').slice(0,180),unsupported: unsupported(url, item.mime)||!!item.unsupported};
      value.audioUrl=httpUrl(item.audioUrl)||undefined;
      value.codecs=String(item.codecs||'').slice(0,200);
      value.masterUrl=httpUrl(item.masterUrl)||undefined;
      const existing=prior.findIndex(v=>v.url===url);
      if(existing>=0){const previous=prior[existing];if(previous.format==='HLS'&&!item.format){if(value.title)previous.title=value.title;continue;}const mime=value.mime||previous.mime;prior[existing]={...previous,...value,mime,media:value.media||previous.media,audioUrl:value.audioUrl||previous.audioUrl,codecs:value.codecs||previous.codecs,masterUrl:value.masterUrl||previous.masterUrl,title:value.title||previous.title,quality:value.quality||previous.quality,bitrate:value.bitrate||previous.bitrate,format:value.format||previous.format,separateAudio:value.separateAudio||previous.separateAudio,unsupported:unsupported(url,mime)||value.unsupported||previous.unsupported,reason:value.reason||previous.reason};continue;}
      prior.push(value);
    }
    map[tabId] = prior.slice(-100);
  });
}
api.webRequest.onHeadersReceived.addListener(details => {
  const mime = details.responseHeaders?.find(h => h.name.toLowerCase() === 'content-type')?.value || '';
  const captured=globalThis.youtubeMedia?.mediaUrl(details.url);
  const capturedSabr=globalThis.youtubeMedia?.sabrUrl(details.url);
  if(capturedSabr && details.tabId>=0 && [200,206].includes(details.statusCode)){
    const seed=youtubeSabrSeed.get(details.requestId);
    if(seed?.tabId===details.tabId&&seed.url===capturedSabr)rememberSabrCapture(details.tabId,seed);
    youtubeSabrSeed.delete(details.requestId);
  }
  if(captured && details.tabId>=0 && [200,206].includes(details.statusCode) && /^(video|audio)\//i.test(mime)) {
    const urls=youtubeObserved.get(details.tabId)||[];
    if(!urls.includes(captured))urls.unshift(captured);
    youtubeObserved.set(details.tabId,urls.slice(0,100));
    if(youtubeObserved.size>50)youtubeObserved.delete(youtubeObserved.keys().next().value);
  }
  const policy = globalThis.mediaPolicy;
  const isMediaMime = policy ? policy.isMediaMime(mime) : /^(video|audio)\//i.test(mime);
  const isMediaExt = policy ? policy.isMediaExtension(details.url) : /\.(mp4|webm|mkv|avi|mov|m4v|flv|wmv|mp3|m4a|ogg|wav)(?:[?#]|$)/i.test(details.url);
  const isHls = policy ? (policy.isHlsMime(mime) || /\.m3u8(?:[?#]|$)/i.test(details.url)) : (/\.m3u8(?:[?#]|$)/i.test(details.url) || /mpegurl/i.test(mime));
  const isDash = policy ? (policy.isDashMime(mime) || /\.mpd(?:[?#]|$)/i.test(details.url)) : (/\.mpd(?:[?#]|$)/i.test(details.url) || /dash\+xml/i.test(mime));

  const isMedia = details.type === 'media' || isMediaMime || isMediaExt || isHls;
  if (isMedia || isDash || unsupported(details.url, mime)) {
    remember(details.tabId, [{
      url: details.url,
      mime,
      media: isMedia,
      unsupported: isDash,
      reason: isDash ? 'DASH streams requiring dynamic manifests or DRM are not supported yet. Choose an MP4 or HLS format.' : ''
    }]).catch(() => {});
  }
  if (isHls) discoverHls(details.tabId, details.url);
}, {urls: ['http://*/*', 'https://*/*']}, ['responseHeaders']);
api.tabs.onRemoved.addListener(tabId => {youtubePlayers.delete(tabId);youtubePlayersByVideo.clear();youtubeObserved.delete(tabId);clearSabrCapture(tabId);requestContext.delete(tabId);hlsSeen.delete(tabId);updateCache(map => { delete map[tabId]; }).catch(() => {});});
api.tabs.onUpdated.addListener((tabId, change) => { if (change.status === 'loading') {youtubePlayers.delete(tabId);youtubePlayersByVideo.clear();youtubeObserved.delete(tabId);clearSabrCapture(tabId);requestContext.delete(tabId);hlsSeen.delete(tabId);updateCache(map => { delete map[tabId]; }).catch(() => {});} });
api.runtime.onInstalled.addListener(async () => {
  try {
    await api.contextMenus.removeAll();
    api.contextMenus.create({id: 'odm-link', title: 'Download with Open Download Manager', contexts: ['link', 'video', 'audio']});
    api.contextMenus.create({id: 'odm-page', title: 'Find downloadable links on this page', contexts: ['page']});
  } catch (error) { await api.storage.local.set({lastError: error.message}); }
  refreshCapturePolicy().catch(() => {});
  scheduleHeartbeat();
  sendHeartbeat();
});
api.contextMenus.onClicked.addListener(async (info, tab) => {
  try {
    if (info.menuItemId === 'odm-page') { await api.tabs.sendMessage(tab.id, {action: 'collectLinks'}); await api.action.setBadgeText({tabId: tab.id, text: 'LINK'}); return; }
    const targetUrl = info.linkUrl || info.srcUrl;
    const policy = await getCapturePolicy();
    let startNow = true;
    if (globalThis.captureEvaluator && policy) {
      const outcome = globalThis.captureEvaluator.evaluateCapture(policy, targetUrl);
      if (outcome.decision === 'excluded-host' || outcome.decision === 'excluded-address') {
        throw new Error(outcome.reason);
      }
      if (outcome.decision === 'confirmation-required') {
        startNow = false;
      }
    }
    await enqueue({url: targetUrl, referer: tab?.url, startNow}, tab?.id);
    await api.action.setBadgeText({tabId: tab.id, text: 'OK'});
  } catch (e) {
    await api.storage.local.set({lastError: e.message});
    if (tab?.id) await api.action.setBadgeText({tabId: tab.id, text: '!'});
  }
});
api.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== api.runtime.id) return false;
  const task = async () => {
    if(message.action==='playerResponse' && sender.tab) {
      const page=new URL(sender.url || sender.tab.url || 'https://invalid.local');
      if(!['www.youtube.com','youtube.com','m.youtube.com'].includes(page.hostname))throw new Error('Invalid player source.');
      const data=message.data;
      if(!data || !Array.isArray(data.formats) || data.formats.length>150)throw new Error('Invalid player data.');
      const clean=globalThis.youtubeMedia.sanitizePlayer(data);
      if(clean){
        const previous=youtubePlayers.get(sender.tab.id);
        if(previous&&previous.videoId!==clean.videoId){clearSabrCapture(sender.tab.id);youtubeObserved.delete(sender.tab.id);}
        youtubePlayers.set(sender.tab.id,clean);if(youtubePlayers.size>50)youtubePlayers.delete(youtubePlayers.keys().next().value);
        youtubePlayersByVideo.set(clean.videoId,clean);if(youtubePlayersByVideo.size>50)youtubePlayersByVideo.delete(youtubePlayersByVideo.keys().next().value);
      }
      return {ok:true};
    }
    if (message.action === 'remember' && sender.tab) { await remember(sender.tab.id, message.items || []); return {ok: true}; }
    if (message.action === 'download') {
      const tabId=sender.tab?.id ?? message.tabId;
      const explicitReferer = (message.item?.videoId ? `https://www.youtube.com/watch?v=${message.item.videoId}` : null)
        || message.item?.canonicalUrl
        || message.item?.referer;
      const referer = explicitReferer || sender.tab?.url;
      return enqueue({...message.item, referer},tabId);
    }
    if (message.action === 'torrent') {
      const magnet = typeof message.magnet === 'string' ? message.magnet.trim() : '';
      const torrentUrl = typeof message.url === 'string' ? httpUrl(message.url) : null;
      if (magnet && (!/^magnet:\?/i.test(magnet) || !/xt=urn:btih:/i.test(magnet))) {
        throw new Error('The magnet link is invalid.');
      }
      if (!magnet && !torrentUrl) throw new Error('Only magnet links and HTTP(S) .torrent files are supported.');
      await nativeSend({action: 'torrent', magnet: magnet || undefined, url: magnet ? undefined : torrentUrl});
      return {ok: true};
    }
    if (message.action === 'list' && !sender.tab) {
      await serial;
      const data = await api.storage.session.get(CACHE_KEY);
      return {ok: true, items: data[CACHE_KEY]?.[message.tabId] || []};
    }
    if (message.action === 'listForFrame' && sender.tab) {
      const page=new URL(sender.tab.url||'https://invalid.local');
      if(['www.youtube.com','youtube.com','m.youtube.com'].includes(page.hostname)) {
        const id = message.videoId || page.searchParams.get('v') || /^\/shorts\/([\w-]{11})/.exec(page.pathname)?.[1];
        console.log('[ODM Background] listForFrame tab:', sender.tab.id, 'page:', sender.tab.url, 'resolved videoId:', id);
        if(!/^[\w-]{11}$/.test(id||'')) {
          console.warn('[ODM Background] listForFrame could not resolve videoId from page or message');
          return {ok:true,items:[]};
        }
        await api.tabs.sendMessage(sender.tab.id,{action:'refreshPlayer'}).catch(()=>{});
        let player=youtubePlayers.get(sender.tab.id);
        if(player?.videoId!==id && youtubePlayersByVideo.has(id)) {
          console.log('[ODM Background] resolving player from videoId cache:', id);
          player = youtubePlayersByVideo.get(id);
        }
        console.log('[ODM Background] cached player videoId:', player?.videoId, 'expected:', id);
        if(player?.videoId===id) {
          const capture=player.sabr?await sabrCapture(sender.tab.id):undefined;
          console.log('[ODM Background] sabrCapture status:', !!capture);
          const active={...player,sabrUrl:capture?.url||player.sabrUrl};
          let items=globalThis.youtubeMedia.choices(active,youtubeObserved.get(sender.tab.id)||[]);
          console.log('[ODM Background] Returning', items.length, 'format choices');
          if(items.length) {
            return {ok:true,items};
          }
        }
        try {
          const watchUrl = `https://www.youtube.com/watch?v=${id}`;
          console.log('[ODM Background] Querying native bridge for formats on:', watchUrl);
          const nativeRes = await nativeSend({action: 'formats', url: watchUrl});
          if (nativeRes?.ok && Array.isArray(nativeRes.items) && nativeRes.items.length) {
            console.log('[ODM Background] Native bridge returned', nativeRes.items.length, 'formats');
            return {ok: true, items: nativeRes.items};
          }
        } catch (e) {
          console.warn('[ODM Background] native formats fallback error:', e);
        }
        return {ok: true, items: []};
      }
      await serial;
      const data = await api.storage.session.get(CACHE_KEY);
      const policy = globalThis.mediaPolicy;
      return {
        ok: true,
        items: (data[CACHE_KEY]?.[sender.tab.id] || []).filter(item => {
          if (item.unsupported || item.media || item.format === 'HLS') return true;
          if (policy) return policy.isMediaMime(item.mime) || policy.isMediaExtension(item.url);
          return /^(video|audio)\//i.test(item.mime) || /\.(m3u8|mp4|webm|mov|m4v|mp3|m4a|ogg|wav)(?:[?#]|$)/i.test(item.url);
        })
      };
    }
    if(message.action==='extractFormats'&&sender.tab){
      const pageUrl=httpUrl(sender.tab.url||'');
      if(!pageUrl)throw new Error('Only HTTP(S) video pages can be inspected.');
      const result=await nativeSend({action:'formats',url:pageUrl});
      return {ok:true,items:Array.isArray(result.items)?result.items.slice(0,100):[]};
    }
    throw new Error('Unsupported extension request.');
  };
  task().then(respond, error => respond({ok: false, error: error.message}));
  return true;
});
const pendingHandoffs = new Map();

function sharedHandoff(url, payload, tabId) {
  const key = JSON.stringify([tabId, payload.referer || '', url]);
  let pending = pendingHandoffs.get(key);
  if (pending) return pending;
  pending = enqueue(payload, tabId).finally(() => pendingHandoffs.delete(key));
  pendingHandoffs.set(key, pending);
  pending.catch(() => {});
  return pending;
}

const interceptedDownloadIds = new Set();

async function handleInterceptedDownload(item, fromDetermining = false) {
  if (!item || interceptedDownloadIds.has(item.id)) return;
  const url = item.finalUrl || item.url;
  if (!httpUrl(url) || item.state === 'complete') return;
  if (nonReplayableDownload(url, item.tabId)) return;

  let rawFilename = item.filename ? item.filename.split(/[\\/]/).pop().trim() : '';
  if (!rawFilename) {
    try {
      const parsed = new URL(url);
      const leaf = parsed.pathname.split('/').filter(Boolean).pop();
      if (leaf && (leaf.includes('.') || fromDetermining || !api.downloads.onDeterminingFilename)) {
        rawFilename = decodeURIComponent(leaf);
      }
    } catch {}
  }

  // If filename is not yet known and the browser supports onDeterminingFilename,
  // allow onDeterminingFilename to receive server response headers and resolve filename.
  if (!fromDetermining && !rawFilename && api.downloads.onDeterminingFilename) {
    return;
  }

  interceptedDownloadIds.add(item.id);
  if (interceptedDownloadIds.size > 200) {
    const oldest = interceptedDownloadIds.values().next().value;
    interceptedDownloadIds.delete(oldest);
  }

  let paused = false, accepted = false;
  try {
    try { await api.downloads.pause(item.id); paused = true; } catch {}

    let tabId = (item.tabId !== undefined && item.tabId >= 0) ? item.tabId : undefined;
    let referer = item.referrer || '';
    if ((!referer || tabId === undefined) && typeof api.tabs?.query === 'function') {
      try {
        const tabs = await api.tabs.query({ active: true, currentWindow: true });
        if (tabs && tabs[0]) {
          if (tabId === undefined && tabs[0].id >= 0) tabId = tabs[0].id;
          if (!referer && tabs[0].url && tabs[0].url.startsWith('http')) {
            referer = tabs[0].url;
          }
        }
      } catch {}
    }

    const payload = {
      url,
      filename: rawFilename || undefined,
      referer: referer || undefined,
      startNow: true,
    };

    // .torrent downloads are not regular file transfers: confirm with the user
    // and let the desktop fetch metadata, show its file picker, and start the
    // swarm only after that confirmation.
    if (isTorrentDownload(url, rawFilename)) {
      const confirmed = await confirmTorrentHandoff(tabId ?? item.tabId);
      if (!confirmed) {
        if (paused) await api.downloads.resume(item.id).catch(() => {});
        return;
      }
      await nativeSend({action: 'torrent', url});
      accepted = true;
      await eraseBrowserDownload(item);
      return;
    }

    await sharedHandoff(url, payload, tabId ?? item.tabId);
    accepted = true;

    await eraseBrowserDownload(item);
  } catch (e) {
    if (paused && !accepted) {
      await api.downloads.resume(item.id).catch(() => {});
    }
    await api.storage.local.set({
      lastError: accepted
        ? `The browser download could not be canceled; Open Download Manager continues the transfer: ${e.message}`
        : `Browser download was kept: ${e.message}`,
    });
  }
}

api.downloads.onCreated.addListener(item => handleInterceptedDownload(item, false));

if (api.downloads.onDeterminingFilename) {
  api.downloads.onDeterminingFilename.addListener((item, _suggest) => handleInterceptedDownload(item, true));
}

// Service worker start: tell the desktop app that this browser is connected.
scheduleHeartbeat();
sendHeartbeat();
