// Created: 2026-10-24
// Reason: Initial public release
// Last updated: 2026-11-08
// Reason: feat(browser): add configurable context menu actions & file capture rules (#29)
// Description: Unit tests verifying background service worker download interception, tab caching, and native messaging handoff.

const test=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
function harness({initialData={},webRequest={},sendNativeMessage}={}) {
  const events={},specs={},nativeRequests=[],heartbeats=[],alarms=[],downloadCalls=[],badges=[];
  const event=name=>({addListener(fn,filter,spec){events[name]=fn;specs[name]=spec;}});
  const data=structuredClone(initialData),downloads=new Map();
  const storage={async get(key){return structuredClone({[key]:data[key]});},async set(value){Object.assign(data,structuredClone(value));},async remove(key){delete data[key];}};
  const browser={runtime:{id:'extension-id',onMessage:event('message'),onInstalled:event('installed'),getManifest:()=>({version:'0.2.0'}),async sendNativeMessage(host,payload){if(payload&&payload.action==='heartbeat'){heartbeats.push(structuredClone(payload));return sendNativeMessage?sendNativeMessage(payload):{ok:true};}nativeRequests.push(structuredClone(payload));return {ok:true};}},storage:{session:storage,local:storage},webRequest:{...webRequest,onHeadersReceived:event('response'),onBeforeRequest:event('beforeRequest'),onBeforeSendHeaders:event('requestHeaders')},tabs:{onRemoved:event('removed'),onUpdated:event('updated')},contextMenus:{onClicked:event('context'),async removeAll(){},create(){}},downloads:{onCreated:event('download')},action:{async setBadgeText(info){badges.push(info);}},alarms:{create(name,options){alarms.push({name,options});},onAlarm:{addListener(fn){events.alarm=fn;}}}};
  Object.assign(browser.downloads,{
    async pause(id){downloadCalls.push(['pause',id]);downloads.get(id).state='paused';},
    async resume(id){downloadCalls.push(['resume',id]);downloads.get(id).state='in_progress';},
    async cancel(id){downloadCalls.push(['cancel',id]);downloads.delete(id);}
  });
  browser.tabs.sendMessage=async()=>({ok:true});
  vm.runInNewContext(fs.readFileSync(path.join(__dirname,'background.js'),'utf8'),{browser,URL,URLSearchParams,Uint8Array,btoa:value=>Buffer.from(value,'binary').toString('base64'),navigator:{userAgent:'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36'},youtubeMedia:require('./youtube-media.js'),captureEvaluator:require('./capture-evaluator.js')});
  return {browser,events,specs,data,nativeRequests,heartbeats,alarms,downloadCalls,badges,createDownload(item){const value={state:'in_progress',tabId:-1,filename:'',referrer:'',...item};downloads.set(value.id,value);return structuredClone(value);},download:id=>downloads.get(id),request(message,sender){return new Promise(resolve=>{const accepted=events.message(message,sender,resolve);if(accepted!==true)resolve(undefined);});}};
}
test('service worker reports the browser to the desktop app with a heartbeat', async()=>{
  const h=harness();
  assert.equal(h.heartbeats.length,1);
  assert.equal(h.heartbeats[0].action,'heartbeat');
  assert.equal(h.heartbeats[0].browser,'Chrome');
  assert.equal(h.heartbeats[0].extensionVersion,'0.2.0');
  assert.equal(h.nativeRequests.length,0);
});
test('heartbeat runs every two minutes while the desktop app answers', async()=>{
  const h=harness();
  const created=h.alarms.map(alarm=>alarm.options.periodInMinutes);
  assert.ok(created.includes(2),'heartbeat alarm uses a two minute period');
});
test('heartbeat backs off to fifteen minutes while the desktop app is closed', async()=>{
  const h=harness({sendNativeMessage:async payload=>{
    if(payload&&payload.action==='heartbeat')return {ok:false,error:'Open Download Manager is not running.',appRunning:false};
    return {ok:true};
  }});
  await new Promise(resolve=>setImmediate(resolve));
  const periods=h.alarms.map(alarm=>alarm.options.periodInMinutes);
  assert.ok(periods.includes(15),'heartbeat backs off when the desktop app is closed');
});
test('video selection forwards explicit one-click download intent',async()=>{
  const h=harness(),sender={id:'extension-id',tab:{id:3}};
  await h.request({action:'download',item:{url:'https://example.com/video.mp4',startNow:true}},sender);
  assert.equal(h.nativeRequests[0].startNow,true);
  await h.request({action:'download',item:{url:'https://example.com/video.mp4'}},sender);
  assert.equal(h.nativeRequests[1].startNow,false);
});
test('generic format discovery forwards the current HTTP page to the native extractor',async()=>{
 const h=harness(),sender={id:'extension-id',tab:{id:31,url:'https://vkvideo.ru/video-211472768_456240796'}};
 const result=await h.request({action:'extractFormats'},sender);
 assert.equal(result.ok,true);
 assert.deepEqual(h.nativeRequests,[{action:'formats',url:'https://vkvideo.ru/video-211472768_456240796',browser:'Chrome',extensionVersion:'0.2.0'}]);
});
test('later page-source reports do not erase captured media metadata',async()=>{
 const h=harness(),sender={id:'extension-id',tab:{id:32,url:'https://player.example/watch'}};
 const url='https://cdn.example/stream?id=video';
 h.events.response({tabId:32,url,statusCode:200,type:'media',responseHeaders:[{name:'Content-Type',value:'video/mp4'}]});
 await h.request({action:'remember',items:[{url,title:'Captured video',media:true}]},sender);
 const result=await h.request({action:'listForFrame'},sender);
 assert.equal(result.items[0].mime,'video/mp4');
 assert.equal(result.items[0].media,true);
});
test('YouTube menu uses captured player formats without a desktop discovery request',async()=>{
 const h=harness(),sender={id:'extension-id',tab:{id:7,url:'https://www.youtube.com/watch?v=wLdZWyLOJCI'}};
 const base='https://r1.googlevideo.com/videoplayback?id=asset&expire=9999999999&itag=';
 await h.request({action:'playerResponse',data:{videoId:'wLdZWyLOJCI',title:'Captured',formats:[{itag:137,url:base+137,mimeType:'video/mp4',height:1080},{itag:140,url:base+140,mimeType:'audio/mp4'}]}},sender);
 const result=await h.request({action:'listForFrame'},sender);
 assert.equal(result.items[0].audioUrl,base+140);assert.equal(h.nativeRequests.length,0);
 await h.request({action:'download',item:{...result.items[0],startNow:true}},sender);
 assert.equal(h.nativeRequests[0].url,base+137);assert.equal(h.nativeRequests[0].audioUrl,base+140);
 const other=await h.request({action:'listForFrame'},{...sender,tab:{...sender.tab,url:'https://www.youtube.com/watch?v=abcdefghijk'}});
 assert.equal(other.items.length,0);
});
test('captured SABR request context is handed to the desktop with the selected tracks',async()=>{
 const h=harness(),sender={id:'extension-id',tab:{id:8,url:'https://www.youtube.com/watch?v=wLdZWyLOJCI'}};
 const stream='https://r1.googlevideo.com/videoplayback?id=asset&sabr=1&expire=9999999999';
 await h.request({action:'playerResponse',data:{videoId:'wLdZWyLOJCI',title:'Captured',sabr:true,sabrUrl:stream,ustreamerConfig:'AQID',clientName:1,clientVersion:'2.20261022.00.00',formats:[
  {itag:137,mimeType:'video/mp4; codecs="avc1"',height:1080,qualityLabel:'1080p',bitrate:3000000,lastModified:'1',approxDurationMs:'10000'},
  {itag:140,mimeType:'audio/mp4; codecs="mp4a.40.2"',audioQuality:'AUDIO_QUALITY_MEDIUM',bitrate:128000,lastModified:'1',approxDurationMs:'10000'}
 ]}},sender);
 h.events.beforeRequest({requestId:'success',tabId:8,url:stream,requestBody:{raw:[{bytes:Uint8Array.from([1,2,3]).buffer}]}});
 h.events.response({requestId:'success',tabId:8,url:stream,statusCode:200,type:'xmlhttprequest',responseHeaders:[{name:'Content-Type',value:'application/vnd.yt-ump'}]});
 // A later in-flight or rejected request must not replace the successful body.
 h.events.beforeRequest({requestId:'pending',tabId:8,url:stream+'&seq=2',requestBody:{raw:[{bytes:Uint8Array.from([4,5,6]).buffer}]}});
 const item=(await h.request({action:'listForFrame'},sender)).items.find(value=>value.quality==='1080p');assert.ok(item);
 await h.request({action:'download',item:{...item,startNow:true}},sender);
 assert.equal(h.nativeRequests[0].sabr.videoItag,137);assert.equal(h.nativeRequests[0].sabr.audioItag,140);assert.equal(h.nativeRequests[0].sabr.requestBody,'AQID');
 assert.deepEqual(h.nativeRequests[0].sabr.formats.map(value=>value.itag),[137,140]);
 assert.ok(Buffer.byteLength(JSON.stringify(h.nativeRequests[0]))<1048576);
 assert.equal(h.nativeRequests[0].sabr.serverAbrStreamingUrl,stream);
 h.events.response({requestId:'pending',tabId:8,url:stream+'&seq=2',statusCode:403});
 await h.request({action:'download',item:{...item,startNow:true}},sender);
 assert.equal(h.nativeRequests[1].sabr.requestBody,'AQID');
 h.events.updated(8,{status:'loading'});
 const rejected=await h.request({action:'download',item:{...item,startNow:true}},sender);
 assert.equal(rejected.ok,false);assert.equal(h.nativeRequests.length,2);
});
test('large YouTube format catalogs stay below the native messaging limit',async()=>{
 const h=harness(),sender={id:'extension-id',tab:{id:18,url:'https://www.youtube.com/watch?v=wLdZWyLOJCI'}};
 const stream='https://r1.googlevideo.com/videoplayback?id=asset&sabr=1&expire=9999999999';
 const formats=Array.from({length:148},(_,index)=>({itag:2000+index,mimeType:'video/mp4; codecs="avc1"',height:144,qualityLabel:'144p',bitrate:100000,lastModified:'1',approxDurationMs:'10000',sourceUrl:'https://example.com/'+('x'.repeat(12000))+index}));
 formats.push({itag:137,mimeType:'video/mp4; codecs="avc1"',height:1080,qualityLabel:'1080p',bitrate:3000000,lastModified:'1',approxDurationMs:'10000'},{itag:140,mimeType:'audio/mp4; codecs="mp4a.40.2"',audioQuality:'AUDIO_QUALITY_MEDIUM',bitrate:128000,lastModified:'1',approxDurationMs:'10000'});
 await h.request({action:'playerResponse',data:{videoId:'wLdZWyLOJCI',title:'Captured',sabr:true,ustreamerConfig:'AQID',clientName:1,clientVersion:'2.20261022.00.00',formats}},sender);
 h.events.beforeRequest({requestId:'large',tabId:18,url:stream,requestBody:{raw:[{bytes:new Uint8Array(200000).buffer}]}});
 h.events.response({requestId:'large',tabId:18,url:stream,statusCode:200,type:'xmlhttprequest',responseHeaders:[]});
 const item=(await h.request({action:'listForFrame'},sender)).items.find(value=>value.quality==='1080p');
 await h.request({action:'download',item:{...item,startNow:true}},sender);
 assert.deepEqual(h.nativeRequests[0].sabr.formats.map(value=>value.itag),[137,140]);
 assert.ok(Buffer.byteLength(JSON.stringify(h.nativeRequests[0]))<1048576);
});
test('captured YouTube session survives a service worker restart',async()=>{
 const sender={id:'extension-id',tab:{id:28,url:'https://www.youtube.com/watch?v=wLdZWyLOJCI'}},stream='https://r1.googlevideo.com/videoplayback?id=asset&sabr=1&expire=9999999999';
 const player={videoId:'wLdZWyLOJCI',title:'Captured',sabr:true,ustreamerConfig:'AQID',clientName:1,clientVersion:'2.20261022.00.00',formats:[{itag:137,mimeType:'video/mp4; codecs="avc1"',height:1080,qualityLabel:'1080p',bitrate:3000000,lastModified:'1',approxDurationMs:'10000'},{itag:140,mimeType:'audio/mp4; codecs="mp4a.40.2"',audioQuality:'AUDIO_QUALITY_MEDIUM',bitrate:128000,lastModified:'1',approxDurationMs:'10000'}]};
 const first=harness();await first.request({action:'playerResponse',data:player},sender);
 first.events.beforeRequest({requestId:'persist',tabId:28,url:stream,requestBody:{raw:[{bytes:Uint8Array.from([1,2,3]).buffer}]}});
 first.events.response({requestId:'persist',tabId:28,url:stream,statusCode:200,type:'xmlhttprequest',responseHeaders:[]});
 await new Promise(resolve=>setImmediate(resolve));
 const restarted=harness({initialData:first.data});await restarted.request({action:'playerResponse',data:player},sender);
 const item=(await restarted.request({action:'listForFrame'},sender)).items.find(value=>value.quality==='1080p');assert.ok(item);
 await restarted.request({action:'download',item:{...item,startNow:true}},sender);
 assert.equal(restarted.nativeRequests[0].sabr.requestBody,'AQID');
});
test('SABR choices are available immediately from player metadata',async()=>{
 const h=harness(),sender={id:'extension-id',tab:{id:29,url:'https://www.youtube.com/watch?v=wLdZWyLOJCI'}};
 await h.request({action:'playerResponse',data:{videoId:'wLdZWyLOJCI',title:'Captured',sabr:true,sabrUrl:'https://r1.googlevideo.com/videoplayback?id=asset&sabr=1&expire=9999999999',ustreamerConfig:'AQID',clientName:1,clientVersion:'2.20261022.00.00',formats:[{itag:137,mimeType:'video/mp4',height:1080,qualityLabel:'1080p',bitrate:3000000},{itag:140,mimeType:'audio/mp4',bitrate:128000}]}},sender);
 const result=await h.request({action:'listForFrame'},sender);assert.ok(result.items.length>0);assert.equal(result.items[0].quality,'1080p');
});
test('duplicate browser downloads share one native handoff',async()=>{
 const h=harness();
 const first=h.createDownload({id:1,url:'https://example.com/file.bin',filename:'C:\\Downloads\\file.bin'}),second=h.createDownload({id:2,url:first.url,filename:first.filename});
 await Promise.all([h.events.download(first),h.events.download(second)]);
 assert.equal(h.nativeRequests.length,1);assert.equal(h.download(1),undefined);assert.equal(h.download(2),undefined);
 assert.deepEqual(h.downloadCalls.filter(([name])=>name==='pause').length,2);assert.deepEqual(h.downloadCalls.filter(([name])=>name==='cancel').length,2);
});
test('POST form downloads stay with the browser because their body cannot be replayed',async()=>{
  const h=harness();
  const url='https://drive.usercontent.google.com/download';
  h.events.requestHeaders({tabId:3,url,method:'POST',requestHeaders:[]});
  const item=h.createDownload({id:7,tabId:3,url,filename:'Folder.zip',referrer:'https://drive.google.com/drive/folders/abc'});
  await h.events.download(item);
  assert.equal(h.nativeRequests.length,0);
  assert.deepEqual(h.downloadCalls,[]);
  assert.ok(h.download(7));
});
test('bare Google Drive download endpoint without a file id is not intercepted',async()=>{
  const h=harness();
  const item=h.createDownload({id:8,tabId:3,url:'https://drive.usercontent.google.com/download',filename:'Folder.zip',referrer:'https://drive.google.com/drive/folders/abc'});
  await h.events.download(item);
  assert.equal(h.nativeRequests.length,0);
  assert.deepEqual(h.downloadCalls,[]);
  assert.ok(h.download(8));
});
test('replayable GET downloads are still intercepted',async()=>{
  const h=harness();
  const url='https://drive.usercontent.google.com/download?id=FILE&export=download&confirm=t';
  h.events.requestHeaders({tabId:3,url,method:'GET',requestHeaders:[]});
  const item=h.createDownload({id:9,tabId:3,url,filename:'movie.mkv'});
  await h.events.download(item);
  assert.equal(h.nativeRequests.length,1);
  assert.equal(h.nativeRequests[0].url,url);
  assert.equal(h.download(9),undefined);
});
test('a later GET clears an earlier POST method for the same URL',async()=>{
  const h=harness();
  const url='https://example.com/export';
  h.events.requestHeaders({tabId:3,url,method:'POST',requestHeaders:[]});
  h.events.requestHeaders({tabId:3,url,method:'GET',requestHeaders:[]});
  const item=h.createDownload({id:10,tabId:-1,url,filename:'export.bin'});
  await h.events.download(item);
  assert.equal(h.nativeRequests.length,1);
  assert.equal(h.nativeRequests[0].url,url);
});
test('browser download is intercepted even if pausing fails on non-resumable download',async()=>{
 const h=harness();
 const download=h.createDownload({id:99,url:'https://example.com/movie.mkv',filename:'movie.mkv'});
 // Simulate Chrome error when pausing non-resumable downloads ("Download cannot be paused")
 h.browser.downloads.pause = async () => { throw new Error('Download cannot be paused'); };
 await h.events.download(download);
 assert.equal(h.nativeRequests.length,1);
 assert.equal(h.nativeRequests[0].url,'https://example.com/movie.mkv');
 assert.deepEqual(h.downloadCalls.filter(([name])=>name==='cancel').length,1);
});
test('magnet links and .torrent URLs are forwarded to the torrent bridge',async()=>{
 const h=harness(),sender={id:'extension-id',tab:{id:4,url:'https://example.com/list'}};
 const magnet='magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862&dn=ubuntu.iso';
 const result=await h.request({action:'torrent',magnet},sender);
 assert.equal(result.ok,true);
 assert.equal(h.nativeRequests[0].action,'torrent');
 assert.equal(h.nativeRequests[0].magnet,magnet);
 assert.equal(h.nativeRequests[0].url,undefined);
 const file=await h.request({action:'torrent',url:'https://example.com/releases/ubuntu.torrent'},sender);
 assert.equal(file.ok,true);
 assert.equal(h.nativeRequests[1].url,'https://example.com/releases/ubuntu.torrent');
 assert.equal(h.nativeRequests[1].magnet,undefined);
});
test('magnet requests without a BitTorrent info hash are rejected',async()=>{
 const h=harness(),sender={id:'extension-id',tab:{id:4}};
 const result=await h.request({action:'torrent',magnet:'magnet:?dn=missing'},sender);
 assert.equal(result.ok,false);
 assert.equal(h.nativeRequests.length,0);
});
test('.torrent downloads are confirmed before the browser hands them over',async()=>{
 const h=harness(),asked=[];
 h.browser.tabs.sendMessage=async(tabId,message)=>{asked.push([tabId,message]);return {confirmed:true};};
 const item=h.createDownload({id:21,tabId:5,url:'https://example.com/releases/ubuntu.torrent',filename:'ubuntu.torrent'});
 await h.events.download(item);
 assert.deepEqual(asked,[[5,{action:'confirmTorrent',kind:'file'}]]);
 assert.equal(h.nativeRequests.length,1);
 assert.equal(h.nativeRequests[0].action,'torrent');
 assert.equal(h.nativeRequests[0].url,'https://example.com/releases/ubuntu.torrent');
 assert.equal(h.download(21),undefined);
 assert.deepEqual(h.downloadCalls.map(([name])=>name),['pause','cancel']);
});
test('declining a .torrent confirmation keeps the browser download',async()=>{
 const h=harness();
 h.browser.tabs.sendMessage=async()=>({confirmed:false});
 const item=h.createDownload({id:22,tabId:5,url:'https://example.com/releases/ubuntu.torrent',filename:'ubuntu.torrent'});
 await h.events.download(item);
 assert.equal(h.nativeRequests.length,0);
 assert.ok(h.download(22));
 assert.deepEqual(h.downloadCalls.map(([name])=>name),['pause','resume']);
});

test('identical URLs from different tabs do not share a credential-bearing handoff',async()=>{
 const h=harness();
 const url='https://example.com/file.bin';
 for(const tabId of [3,4])h.events.requestHeaders({tabId,url,requestHeaders:[{name:'Cookie',value:'session='+tabId}]});
 await Promise.all([3,4].map(tabId=>h.events.download(h.createDownload({id:tabId,tabId,url}))));
 assert.equal(h.nativeRequests.length,2);
 assert.deepEqual(h.nativeRequests.map(r=>r.requestHeaders.cookie).sort(),['session=3','session=4']);
});

test('observing a video-only leaf never erases its master audio pairing',async()=>{
  const h=harness(),sender={id:'extension-id',tab:{id:3}};
  const url='https://example.com/video.m3u8';
  await h.request({action:'remember',items:[{url,format:'HLS',quality:'720p',audioUrl:'https://example.com/audio.m3u8',masterUrl:'https://example.com/master.m3u8',codecs:'avc1,mp4a'}]},sender);
  await h.request({action:'remember',items:[{url,format:'HLS'}]},sender);
  const item=(await h.request({action:'listForFrame'},sender)).items[0];
  assert.equal(item.audioUrl,'https://example.com/audio.m3u8');
  assert.equal(item.masterUrl,'https://example.com/master.m3u8');
});
test('frame list is restricted to its sender tab and excludes non-media links',async()=>{
  const h=harness();
  await h.request({action:'remember',items:[{url:'https://example.com/a.mp4'},{url:'https://example.com/page'},{url:'https://example.com/a.mpd'}]},{id:'extension-id',tab:{id:3}});
  await h.request({action:'remember',items:[{url:'https://private.example/other.mp4'}]},{id:'extension-id',tab:{id:4}});
  const result=await h.request({action:'listForFrame',tabId:4},{id:'extension-id',tab:{id:3}});
  assert.equal(result.items.length,2); assert.equal(result.items[0].url,'https://example.com/a.mp4'); assert.equal(result.items[1].unsupported,true);
  assert.equal(await h.request({action:'listForFrame'},{id:'foreign',tab:{id:3}}),undefined);
  assert.equal((await h.request({action:'listForFrame'},{id:'extension-id'})).ok,false);
});

test('request credentials stay out of storage and cannot cross tab or URL boundaries',async()=>{
  const h=harness(),url='https://example.com/video.mp4';
  h.events.requestHeaders({tabId:3,url,requestHeaders:[{name:'Cookie',value:'session=test-only'}]});
  await h.request({action:'download',item:{url}},{id:'extension-id',tab:{id:3}});
  assert.equal(h.nativeRequests[0].requestHeaders.cookie,'session=test-only');
  await h.request({action:'download',item:{url}},{id:'extension-id',tab:{id:4}});
  assert.equal(h.nativeRequests[1].requestHeaders,undefined);
  assert.equal(JSON.stringify(h.data).includes('session=test-only'),false);
  h.events.updated(3,{status:'loading'});
  await h.request({action:'download',item:{url}},{id:'extension-id',tab:{id:3}});
  assert.equal(h.nativeRequests[2].requestHeaders,undefined);
});
test('Chromium is asked for extraHeaders so the captured cookie reaches the desktop',async()=>{
  // Like Chromium: `browser` is defined, and Cookie is only visible when extraHeaders was requested.
  const h=harness({webRequest:{OnBeforeSendHeadersOptions:{EXTRA_HEADERS:'extraHeaders'}}}),url='https://example.com/private.mp4';
  const visible=h.specs.requestHeaders.includes('extraHeaders')?[{name:'Cookie',value:'session=test-only'}]:[];
  h.events.requestHeaders({tabId:3,url,requestHeaders:[{name:'User-Agent',value:'Test'},...visible]});
  await h.request({action:'download',item:{url}},{id:'extension-id',tab:{id:3}});
  assert.equal(h.nativeRequests[0].requestHeaders.cookie,'session=test-only');
  assert.deepEqual([...harness().specs.requestHeaders],['requestHeaders']);
});

test('individual HLS media fragments are not listed as complete videos',async()=>{
  const h=harness(),sender={id:'extension-id',tab:{id:3}};
  await h.request({action:'remember',items:['a.m4s','a.ts','init.mp4','movie.mp4'].map(name=>({url:'https://example.com/'+name}))},sender);
  const result=await h.request({action:'listForFrame'},sender);
  assert.equal(result.items.length,1);assert.equal(result.items[0].url,'https://example.com/movie.mp4');
});
test('media cache is bounded, deduplicated, and cleared on navigation',async()=>{
  const h=harness(),sender={id:'extension-id',tab:{id:9}};
  const items=Array.from({length:120},(_,i)=>({url:`https://example.com/${i}.mp4`}));
  await h.request({action:'remember',items:items.slice(0,80)},sender);
  await h.request({action:'remember',items:items.slice(60)},sender);
  let result=await h.request({action:'listForFrame'},sender);
  assert.equal(result.items.length,100); assert.equal(new Set(result.items.map(i=>i.url)).size,100);
  h.events.updated(9,{status:'loading'});
  result=await h.request({action:'listForFrame'},sender); assert.equal(result.items.length,0);
});

test('real human scenario: context menu download on allowed link forwards intent and sets OK badge', async () => {
  const policy = {
    enabled: true,
    fileTypes: ['iso', 'zip', 'mp4'],
    excludedHosts: ['*.internal.corp'],
    excludedAddresses: ['192.168.1.100'],
    confirmationMode: 'askForUnknown'
  };
  const h = harness({ initialData: { capturePolicy: policy } });
  const tab = { id: 10, url: 'https://releases.ubuntu.com/' };

  // Real human right-clicks link and clicks "Download with Open Download Manager"
  await h.events.context({ menuItemId: 'odm-link', linkUrl: 'https://releases.ubuntu.com/noble.iso' }, tab);

  assert.equal(h.nativeRequests.length, 1);
  assert.equal(h.nativeRequests[0].url, 'https://releases.ubuntu.com/noble.iso');
  assert.equal(h.nativeRequests[0].startNow, true);
  assert.deepEqual(h.badges[h.badges.length - 1], { tabId: 10, text: 'OK' });
});

test('real human scenario: context menu download on excluded corporate host is blocked before dispatch', async () => {
  const policy = {
    enabled: true,
    fileTypes: ['iso', 'zip', 'mp4'],
    excludedHosts: ['*.internal.corp', 'secret.bank.com'],
    excludedAddresses: [],
    confirmationMode: 'askForUnknown'
  };
  const h = harness({ initialData: { capturePolicy: policy } });
  const tab = { id: 11, url: 'https://portal.internal.corp/' };

  // Real human right-clicks an intranet internal link
  await h.events.context({ menuItemId: 'odm-link', linkUrl: 'https://dev.internal.corp/firmware.zip' }, tab);

  // Must NOT trigger native messaging
  assert.equal(h.nativeRequests.length, 0);
  // Last error must describe the exclusion
  assert.match(h.data.lastError, /Host 'dev\.internal\.corp' matches exclusion '\*\.internal\.corp'/);
  // Badge must show '!' error indicator
  assert.deepEqual(h.badges[h.badges.length - 1], { tabId: 11, text: '!' });
});

test('real human scenario: context menu on excluded address or path prefix is blocked', async () => {
  const policy = {
    enabled: true,
    fileTypes: ['zip', 'exe'],
    excludedHosts: [],
    excludedAddresses: ['10.0.0.1', 'https://example.com/confidential/'],
    confirmationMode: 'askForUnknown'
  };
  const h = harness({ initialData: { capturePolicy: policy } });
  const tab = { id: 12, url: 'https://example.com/' };

  // Real human right-clicks confidential link
  await h.events.context({ menuItemId: 'odm-link', linkUrl: 'https://example.com/confidential/keys.zip' }, tab);

  assert.equal(h.nativeRequests.length, 0);
  assert.match(h.data.lastError, /Address matches exclusion/);
  assert.deepEqual(h.badges[h.badges.length - 1], { tabId: 12, text: '!' });
});

test('real human scenario: context menu on unrecognized file extension in askForUnknown mode sets startNow=false for desktop review', async () => {
  const policy = {
    enabled: true,
    fileTypes: ['zip', 'mp4'],
    excludedHosts: [],
    excludedAddresses: [],
    confirmationMode: 'askForUnknown'
  };
  const h = harness({ initialData: { capturePolicy: policy } });
  const tab = { id: 13, url: 'https://downloads.example.com/' };

  // Real human right-clicks on an unfamiliar .xyz file
  await h.events.context({ menuItemId: 'odm-link', linkUrl: 'https://downloads.example.com/installer.xyz' }, tab);

  assert.equal(h.nativeRequests.length, 1);
  assert.equal(h.nativeRequests[0].url, 'https://downloads.example.com/installer.xyz');
  // startNow must be false so the desktop client opens the confirmation/review dialog rather than downloading silently
  assert.equal(h.nativeRequests[0].startNow, false);
  assert.deepEqual(h.badges[h.badges.length - 1], { tabId: 13, text: 'OK' });
});
